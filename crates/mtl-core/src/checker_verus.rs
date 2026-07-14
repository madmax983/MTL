//! Layer C — Static stack-effect checker soundness (Milestone 1)
//! ==============================================================
//!
//! Mechanization of the v0.6 checker's soundness theorems (docs/design/v0.6-checker.md
//! §3) over the FROZEN `spec_step` semantics. This file is **purely additive**: it
//! pulls in `mtl_core` unmodified via `#[path] mod mtl_core;` (exactly the layout of
//! `p5_universality.rs`) and adds a spec-level abstract-interpretation checker plus
//! soundness proofs. It touches no verified file.
//!
//! ## Milestone-1 fragment (DELIBERATELY NARROW — review §20, six-milestone plan)
//!
//! The fragment = programs whose words are only:
//!   * `PushInt`                       (push an integer literal)
//!   * `PushQuote` — a literal quote pushed **as a value**, never applied
//!   * the shuffles  `Dup Drop Swap Rot Over`
//!   * the arith/cmp `Add Sub Mul Div Mod Xor Eq Lt`
//!   * `If`, where BOTH branches are literal quotes (`Lit`) drawn from the SAME fragment.
//!
//! EXCLUDED this milestone: `Apply`/`Dip` inlining, `Cat`/`Cons`, `Uncons`, the four
//! recursion combinators (`PrimRec Times LinRec Fold`) and `Call`. `:!` (self-apply)
//! and opaque-`uncons` are **permanently outside** the Layer-C theorem (the Layer-C
//! boundary — the design's §14.5 / §20 host-and-open-recursion seam); they are not a
//! milestone to be reached later, they are out of scope by construction.
//!
//! ## The three theorems (design §3), scoped to this fragment
//!
//!   * **T-Static** — if `check` accepts `p` with effect `eff`, then from any initial
//!     stack, running `spec_step` never reaches `Fault(Underflow)` / `Fault(TypeMismatch)`,
//!     and any `Halt(final)` has `final` satisfying `eff.post`. Overflow/DivByZero are
//!     explicitly ALLOWED (arithmetic-value faults, excluded from Layer C).
//!   * **T-Progress** — every non-halted reachable state either steps or is an arithmetic
//!     (Overflow/DivByZero) fault; never *stuck* at an Underflow/TypeMismatch point.
//!   * **T-Branch** — an `If` on two Lit branches of equal net height delta and joinable
//!     per-cell outputs has a single well-defined post shape independent of the flag.
//!
//! ## Row-polymorphism (milestone-1 slice)
//!
//! `Effect` inference is `∀ρ. ρ ++ pre → ρ ++ post`. This milestone proves the
//! `pre = []` slice: the checker starts from the empty abstract stack, so accepted
//! programs are *self-contained* (they push their own operands before consuming). The
//! polymorphic base `ρ` is the ARBITRARY concrete stack the program runs on top of;
//! `models_stack` constrains only the top `post.len()` cells, leaving `ρ` free and (as
//! the primitive lemmas show) untouched below. Non-empty-`pre` borrow inference is a
//! later-milestone extension, not attempted here.
//!
//! ## HONEST STATUS (proved / bridged boundary — see the per-item comments)
//!
//!   FULLY PROVEN (0 errors, no assume/admit):
//!     * `lemma_prim_step_sound`  — per-primitive progress+preservation (T-Progress core)
//!     * `thm_progress`           — T-Progress over one reachable straight-line step
//!     * `lemma_sl_invariant` / `thm_static_straightline` — T-Static for the If-FREE fragment
//!     * `thm_branch_progress`    — the `If` step never faults Underflow/TypeMismatch
//!     * `lemma_join_sound`       — T-Branch: post-If shape well-defined independent of flag
//!     * `lemma_check_invariant` / `thm_static_with_if` — T-Static for the full straight-line
//!       + `If` fragment (milestone-2 Part A). The If-INLINING correspondence is now
//!       machine-checked: `lemma_check_compose` (checker splice, à la p5's
//!       `lemma_stepn_compose`) + `lemma_check_depth_mono` + `lemma_joinable_eq` collapse
//!       the branch/join mismatch, so the induction on the concrete step count closes with
//!       NO `assume`. The milestone-1 gap is GONE.

use vstd::prelude::*;

#[path = "mtl_core.rs"]
mod mtl_core;
use mtl_core::*;

verus! {

// ============================================================
// 0. Iterated spec_step (concrete step counting)
// ============================================================
//
// Same definition as p5_universality.rs's `spec_stepn` (that file is a sibling
// standalone verus root, so we re-declare it here rather than share it). Run
// `spec_step` exactly `k` times; a Halt/Fault short-circuits and is returned.
pub open spec fn spec_stepn(s: SpecState, k: nat) -> SpecStep
    decreases k,
{
    if k == 0 {
        SpecStep::Next(s)
    } else {
        match spec_step(s) {
            SpecStep::Next(s2) => spec_stepn(s2, (k - 1) as nat),
            other => other,
        }
    }
}

// ============================================================
// 1. Abstract value lattice (milestone-1 restriction: Int | Lit)
// ============================================================
//
// The design lattice is `Int | Lit(body) | OpaqueQuote | Any`. The milestone-1
// fragment never produces `OpaqueQuote`/`Any` (those arise only from host `Call`
// results, borrowed values, or `Any`-joins — all excluded), so two cells suffice:
//   * `AInt`         — definitely an integer.
//   * `ALit(body)`   — definitely a quotation whose literal body is exactly `body`.

pub enum AbsVal {
    AInt,
    ALit(Seq<SpecWord>),
}

/// A concrete `SpecValue` refines an abstract cell.
pub open spec fn models_val(v: SpecValue, a: AbsVal) -> bool {
    match a {
        AbsVal::AInt => v is Int,
        AbsVal::ALit(body) => match v {
            SpecValue::Quote(q) => q == body,
            SpecValue::Int(_) => false,
        },
    }
}

/// Row-polymorphic stack refinement: the TOP `astk.len()` cells of the concrete
/// stack `cs` refine `astk` cell-by-cell; everything below (the polymorphic base
/// `ρ`) is unconstrained. `cs.len() >= astk.len()` is the height guarantee.
pub open spec fn models_stack(cs: Seq<SpecValue>, astk: Seq<AbsVal>) -> bool {
    &&& cs.len() >= astk.len()
    &&& forall|j: int| 0 <= j < astk.len() ==>
            models_val(#[trigger] cs[cs.len() - astk.len() + j], astk[j])
}

// ------------------------------------------------------------
// models_stack helper lemmas (push / the base offset)
// ------------------------------------------------------------

/// Pushing a refining value onto both sides preserves refinement. This is the
/// workhorse for Dup/Over/PushInt/PushQuote and every arith result cell.
pub proof fn lemma_models_push(cs: Seq<SpecValue>, astk: Seq<AbsVal>, v: SpecValue, a: AbsVal)
    requires
        models_stack(cs, astk),
        models_val(v, a),
    ensures
        models_stack(cs.push(v), astk.push(a)),
{
    let cs2 = cs.push(v);
    let astk2 = astk.push(a);
    assert forall|j: int| 0 <= j < astk2.len() implies
        models_val(cs2[cs2.len() - astk2.len() + j], astk2[j])
    by {
        if j < astk.len() {
            // old cell: index unchanged because both lengths grew by 1.
            assert(cs2.len() - astk2.len() + j == cs.len() - astk.len() + j);
            assert(cs2[cs2.len() - astk2.len() + j] == cs[cs.len() - astk.len() + j]);
            assert(astk2[j] == astk[j]);
        } else {
            // the freshly pushed top cell.
            assert(j == astk.len());
            assert(cs2.len() - astk2.len() + j == cs2.len() - 1);
            assert(cs2[cs2.len() - 1] == v);
            assert(astk2[j] == a);
        }
    }
}

// ============================================================
// 2. The abstract checker (milestone-1)
// ============================================================
//
// `abs_step_prim` types the shuffles + arith/cmp abstractly. It returns
// `Some(astk')` when the primitive is abstractly well-typed on `astk` (enough
// cells, right kinds), and `None` when it would provably fault
// Underflow/TypeMismatch. Excluded primitives and `If` return `None` here (If is
// handled separately by the recursive `check_m1`, which has the branch bodies).

pub open spec fn absv_is_int(a: AbsVal) -> bool {
    a matches AbsVal::AInt
}

/// The six arith/cmp primitives that can only fault Overflow (Add/Sub/Mul) or
/// never fault (Xor/Eq/Lt) — i.e. no DivByZero arm.
pub open spec fn is_arith6(p: SpecPrim) -> bool {
    match p {
        SpecPrim::Add | SpecPrim::Sub | SpecPrim::Mul
        | SpecPrim::Xor | SpecPrim::Eq | SpecPrim::Lt => true,
        _ => false,
    }
}

/// The two primitives with a DivByZero / Overflow arm.
pub open spec fn is_divmod2(p: SpecPrim) -> bool {
    match p {
        SpecPrim::Div | SpecPrim::Mod => true,
        _ => false,
    }
}

pub open spec fn abs_step_prim(astk: Seq<AbsVal>, p: SpecPrim) -> Option<Seq<AbsVal>> {
    let n = astk.len() as int;
    match p {
        // ---- shuffles: no type constraint, only arity ----
        SpecPrim::Dup =>
            if n < 1 { None } else { Some(astk.push(astk[n - 1])) },
        SpecPrim::Drop =>
            if n < 1 { None } else { Some(astk.subrange(0, n - 1)) },
        SpecPrim::Swap =>
            if n < 2 { None } else {
                Some(astk.subrange(0, n - 2).push(astk[n - 1]).push(astk[n - 2]))
            },
        SpecPrim::Rot =>
            if n < 3 { None } else {
                Some(astk.subrange(0, n - 3).push(astk[n - 2]).push(astk[n - 1]).push(astk[n - 3]))
            },
        SpecPrim::Over =>
            if n < 2 { None } else { Some(astk.push(astk[n - 2])) },
        // ---- arith / cmp: require top two = AInt, push AInt ----
        SpecPrim::Add | SpecPrim::Sub | SpecPrim::Mul | SpecPrim::Div | SpecPrim::Mod
        | SpecPrim::Xor | SpecPrim::Eq | SpecPrim::Lt =>
            if n < 2 { None }
            else if absv_is_int(astk[n - 1]) && absv_is_int(astk[n - 2]) {
                Some(astk.subrange(0, n - 2).push(AbsVal::AInt))
            } else { None },
        // ---- everything else is out of the milestone-1 straight-line fragment ----
        _ => None,
    }
}

/// Straight-line abstract step over a single word (no `If`, no excluded prims).
/// `PushQuote(q)` pushes `ALit(q)` — the literal body is tracked (design's `Lit`).
pub open spec fn abs_step_word(astk: Seq<AbsVal>, w: SpecWord) -> Option<Seq<AbsVal>> {
    match w {
        SpecWord::PushInt(_) => Some(astk.push(AbsVal::AInt)),
        SpecWord::PushQuote(q) => Some(astk.push(AbsVal::ALit(q))),
        SpecWord::Prim(p) => abs_step_prim(astk, p),
        SpecWord::Call(_) => None,
    }
}

/// `abs_run` folds `abs_step_word` head-first over the program — matching the
/// head-first consumption of `spec_step`. `Some(post)` = accepted (checker verdict
/// `Static`, with `pre = []` and this `post`); `None` = rejected.
pub open spec fn abs_run(astk: Seq<AbsVal>, p: Seq<SpecWord>) -> Option<Seq<AbsVal>>
    decreases p.len(),
{
    if p.len() == 0 {
        Some(astk)
    } else {
        match abs_step_word(astk, p[0]) {
            Some(astk2) => abs_run(astk2, p.subrange(1, p.len() as int)),
            None => None,
        }
    }
}

/// Milestone-1 top-level checker entry: accept with `pre = []`.
pub open spec fn check_static_m1(p: Seq<SpecWord>) -> Option<Seq<AbsVal>> {
    abs_run(seq![], p)
}

// ------------------------------------------------------------
// The straight-line fragment predicate (excludes If + all excluded prims).
// PushQuote bodies are NOT required straight-line: a pushed quote is a VALUE in
// this fragment (never applied), so its contents are irrelevant to execution.
// ------------------------------------------------------------

pub open spec fn is_sl_prim(p: SpecPrim) -> bool {
    match p {
        SpecPrim::Dup | SpecPrim::Drop | SpecPrim::Swap | SpecPrim::Rot | SpecPrim::Over
        | SpecPrim::Add | SpecPrim::Sub | SpecPrim::Mul | SpecPrim::Div | SpecPrim::Mod
        | SpecPrim::Xor | SpecPrim::Eq | SpecPrim::Lt => true,
        _ => false,
    }
}

pub open spec fn is_sl_word(w: SpecWord) -> bool {
    match w {
        SpecWord::PushInt(_) => true,
        SpecWord::PushQuote(_) => true,
        SpecWord::Prim(p) => is_sl_prim(p),
        SpecWord::Call(_) => false,
    }
}

pub open spec fn is_straightline(p: Seq<SpecWord>) -> bool {
    forall|i: int| 0 <= i < p.len() ==> is_sl_word(#[trigger] p[i])
}

// ============================================================
// 3. T-Progress — per-primitive progress + preservation
// ============================================================
//
// The heart of the mechanization. For every straight-line primitive: if the
// abstract stack accepts it (`abs_step_prim = Some(astk2)`) and the concrete stack
// refines the abstract stack, then `spec_step_prim`:
//   * NEVER faults Underflow or TypeMismatch (progress), and
//   * on `Next(s2)` the result refines `astk2` with the continuation untouched
//     (preservation), and
//   * the only faults it may raise are Overflow / DivByZero (allowed).

pub proof fn lemma_prim_step_sound(
    cs: Seq<SpecValue>, astk: Seq<AbsVal>, p: SpecPrim, rest: Seq<SpecWord>, astk2: Seq<AbsVal>,
)
    requires
        is_sl_prim(p),
        models_stack(cs, astk),
        abs_step_prim(astk, p) == Some(astk2),
    ensures
        match spec_step_prim(cs, p, rest) {
            SpecStep::Next(s2) => models_stack(s2.stack, astk2) && s2.cont == rest,
            SpecStep::Fault(e) => e == Error::Overflow || e == Error::DivByZero,
            _ => false,
        },
{
    let n = cs.len() as int;
    let m = astk.len() as int;
    // abs accepted => enough abstract cells => enough concrete cells (models_stack
    // gives cs.len() >= astk.len(), and abs_step_prim's arity guard gives the rest).
    match p {
        SpecPrim::Dup => {
            assert(m >= 1);
            assert(n >= 1);
            // spec_step_prim Dup: Next(stk.push(stk[n-1])).
            // astk2 = astk.push(astk[m-1]); the duplicated top cell refines because
            // cs's top cell refines astk's top cell.
            assert(models_val(cs[n - 1], astk[m - 1])) by {
                assert(cs[cs.len() - astk.len() + (m - 1)] == cs[n - 1]);
            }
            lemma_models_push(cs, astk, cs[n - 1], astk[m - 1]);
        }
        SpecPrim::Drop => {
            assert(m >= 1);
            assert(n >= 1);
            // Next(cs.subrange(0, n-1)); astk2 = astk.subrange(0, m-1).
            lemma_models_drop(cs, astk);
        }
        SpecPrim::Swap => {
            assert(m >= 2);
            lemma_prim_swap(cs, astk);
        }
        SpecPrim::Rot => {
            assert(m >= 3);
            lemma_prim_rot(cs, astk);
        }
        SpecPrim::Over => {
            assert(m >= 2);
            assert(models_val(cs[n - 2], astk[m - 2])) by {
                assert(cs[cs.len() - astk.len() + (m - 2)] == cs[n - 2]);
            }
            lemma_models_push(cs, astk, cs[n - 2], astk[m - 2]);
        }
        SpecPrim::Add | SpecPrim::Sub | SpecPrim::Mul | SpecPrim::Xor
        | SpecPrim::Eq | SpecPrim::Lt => {
            lemma_prim_arith_totalish(cs, astk, p, rest, astk2);
        }
        SpecPrim::Div | SpecPrim::Mod => {
            lemma_prim_divmod(cs, astk, p, rest, astk2);
        }
        _ => {
            // is_sl_prim(p) rules out all other primitives.
            assert(false);
        }
    }
}

// ---- supporting lemmas for the per-primitive proof ----

/// Dropping the top of both stacks preserves refinement (Drop).
proof fn lemma_models_drop(cs: Seq<SpecValue>, astk: Seq<AbsVal>)
    requires
        models_stack(cs, astk),
        astk.len() >= 1,
    ensures
        models_stack(cs.subrange(0, cs.len() - 1), astk.subrange(0, astk.len() - 1)),
{
    let n = cs.len() as int;
    let m = astk.len() as int;
    let cs2 = cs.subrange(0, n - 1);
    let astk2 = astk.subrange(0, m - 1);
    assert(cs2.len() == n - 1);
    assert(astk2.len() == m - 1);
    assert forall|j: int| 0 <= j < astk2.len() implies
        models_val(cs2[cs2.len() - astk2.len() + j], astk2[j])
    by {
        assert(cs2.len() - astk2.len() + j == n - astk.len() + j);
        assert(cs2[cs2.len() - astk2.len() + j] == cs[cs.len() - astk.len() + j]);
        assert(astk2[j] == astk[j]);
    }
}

/// Swap of the top two cells preserves refinement.
proof fn lemma_prim_swap(cs: Seq<SpecValue>, astk: Seq<AbsVal>)
    requires
        models_stack(cs, astk),
        astk.len() >= 2,
    ensures
        models_stack(
            cs.subrange(0, cs.len() - 2).push(cs[cs.len() as int - 1]).push(cs[cs.len() as int - 2]),
            astk.subrange(0, astk.len() - 2).push(astk[astk.len() as int - 1]).push(astk[astk.len() as int - 2]),
        ),
{
    let n = cs.len() as int;
    let m = astk.len() as int;
    // top cell of cs refines top cell of astk, ditto second.
    assert(cs[cs.len() - astk.len() + (m - 1)] == cs[n - 1]);
    assert(cs[cs.len() - astk.len() + (m - 2)] == cs[n - 2]);
    assert(models_val(cs[n - 1], astk[m - 1]));
    assert(models_val(cs[n - 2], astk[m - 2]));
    lemma_models_subrange(cs, astk, 2);
    lemma_models_push(cs.subrange(0, n - 2), astk.subrange(0, m - 2), cs[n - 1], astk[m - 1]);
    lemma_models_push(
        cs.subrange(0, n - 2).push(cs[n - 1]),
        astk.subrange(0, m - 2).push(astk[m - 1]),
        cs[n - 2], astk[m - 2],
    );
}

/// Rot ( a b c -- b c a ) preserves refinement.
proof fn lemma_prim_rot(cs: Seq<SpecValue>, astk: Seq<AbsVal>)
    requires
        models_stack(cs, astk),
        astk.len() >= 3,
    ensures
        models_stack(
            cs.subrange(0, cs.len() - 3)
                .push(cs[cs.len() as int - 2]).push(cs[cs.len() as int - 1]).push(cs[cs.len() as int - 3]),
            astk.subrange(0, astk.len() - 3)
                .push(astk[astk.len() as int - 2]).push(astk[astk.len() as int - 1]).push(astk[astk.len() as int - 3]),
        ),
{
    let n = cs.len() as int;
    let m = astk.len() as int;
    assert(cs[cs.len() - astk.len() + (m - 1)] == cs[n - 1]);
    assert(cs[cs.len() - astk.len() + (m - 2)] == cs[n - 2]);
    assert(cs[cs.len() - astk.len() + (m - 3)] == cs[n - 3]);
    assert(models_val(cs[n - 1], astk[m - 1]));
    assert(models_val(cs[n - 2], astk[m - 2]));
    assert(models_val(cs[n - 3], astk[m - 3]));
    lemma_models_subrange(cs, astk, 3);
    lemma_models_push(cs.subrange(0, n - 3), astk.subrange(0, m - 3), cs[n - 2], astk[m - 2]);
    lemma_models_push(
        cs.subrange(0, n - 3).push(cs[n - 2]),
        astk.subrange(0, m - 3).push(astk[m - 2]),
        cs[n - 1], astk[m - 1],
    );
    lemma_models_push(
        cs.subrange(0, n - 3).push(cs[n - 2]).push(cs[n - 1]),
        astk.subrange(0, m - 3).push(astk[m - 2]).push(astk[m - 1]),
        cs[n - 3], astk[m - 3],
    );
}

/// Dropping the top `k` cells of both stacks preserves refinement of the base.
proof fn lemma_models_subrange(cs: Seq<SpecValue>, astk: Seq<AbsVal>, k: int)
    requires
        models_stack(cs, astk),
        0 <= k <= astk.len(),
    ensures
        models_stack(cs.subrange(0, cs.len() - k), astk.subrange(0, astk.len() - k)),
{
    let n = cs.len() as int;
    let m = astk.len() as int;
    let cs2 = cs.subrange(0, n - k);
    let astk2 = astk.subrange(0, m - k);
    assert forall|j: int| 0 <= j < astk2.len() implies
        models_val(cs2[cs2.len() - astk2.len() + j], astk2[j])
    by {
        assert(cs2[cs2.len() - astk2.len() + j] == cs[cs.len() - astk.len() + j]);
        assert(astk2[j] == astk[j]);
    }
}

/// Arith/cmp that can only ever fault Overflow (Add/Sub/Mul) or never fault
/// (Xor/Eq/Lt): abstract acceptance forces both operands to be concrete Ints,
/// so no Underflow/TypeMismatch is reachable; a `Next` result pushes an Int (AInt).
proof fn lemma_prim_arith_totalish(
    cs: Seq<SpecValue>, astk: Seq<AbsVal>, p: SpecPrim, rest: Seq<SpecWord>, astk2: Seq<AbsVal>,
)
    requires
        models_stack(cs, astk),
        is_arith6(p),
        abs_step_prim(astk, p) == Some(astk2),
    ensures
        match spec_step_prim(cs, p, rest) {
            SpecStep::Next(s2) => models_stack(s2.stack, astk2) && s2.cont == rest,
            SpecStep::Fault(e) => e == Error::Overflow || e == Error::DivByZero,
            _ => false,
        },
{
    let n = cs.len() as int;
    let m = astk.len() as int;
    // abs accepted arith => m >= 2 and top two are AInt.
    assert(m >= 2);
    assert(absv_is_int(astk[m - 1]) && absv_is_int(astk[m - 2]));
    // therefore the two concrete top cells are Ints.
    assert(cs[cs.len() - astk.len() + (m - 1)] == cs[n - 1]);
    assert(cs[cs.len() - astk.len() + (m - 2)] == cs[n - 2]);
    assert(cs[n - 1] is Int);
    assert(cs[n - 2] is Int);
    // astk2 = astk.subrange(0, m-2).push(AInt). The Next result cell is Int(..).
    lemma_models_subrange(cs, astk, 2);
    let base_cs = cs.subrange(0, n - 2);
    let base_astk = astk.subrange(0, m - 2);
    // In the Next arms the pushed value is SpecValue::Int(r) for some r; refine it.
    assert forall|r: int| models_stack(base_cs.push(SpecValue::Int(r)), base_astk.push(AbsVal::AInt))
    by {
        lemma_models_push(base_cs, base_astk, SpecValue::Int(r), AbsVal::AInt);
    }
    // spec_step_prim on Add/Sub/Mul routes through spec_arith; Xor/Eq/Lt are inline.
    // In all cases with two Ints: Next(Int(..)) or (for Add/Sub/Mul) Fault(Overflow).
    // The base_cs.push(Int(r)) refinement above closes the Next arm.
    assert(base_cs == cs.subrange(0, n - 2));
    assert(base_astk == astk.subrange(0, m - 2));
}

/// Div/Mod: two concrete Ints, so no Underflow/TypeMismatch; may fault DivByZero
/// (b == 0) or Overflow (i64::MIN / -1); otherwise Next pushes an Int (AInt).
proof fn lemma_prim_divmod(
    cs: Seq<SpecValue>, astk: Seq<AbsVal>, p: SpecPrim, rest: Seq<SpecWord>, astk2: Seq<AbsVal>,
)
    requires
        models_stack(cs, astk),
        is_divmod2(p),
        abs_step_prim(astk, p) == Some(astk2),
    ensures
        match spec_step_prim(cs, p, rest) {
            SpecStep::Next(s2) => models_stack(s2.stack, astk2) && s2.cont == rest,
            SpecStep::Fault(e) => e == Error::Overflow || e == Error::DivByZero,
            _ => false,
        },
{
    let n = cs.len() as int;
    let m = astk.len() as int;
    assert(m >= 2);
    assert(absv_is_int(astk[m - 1]) && absv_is_int(astk[m - 2]));
    assert(cs[cs.len() - astk.len() + (m - 1)] == cs[n - 1]);
    assert(cs[cs.len() - astk.len() + (m - 2)] == cs[n - 2]);
    assert(cs[n - 1] is Int);
    assert(cs[n - 2] is Int);
    lemma_models_subrange(cs, astk, 2);
    let base_cs = cs.subrange(0, n - 2);
    let base_astk = astk.subrange(0, m - 2);
    assert forall|r: int| models_stack(base_cs.push(SpecValue::Int(r)), base_astk.push(AbsVal::AInt))
    by {
        lemma_models_push(base_cs, base_astk, SpecValue::Int(r), AbsVal::AInt);
    }
}

// ============================================================
// 4. T-Static (straight-line, If-free fragment) — FULLY PROVEN
// ============================================================
//
// The preservation invariant, inducted over `spec_stepn`. From any state whose
// stack refines `astk` and whose continuation is an accepted straight-line
// program, `k` steps never reach Underflow/TypeMismatch, and any Halt refines the
// checker's `post`.

pub proof fn lemma_sl_invariant(s: SpecState, astk: Seq<AbsVal>, k: nat)
    requires
        is_straightline(s.cont),
        models_stack(s.stack, astk),
        abs_run(astk, s.cont) is Some,
    ensures
        match spec_stepn(s, k) {
            SpecStep::Fault(e) => e == Error::Overflow || e == Error::DivByZero,
            SpecStep::Halt(fin) => models_stack(fin, abs_run(astk, s.cont)->Some_0),
            SpecStep::Next(_) => true,
            SpecStep::Invoke(..) => false,
        },
    decreases k,
{
    if k == 0 {
        // spec_stepn(s, 0) == Next(s): trivially satisfied.
    } else {
        if s.cont.len() == 0 {
            // spec_step(s) == Halt(s.stack); abs_run(astk, []) == Some(astk).
            assert(spec_step(s) == SpecStep::Halt(s.stack));
            assert(abs_run(astk, s.cont) == Some(astk));
            assert(spec_stepn(s, k) == SpecStep::Halt(s.stack));
        } else {
            let w = s.cont[0];
            let rest = s.cont.subrange(1, s.cont.len() as int);
            // abs_run accepted the head: unfold one step to expose that
            // abs_step_word(astk, w) is Some and abs_run continues from its result.
            assert(abs_run(astk, s.cont) == match abs_step_word(astk, w) {
                Some(astk2) => abs_run(astk2, rest),
                None => None::<Seq<AbsVal>>,
            });
            assert(abs_step_word(astk, w) is Some);
            // rest is straight-line (suffix of a straight-line seq).
            assert(is_straightline(rest)) by {
                assert forall|i: int| 0 <= i < rest.len() implies is_sl_word(rest[i]) by {
                    assert(rest[i] == s.cont[i + 1]);
                }
            }
            // is_sl_word(w) holds (w == s.cont[0]).
            assert(is_sl_word(w)) by { assert(w == s.cont[0]); }
            match w {
                SpecWord::PushInt(x) => {
                    let s2 = SpecState { stack: s.stack.push(SpecValue::Int(x)), cont: rest };
                    assert(spec_step(s) == SpecStep::Next(s2));
                    lemma_models_push(s.stack, astk, SpecValue::Int(x), AbsVal::AInt);
                    let astk2 = astk.push(AbsVal::AInt);
                    assert(abs_step_word(astk, w) == Some(astk2));
                    lemma_abs_run_step(astk, s.cont, w, rest, astk2);
                    lemma_sl_invariant(s2, astk2, (k - 1) as nat);
                    assert(spec_stepn(s, k) == spec_stepn(s2, (k - 1) as nat));
                }
                SpecWord::PushQuote(q) => {
                    let s2 = SpecState { stack: s.stack.push(SpecValue::Quote(q)), cont: rest };
                    assert(spec_step(s) == SpecStep::Next(s2));
                    assert(models_val(SpecValue::Quote(q), AbsVal::ALit(q)));
                    lemma_models_push(s.stack, astk, SpecValue::Quote(q), AbsVal::ALit(q));
                    let astk2 = astk.push(AbsVal::ALit(q));
                    assert(abs_step_word(astk, w) == Some(astk2));
                    lemma_abs_run_step(astk, s.cont, w, rest, astk2);
                    lemma_sl_invariant(s2, astk2, (k - 1) as nat);
                    assert(spec_stepn(s, k) == spec_stepn(s2, (k - 1) as nat));
                }
                SpecWord::Prim(p) => {
                    assert(is_sl_prim(p));
                    // abs_run accepted => abs_step_prim(astk, p) is Some(astk2).
                    assert(abs_step_word(astk, w) is Some);
                    let astk2 = abs_step_prim(astk, p).unwrap();
                    assert(abs_step_prim(astk, p) == Some(astk2));
                    lemma_prim_step_sound(s.stack, astk, p, rest, astk2);
                    assert(spec_step(s) == spec_step_prim(s.stack, p, rest));
                    match spec_step_prim(s.stack, p, rest) {
                        SpecStep::Next(s2) => {
                            assert(s2.cont == rest);
                            assert(models_stack(s2.stack, astk2));
                            lemma_abs_run_step(astk, s.cont, w, rest, astk2);
                            lemma_sl_invariant(s2, astk2, (k - 1) as nat);
                            assert(spec_stepn(s, k) == spec_stepn(s2, (k - 1) as nat));
                        }
                        SpecStep::Fault(e) => {
                            assert(e == Error::Overflow || e == Error::DivByZero);
                            assert(spec_stepn(s, k) == SpecStep::Fault(e));
                        }
                        _ => {
                            assert(false);
                        }
                    }
                }
                SpecWord::Call(_) => {
                    // is_sl_word(w) is false for Call — contradiction.
                    assert(false);
                }
            }
        }
    }
}

/// Unfolds one head-step of `abs_run`: if the head word abstract-steps to `astk2`,
/// then `abs_run(astk, cont) == abs_run(astk2, rest)`.
proof fn lemma_abs_run_step(
    astk: Seq<AbsVal>, cont: Seq<SpecWord>, w: SpecWord, rest: Seq<SpecWord>, astk2: Seq<AbsVal>,
)
    requires
        cont.len() > 0,
        w == cont[0],
        rest == cont.subrange(1, cont.len() as int),
        abs_step_word(astk, w) == Some(astk2),
    ensures
        abs_run(astk, cont) == abs_run(astk2, rest),
{
    // abs_run unfolds: match abs_step_word(astk, cont[0]) { Some(a2) => abs_run(a2, rest) }.
    assert(abs_run(astk, cont) == abs_run(astk2, rest));
}

/// **T-Static (straight-line fragment).** If the milestone-1 checker accepts an
/// If-free program `p` with post-shape `post`, then from ANY initial stack `rho`,
/// running `spec_step` for any number of steps `k`:
///   * never reaches `Fault(Underflow)` or `Fault(TypeMismatch)` (only Overflow /
///     DivByZero are possible faults), and
///   * if it `Halt`s, the final stack refines `post` (i.e. satisfies `eff.post`).
/// `rho` is the polymorphic base ρ — arbitrary, so this is the `pre = []` slice of
/// the row-polymorphic effect `∀ρ. ρ → ρ ++ post`.
pub proof fn thm_static_straightline(p: Seq<SpecWord>, rho: Seq<SpecValue>, k: nat)
    requires
        is_straightline(p),
        check_static_m1(p) is Some,
    ensures
        ({
            let s0 = SpecState { stack: rho, cont: p };
            &&& !(spec_stepn(s0, k) matches SpecStep::Fault(e)
                    && (e == Error::Underflow || e == Error::TypeMismatch))
            &&& (spec_stepn(s0, k) matches SpecStep::Halt(fin)
                    ==> models_stack(fin, check_static_m1(p)->Some_0))
        }),
{
    let s0 = SpecState { stack: rho, cont: p };
    // empty abstract stack is refined by ANY concrete stack (astk.len() == 0).
    assert(models_stack(rho, Seq::<AbsVal>::empty()));
    assert(abs_run(Seq::<AbsVal>::empty(), p) is Some);
    assert(check_static_m1(p) == abs_run(Seq::<AbsVal>::empty(), s0.cont));
    lemma_sl_invariant(s0, Seq::<AbsVal>::empty(), k);
}

/// **T-Progress (fragment).** A single reachable straight-line state under the
/// invariant is never *stuck*: it either halts, steps, or is an arithmetic
/// (Overflow/DivByZero) fault — never Underflow/TypeMismatch. This is the k = 1
/// specialization, stated as the progress judgment.
pub proof fn thm_progress(s: SpecState, astk: Seq<AbsVal>)
    requires
        is_straightline(s.cont),
        models_stack(s.stack, astk),
        abs_run(astk, s.cont) is Some,
    ensures
        match spec_step(s) {
            SpecStep::Fault(e) => e == Error::Overflow || e == Error::DivByZero,
            SpecStep::Invoke(..) => false,
            _ => true,   // Next (progress) or Halt (done)
        },
{
    lemma_sl_invariant(s, astk, 1);
    // spec_stepn(s, 1) == spec_step(s) (one step): unfold the tail 0-step.
    assert(spec_stepn(s, 1) == spec_step(s)) by {
        match spec_step(s) {
            SpecStep::Next(s2) => { assert(spec_stepn(s2, 0) == SpecStep::Next(s2)); }
            _ => {}
        }
    }
}

// ============================================================
// 5. T-Branch — branch-stack compatibility (abstract, FULLY PROVEN)
// ============================================================
//
// The design's If rule requires the two Lit branch effects to have EQUAL net
// height delta and JOINABLE per-cell outputs; that is exactly what makes the
// post-If shape well-defined independent of the runtime flag. Here we (a) prove
// the concrete `If` step never faults Underflow/TypeMismatch when the top three
// cells are Int, Quote, Quote (progress at the branch point), and (b) prove the
// abstract join is well-defined and both branch posts refine it.

/// Per-cell abstract join: `Int join Int = Int`; `Lit join Lit = Lit` only when the
/// bodies coincide (milestone-1 keeps `Lit` precise — an unequal-body join would be
/// `OpaqueQuote`, which is out of the milestone-1 lattice, hence NOT joinable here).
pub open spec fn join_cell(a: AbsVal, b: AbsVal) -> Option<AbsVal> {
    match (a, b) {
        (AbsVal::AInt, AbsVal::AInt) => Some(AbsVal::AInt),
        (AbsVal::ALit(x), AbsVal::ALit(y)) => if x == y { Some(AbsVal::ALit(x)) } else { None },
        _ => None,
    }
}

/// Two abstract post-stacks are joinable iff equal height (equal net delta from a
/// shared pre) and every cell joins.
pub open spec fn joinable(pt: Seq<AbsVal>, pf: Seq<AbsVal>) -> bool {
    &&& pt.len() == pf.len()
    &&& forall|j: int| 0 <= j < pt.len() ==> (#[trigger] join_cell(pt[j], pf[j])) is Some
}

pub open spec fn join_stacks(pt: Seq<AbsVal>, pf: Seq<AbsVal>) -> Seq<AbsVal>
    recommends joinable(pt, pf),
{
    Seq::new(pt.len(), |j: int| join_cell(pt[j], pf[j]).unwrap())
}

/// **T-Branch join soundness.** If two branch posts are joinable, then ANY concrete
/// stack that refines EITHER branch post also refines the join. Hence whichever
/// branch the runtime flag selects, the post-If stack refines the single joined
/// shape — the post-If shape is well-defined independent of the flag.
pub proof fn lemma_join_sound(cs: Seq<SpecValue>, pt: Seq<AbsVal>, pf: Seq<AbsVal>)
    requires
        joinable(pt, pf),
        models_stack(cs, pt) || models_stack(cs, pf),
    ensures
        models_stack(cs, join_stacks(pt, pf)),
{
    let j = join_stacks(pt, pf);
    assert(j.len() == pt.len());
    assert forall|i: int| 0 <= i < j.len() implies
        models_val(cs[cs.len() - j.len() + i], j[i])
    by {
        assert(j[i] == join_cell(pt[i], pf[i]).unwrap());
        assert(join_cell(pt[i], pf[i]) is Some);
        // In both AInt/AInt and ALit(x)/ALit(x) cases, the join equals the branch
        // cell that cs already refines.
        if models_stack(cs, pt) {
            assert(models_val(cs[cs.len() - pt.len() + i], pt[i]));
            assert(cs.len() - j.len() + i == cs.len() - pt.len() + i);
            // join_cell(pt[i],pf[i]) is Some => cells share kind => join refines same.
            match (pt[i], pf[i]) {
                (AbsVal::AInt, AbsVal::AInt) => { assert(j[i] == AbsVal::AInt); }
                (AbsVal::ALit(x), AbsVal::ALit(y)) => { assert(x == y); assert(j[i] == pt[i]); }
                _ => { assert(false); }
            }
        } else {
            assert(models_val(cs[cs.len() - pf.len() + i], pf[i]));
            assert(cs.len() - j.len() + i == cs.len() - pf.len() + i);
            match (pt[i], pf[i]) {
                (AbsVal::AInt, AbsVal::AInt) => { assert(j[i] == AbsVal::AInt); }
                (AbsVal::ALit(x), AbsVal::ALit(y)) => { assert(x == y); assert(j[i] == pf[i]); }
                _ => { assert(false); }
            }
        }
    }
}

/// **T-Branch progress.** When the top three cells are `Int, Quote, Quote`, the
/// `If` step never faults: it selects one branch and splices it into the
/// continuation. (The `Int` condition is the discriminant; two quote branches make
/// both success arms well-typed — no Underflow/TypeMismatch.)
pub proof fn thm_branch_progress(cs: Seq<SpecValue>, rest: Seq<SpecWord>)
    requires
        cs.len() >= 3,
        cs[cs.len() as int - 3] is Int,
        cs[cs.len() as int - 2] is Quote,
        cs[cs.len() as int - 1] is Quote,
    ensures
        ({
            let n = cs.len() as int;
            let c = cs[n - 3]->Int_0;
            let t = cs[n - 2]->Quote_0;
            let f = cs[n - 1]->Quote_0;
            spec_step_prim(cs, SpecPrim::If, rest) == SpecStep::Next(SpecState {
                stack: cs.subrange(0, n - 3),
                cont: (if c != 0 { t } else { f }) + rest,
            })
        }),
{
    // Direct from the `If` arm of spec_step_prim with all three cells well-typed.
}

// ============================================================
// 6. T-Static WITH If — FULLY PROVEN (milestone-2 Part A)
// ============================================================
//
// The full T-Static over programs CONTAINING `If`. The abstract checker
// (`check_m1` below) summarizes an `If` with the joined branch effect and
// continues; `spec_step` INLINES the selected branch body into the continuation
// and executes it step-by-step. Tying those together is a non-lockstep induction:
// one abstract `If` step corresponds to a *stutter* of concrete steps (the whole
// branch body, of arbitrary length). This is now CLOSED (no `assume`): the key
// realization is that in the milestone-1 lattice `joinable(pt, pf) ==> pt == pf`
// (`lemma_joinable_eq`), so the concrete run through ONE branch already lands in the
// join's shape — no monotonicity argument needed. The splice is handled at the
// checker level by `lemma_check_compose` (the analogue of p5's `lemma_stepn_compose`),
// which lets the `If` case treat the inlined `branch + rest` as a single program and
// re-enter the same-`depth` invariant at `k - 1`. See `lemma_check_invariant`.

/// Depth-fuelled If-aware checker (design: "bounds inline depth"). `depth` bounds
/// nested-If recursion so the spec fn is well-founded; real programs nest finitely.
pub open spec fn check_m1(astk: Seq<AbsVal>, p: Seq<SpecWord>, depth: nat) -> Option<Seq<AbsVal>>
    decreases depth, p.len(),
{
    if p.len() == 0 {
        Some(astk)
    } else {
        let w = p[0];
        let rest = p.subrange(1, p.len() as int);
        match w {
            SpecWord::PushInt(_) => check_m1(astk.push(AbsVal::AInt), rest, depth),
            SpecWord::PushQuote(q) => check_m1(astk.push(AbsVal::ALit(q)), rest, depth),
            SpecWord::Prim(SpecPrim::If) => {
                let m = astk.len() as int;
                if m < 3 {
                    None
                } else if depth == 0 {
                    None
                } else {
                    match (astk[m - 3], astk[m - 2], astk[m - 1]) {
                        (AbsVal::AInt, AbsVal::ALit(t), AbsVal::ALit(f)) => {
                            let base = astk.subrange(0, m - 3);
                            match (check_m1(base, t, (depth - 1) as nat),
                                   check_m1(base, f, (depth - 1) as nat)) {
                                (Some(pt), Some(pf)) =>
                                    if joinable(pt, pf) {
                                        check_m1(join_stacks(pt, pf), rest, depth)
                                    } else { None },
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }
            }
            // ---- Times `.` : n [Q] . — the body Q must be an abstract fixpoint
            // (net-zero height + per-cell type-stable, design §2), i.e. it maps the
            // base abstract stack back to itself. Then `n [Q] .` pops both and leaves
            // the base untouched. `depth` bounds the checker's fixpoint re-check of Q.
            SpecWord::Prim(SpecPrim::Times) => {
                let m = astk.len() as int;
                if m < 2 {
                    None
                } else if depth == 0 {
                    None
                } else {
                    match (astk[m - 2], astk[m - 1]) {
                        (AbsVal::AInt, AbsVal::ALit(q)) => {
                            let base = astk.subrange(0, m - 2);
                            if check_m1(base, q, (depth - 1) as nat) == Some(base) {
                                check_m1(base, rest, depth)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                }
            }
            SpecWord::Prim(p2) =>
                match abs_step_prim(astk, p2) {
                    Some(astk2) => check_m1(astk2, rest, depth),
                    None => None,
                },
            SpecWord::Call(_) => None,
        }
    }
}

// ------------------------------------------------------------
// Part-A machinery: the three reusable lemmas that DISCHARGE the If-inlining
// bridge (milestone-2, Part A). See `lemma_check_invariant` for the assembly.
// ------------------------------------------------------------

/// `abs_step_prim` only ever ACCEPTS a straight-line primitive: its `Some` arms are
/// exactly the shuffles + arith/cmp, i.e. `is_sl_prim`. (`If` and all excluded prims
/// route to `None`.) Lets the `Prim` arm of `check_m1` reuse `lemma_prim_step_sound`.
proof fn lemma_abs_step_prim_sl(astk: Seq<AbsVal>, p: SpecPrim)
    requires
        abs_step_prim(astk, p) is Some,
    ensures
        is_sl_prim(p),
{
    match p {
        SpecPrim::Dup | SpecPrim::Drop | SpecPrim::Swap | SpecPrim::Rot | SpecPrim::Over
        | SpecPrim::Add | SpecPrim::Sub | SpecPrim::Mul | SpecPrim::Div | SpecPrim::Mod
        | SpecPrim::Xor | SpecPrim::Eq | SpecPrim::Lt => {}
        _ => { assert(abs_step_prim(astk, p) is None); }
    }
}

/// **Bottom-frame invariance of `abs_step_prim`.** Every straight-line primitive acts
/// only on the TOP of the stack (`astk[n-1]`, `subrange(0, n-k)`, `push`), so
/// prepending a frame `fr` at the BOTTOM shifts all indices uniformly and commutes:
/// `abs_step_prim(fr + astk, p) == fr + abs_step_prim(astk, p)`. The workhorse for the
/// recursion combinators, whose runtime counters/tail pile up below the accumulator.
proof fn lemma_abs_step_prim_frame(fr: Seq<AbsVal>, astk: Seq<AbsVal>, p: SpecPrim)
    requires
        abs_step_prim(astk, p) is Some,
    ensures
        abs_step_prim(fr + astk, p)
            == Some(fr + abs_step_prim(astk, p)->Some_0),
{
    let m = astk.len() as int;
    let a2 = astk.len() as int + fr.len() as int;
    let big = fr + astk;
    assert(big.len() == fr.len() + astk.len());
    // Top-relative index correspondence: big[fr.len() + j] == astk[j].
    assert forall|j: int| 0 <= j < m implies big[fr.len() + j] == astk[j] by {}
    lemma_abs_step_prim_sl(astk, p);
    match p {
        SpecPrim::Dup => {
            assert(m >= 1);
            assert(big[a2 - 1] == astk[m - 1]);
            assert(big.push(big[a2 - 1]) =~= fr + astk.push(astk[m - 1]));
        }
        SpecPrim::Drop => {
            assert(m >= 1);
            assert(big.subrange(0, a2 - 1) =~= fr + astk.subrange(0, m - 1));
        }
        SpecPrim::Swap => {
            assert(m >= 2);
            assert(big[a2 - 1] == astk[m - 1]);
            assert(big[a2 - 2] == astk[m - 2]);
            assert(big.subrange(0, a2 - 2).push(big[a2 - 1]).push(big[a2 - 2])
                =~= fr + astk.subrange(0, m - 2).push(astk[m - 1]).push(astk[m - 2]));
        }
        SpecPrim::Rot => {
            assert(m >= 3);
            assert(big[a2 - 1] == astk[m - 1]);
            assert(big[a2 - 2] == astk[m - 2]);
            assert(big[a2 - 3] == astk[m - 3]);
            assert(big.subrange(0, a2 - 3).push(big[a2 - 2]).push(big[a2 - 1]).push(big[a2 - 3])
                =~= fr + astk.subrange(0, m - 3).push(astk[m - 2]).push(astk[m - 1]).push(astk[m - 3]));
        }
        SpecPrim::Over => {
            assert(m >= 2);
            assert(big[a2 - 2] == astk[m - 2]);
            assert(big.push(big[a2 - 2]) =~= fr + astk.push(astk[m - 2]));
        }
        SpecPrim::Add | SpecPrim::Sub | SpecPrim::Mul | SpecPrim::Div | SpecPrim::Mod
        | SpecPrim::Xor | SpecPrim::Eq | SpecPrim::Lt => {
            assert(m >= 2);
            assert(absv_is_int(astk[m - 1]) && absv_is_int(astk[m - 2]));
            assert(big[a2 - 1] == astk[m - 1]);
            assert(big[a2 - 2] == astk[m - 2]);
            assert(absv_is_int(big[a2 - 1]) && absv_is_int(big[a2 - 2]));
            assert(big.subrange(0, a2 - 2).push(AbsVal::AInt)
                =~= fr + astk.subrange(0, m - 2).push(AbsVal::AInt));
        }
        _ => { assert(false); }
    }
}

/// `join_cell(x, x)` is always `Some` (reflexivity): equal cells join to themselves.
proof fn lemma_join_cell_refl(x: AbsVal)
    ensures
        join_cell(x, x) is Some,
        join_cell(x, x)->Some_0 == x,
{
    match x {
        AbsVal::AInt => {}
        AbsVal::ALit(_) => {}
    }
}

/// **Bottom-frame invariance of `check_m1`.** Lifts `lemma_abs_step_prim_frame` (and
/// the analogous top-relative reasoning for `If`/`Times`) through the whole checker:
/// running the checker under an extra bottom frame `fr` appends `fr` to the post.
/// This is what makes the recursion-combinator bodies analyzable on a MINIMAL stack
/// (empty for the primrec initializer, `[counter]++acc` for the combine) and then
/// re-instated on the real running stack.
pub proof fn lemma_check_frame(fr: Seq<AbsVal>, astk: Seq<AbsVal>, p: Seq<SpecWord>, depth: nat)
    requires
        check_m1(astk, p, depth) is Some,
    ensures
        check_m1(fr + astk, p, depth)
            == Some(fr + check_m1(astk, p, depth)->Some_0),
    decreases depth, p.len(),
{
    let big = fr + astk;
    if p.len() == 0 {
        assert(check_m1(astk, p, depth) == Some(astk));
        assert(check_m1(big, p, depth) == Some(big));
    } else {
        let w = p[0];
        let rest = p.subrange(1, p.len() as int);
        match w {
            SpecWord::PushInt(_) => {
                assert(big.push(AbsVal::AInt) =~= fr + astk.push(AbsVal::AInt));
                lemma_check_frame(fr, astk.push(AbsVal::AInt), rest, depth);
            }
            SpecWord::PushQuote(q) => {
                assert(big.push(AbsVal::ALit(q)) =~= fr + astk.push(AbsVal::ALit(q)));
                lemma_check_frame(fr, astk.push(AbsVal::ALit(q)), rest, depth);
            }
            SpecWord::Prim(SpecPrim::If) => {
                let m = astk.len() as int;
                let mb = big.len() as int;
                assert(m >= 3);
                assert(depth >= 1);
                assert(big[mb - 3] == astk[m - 3]);
                assert(big[mb - 2] == astk[m - 2]);
                assert(big[mb - 1] == astk[m - 1]);
                match (astk[m - 3], astk[m - 2], astk[m - 1]) {
                    (AbsVal::AInt, AbsVal::ALit(t), AbsVal::ALit(f)) => {
                        let base = astk.subrange(0, m - 3);
                        assert(big.subrange(0, mb - 3) =~= fr + base);
                        let pt = check_m1(base, t, (depth - 1) as nat)->Some_0;
                        let pf = check_m1(base, f, (depth - 1) as nat)->Some_0;
                        assert(joinable(pt, pf));
                        lemma_joinable_eq(pt, pf);
                        lemma_check_frame(fr, base, t, (depth - 1) as nat);
                        lemma_check_frame(fr, base, f, (depth - 1) as nat);
                        // framed branch posts are fr+pt and fr+pf, and pt == pf.
                        assert(check_m1(fr + base, t, (depth - 1) as nat) == Some(fr + pt));
                        assert(check_m1(fr + base, f, (depth - 1) as nat) == Some(fr + pf));
                        assert(fr + pt =~= fr + pf);
                        assert(joinable(fr + pt, fr + pf)) by {
                            assert forall|j: int| 0 <= j < (fr + pt).len() implies
                                (#[trigger] join_cell((fr + pt)[j], (fr + pf)[j])) is Some by {
                                assert((fr + pt)[j] == (fr + pf)[j]);
                                lemma_join_cell_refl((fr + pt)[j]);
                            }
                        }
                        assert(join_stacks(fr + pt, fr + pf) =~= fr + join_stacks(pt, pf)) by {
                            assert forall|i: int| 0 <= i < (fr + pt).len() implies
                                join_stacks(fr + pt, fr + pf)[i] == (fr + join_stacks(pt, pf))[i] by {
                                lemma_join_cell_refl((fr + pt)[i]);
                                lemma_joinable_eq(pt, pf);
                            }
                        }
                        lemma_check_frame(fr, join_stacks(pt, pf), rest, depth);
                    }
                    _ => { assert(false); }
                }
            }
            SpecWord::Prim(SpecPrim::Times) => {
                let m = astk.len() as int;
                let mb = big.len() as int;
                assert(m >= 2);
                assert(depth >= 1);
                assert(big[mb - 2] == astk[m - 2]);
                assert(big[mb - 1] == astk[m - 1]);
                match (astk[m - 2], astk[m - 1]) {
                    (AbsVal::AInt, AbsVal::ALit(q)) => {
                        let base = astk.subrange(0, m - 2);
                        assert(big.subrange(0, mb - 2) =~= fr + base);
                        assert(check_m1(base, q, (depth - 1) as nat) == Some(base));
                        lemma_check_frame(fr, base, q, (depth - 1) as nat);
                        assert(check_m1(fr + base, q, (depth - 1) as nat) == Some(fr + base));
                        lemma_check_frame(fr, base, rest, depth);
                    }
                    _ => { assert(false); }
                }
            }
            SpecWord::Prim(p2) => {
                match abs_step_prim(astk, p2) {
                    Some(astk2) => {
                        lemma_abs_step_prim_frame(fr, astk, p2);
                        assert(abs_step_prim(big, p2) == Some(fr + astk2));
                        lemma_check_frame(fr, astk2, rest, depth);
                    }
                    None => { assert(false); }
                }
            }
            SpecWord::Call(_) => { assert(false); }
        }
    }
}

/// In the milestone-1 lattice, `join_cell` is `Some` ONLY on equal cells
/// (`AInt/AInt` or `ALit(x)/ALit(x)`), so `joinable(pt, pf)` forces `pt == pf`, and
/// the join is that common stack. This is what collapses the two-branch/one-join
/// mismatch in the `If` case (the concrete run goes through ONE branch's post, which
/// is already equal to the join — no monotonicity argument needed).
proof fn lemma_joinable_eq(pt: Seq<AbsVal>, pf: Seq<AbsVal>)
    requires
        joinable(pt, pf),
    ensures
        pt == pf,
        join_stacks(pt, pf) == pt,
{
    assert(pt.len() == pf.len());
    assert forall|j: int| 0 <= j < pt.len() implies pt[j] == pf[j] by {
        assert(join_cell(pt[j], pf[j]) is Some);
        match (pt[j], pf[j]) {
            (AbsVal::AInt, AbsVal::AInt) => {}
            (AbsVal::ALit(x), AbsVal::ALit(y)) => { assert(x == y); }
            _ => { assert(false); }
        }
    }
    assert(pt =~= pf);
    let j = join_stacks(pt, pf);
    assert(j.len() == pt.len());
    assert forall|i: int| 0 <= i < j.len() implies j[i] == pt[i] by {
        assert(j[i] == join_cell(pt[i], pf[i]).unwrap());
        assert(join_cell(pt[i], pf[i]) is Some);
        match (pt[i], pf[i]) {
            (AbsVal::AInt, AbsVal::AInt) => {}
            (AbsVal::ALit(x), AbsVal::ALit(y)) => { assert(x == y); }
            _ => { assert(false); }
        }
    }
    assert(j =~= pt);
}

/// Head-first split of a concatenation: for a nonempty `p1`, `(p1 + p2)[0] == p1[0]`
/// and `(p1 + p2).subrange(1, ..) == p1.subrange(1, ..) + p2`.
proof fn lemma_concat_head(p1: Seq<SpecWord>, p2: Seq<SpecWord>)
    requires
        p1.len() > 0,
    ensures
        (p1 + p2)[0] == p1[0],
        (p1 + p2).subrange(1, (p1 + p2).len() as int)
            =~= p1.subrange(1, p1.len() as int) + p2,
{
}

/// **check_m1 depth-monotonicity.** `depth` is a fuel bound on nested-If inlining;
/// raising it never changes an already-accepted verdict. Needed to lift a branch
/// checked at `depth - 1` up to `depth` before splicing it before `rest`.
proof fn lemma_check_depth_mono(astk: Seq<AbsVal>, p: Seq<SpecWord>, d: nat, d2: nat)
    requires
        d <= d2,
        check_m1(astk, p, d) is Some,
    ensures
        check_m1(astk, p, d2) == check_m1(astk, p, d),
    decreases d, p.len(),
{
    if p.len() == 0 {
    } else {
        let w = p[0];
        let rest = p.subrange(1, p.len() as int);
        match w {
            SpecWord::PushInt(_) => {
                lemma_check_depth_mono(astk.push(AbsVal::AInt), rest, d, d2);
            }
            SpecWord::PushQuote(q) => {
                lemma_check_depth_mono(astk.push(AbsVal::ALit(q)), rest, d, d2);
            }
            SpecWord::Prim(SpecPrim::If) => {
                let m = astk.len() as int;
                assert(m >= 3);
                assert(d >= 1);
                assert(d2 >= 1);
                match (astk[m - 3], astk[m - 2], astk[m - 1]) {
                    (AbsVal::AInt, AbsVal::ALit(t), AbsVal::ALit(f)) => {
                        let base = astk.subrange(0, m - 3);
                        lemma_check_depth_mono(base, t, (d - 1) as nat, (d2 - 1) as nat);
                        lemma_check_depth_mono(base, f, (d - 1) as nat, (d2 - 1) as nat);
                        let pt = check_m1(base, t, (d - 1) as nat)->Some_0;
                        let pf = check_m1(base, f, (d - 1) as nat)->Some_0;
                        assert(joinable(pt, pf));
                        lemma_check_depth_mono(join_stacks(pt, pf), rest, d, d2);
                    }
                    _ => { assert(false); }
                }
            }
            SpecWord::Prim(SpecPrim::Times) => {
                let m = astk.len() as int;
                assert(m >= 2);
                assert(d >= 1);
                assert(d2 >= 1);
                match (astk[m - 2], astk[m - 1]) {
                    (AbsVal::AInt, AbsVal::ALit(q)) => {
                        let base = astk.subrange(0, m - 2);
                        assert(check_m1(base, q, (d - 1) as nat) == Some(base));
                        lemma_check_depth_mono(base, q, (d - 1) as nat, (d2 - 1) as nat);
                        lemma_check_depth_mono(base, rest, d, d2);
                    }
                    _ => { assert(false); }
                }
            }
            SpecWord::Prim(p2) => {
                match abs_step_prim(astk, p2) {
                    Some(astk2) => {
                        lemma_check_depth_mono(astk2, rest, d, d2);
                    }
                    None => { assert(false); }
                }
            }
            SpecWord::Call(_) => { assert(false); }
        }
    }
}

/// **check_m1 sequential composition.** Running the checker over `p1 + p2` at a fixed
/// depth equals running `p1`, then running `p2` from the resulting abstract stack —
/// the checker analogue of p5's `lemma_stepn_compose`. This is the splice lemma that
/// lets the `If` case treat the inlined `branch + rest` as one program.
proof fn lemma_check_compose(astk: Seq<AbsVal>, p1: Seq<SpecWord>, p2: Seq<SpecWord>, depth: nat)
    requires
        check_m1(astk, p1, depth) is Some,
    ensures
        check_m1(astk, p1 + p2, depth)
            == check_m1(check_m1(astk, p1, depth)->Some_0, p2, depth),
    decreases p1.len(),
{
    if p1.len() == 0 {
        assert(p1 + p2 =~= p2);
        assert(check_m1(astk, p1, depth) == Some(astk));
    } else {
        lemma_concat_head(p1, p2);
        let w = p1[0];
        let rest1 = p1.subrange(1, p1.len() as int);
        assert((p1 + p2)[0] == w);
        assert((p1 + p2).subrange(1, (p1 + p2).len() as int) =~= rest1 + p2);
        match w {
            SpecWord::PushInt(_) => {
                lemma_check_compose(astk.push(AbsVal::AInt), rest1, p2, depth);
            }
            SpecWord::PushQuote(q) => {
                lemma_check_compose(astk.push(AbsVal::ALit(q)), rest1, p2, depth);
            }
            SpecWord::Prim(SpecPrim::If) => {
                let m = astk.len() as int;
                assert(m >= 3);
                assert(depth >= 1);
                match (astk[m - 3], astk[m - 2], astk[m - 1]) {
                    (AbsVal::AInt, AbsVal::ALit(t), AbsVal::ALit(f)) => {
                        let base = astk.subrange(0, m - 3);
                        let pt = check_m1(base, t, (depth - 1) as nat)->Some_0;
                        let pf = check_m1(base, f, (depth - 1) as nat)->Some_0;
                        assert(joinable(pt, pf));
                        lemma_check_compose(join_stacks(pt, pf), rest1, p2, depth);
                    }
                    _ => { assert(false); }
                }
            }
            SpecWord::Prim(SpecPrim::Times) => {
                let m = astk.len() as int;
                assert(m >= 2);
                assert(depth >= 1);
                match (astk[m - 2], astk[m - 1]) {
                    (AbsVal::AInt, AbsVal::ALit(q)) => {
                        let base = astk.subrange(0, m - 2);
                        assert(check_m1(base, q, (depth - 1) as nat) == Some(base));
                        lemma_check_compose(base, rest1, p2, depth);
                    }
                    _ => { assert(false); }
                }
            }
            SpecWord::Prim(p2p) => {
                match abs_step_prim(astk, p2p) {
                    Some(astk2) => {
                        lemma_check_compose(astk2, rest1, p2, depth);
                    }
                    None => { assert(false); }
                }
            }
            SpecWord::Call(_) => { assert(false); }
        }
    }
}

/// **Times splice re-check.** In the `k > 0` unfolding of `times`, `spec_step`
/// splices `q + [PushInt(k-1), PushQuote(q), Times] + rest` into the continuation.
/// Given the fixpoint condition `check_m1(base, q, depth-1) == Some(base)`, this
/// spliced program re-checks to the SAME post as `rest` alone — so the concrete-step
/// induction (`lemma_check_invariant`) absorbs each `times` unfolding for free.
proof fn lemma_times_splice(
    base: Seq<AbsVal>, q: Seq<SpecWord>, kcount: int, mid: Seq<SpecWord>,
    rest: Seq<SpecWord>, depth: nat,
)
    requires
        depth >= 1,
        check_m1(base, q, (depth - 1) as nat) == Some(base),
        mid == seq![
            SpecWord::PushInt(kcount - 1),
            SpecWord::PushQuote(q),
            SpecWord::Prim(SpecPrim::Times)
        ],
    ensures
        check_m1(base, (q + mid) + rest, depth) == check_m1(base, rest, depth),
{
    // Lift the fixpoint check from depth-1 to depth so `q` composes at `depth`.
    lemma_check_depth_mono(base, q, (depth - 1) as nat, depth);
    assert(check_m1(base, q, depth) == Some(base));
    // Re-associate and split off `q` via the composition lemma.
    assert((q + mid) + rest =~= q + (mid + rest));
    lemma_check_compose(base, q, mid + rest, depth);
    assert(check_m1(base, q + (mid + rest), depth) == check_m1(base, mid + rest, depth));
    // Evaluate the 3-word `mid` prefix: PushInt; PushQuote(q); Times(fixpoint) -> rest.
    let astk1 = base.push(AbsVal::AInt);
    let astk2 = astk1.push(AbsVal::ALit(q));
    lemma_concat_head(mid, rest);
    assert((mid + rest)[0] == SpecWord::PushInt(kcount - 1));
    let tail1 = (mid + rest).subrange(1, (mid + rest).len() as int);
    assert(tail1 =~= seq![SpecWord::PushQuote(q), SpecWord::Prim(SpecPrim::Times)] + rest);
    assert(check_m1(base, mid + rest, depth) == check_m1(astk1, tail1, depth));
    assert(tail1[0] == SpecWord::PushQuote(q));
    let tail2 = tail1.subrange(1, tail1.len() as int);
    assert(tail2 =~= seq![SpecWord::Prim(SpecPrim::Times)] + rest);
    assert(check_m1(astk1, tail1, depth) == check_m1(astk2, tail2, depth));
    // Times arm on astk2 = base ++ [AInt, ALit(q)].
    let m2 = astk2.len() as int;
    assert(m2 >= 2);
    assert(astk2[m2 - 2] == AbsVal::AInt);
    assert(astk2[m2 - 1] == AbsVal::ALit(q));
    assert(astk2.subrange(0, m2 - 2) =~= base);
    assert(tail2[0] == SpecWord::Prim(SpecPrim::Times));
    assert(tail2.subrange(1, tail2.len() as int) =~= rest);
    assert(check_m1(astk2, tail2, depth) == check_m1(base, rest, depth));
}

/// **The If-aware preservation invariant (Part A core).** Same shape as
/// `lemma_sl_invariant`, but over `check_m1` (which summarizes each `If` with the
/// joined branch effect). Induction is on the concrete step count `k` ALONE: the
/// `If` case does NOT recurse on branch structure — it splices the selected branch
/// into the continuation and re-enters the invariant on the spliced program at the
/// same `depth`, using `lemma_check_depth_mono` + `lemma_check_compose` to show the
/// spliced `branch + rest` still checks to the SAME post, and `lemma_joinable_eq`
/// to identify the branch's post with the join.
pub proof fn lemma_check_invariant(s: SpecState, astk: Seq<AbsVal>, depth: nat, k: nat)
    requires
        models_stack(s.stack, astk),
        check_m1(astk, s.cont, depth) is Some,
    ensures
        match spec_stepn(s, k) {
            SpecStep::Fault(e) => e == Error::Overflow || e == Error::DivByZero,
            SpecStep::Halt(fin) => models_stack(fin, check_m1(astk, s.cont, depth)->Some_0),
            SpecStep::Next(_) => true,
            SpecStep::Invoke(..) => false,
        },
    decreases k,
{
    if k == 0 {
    } else if s.cont.len() == 0 {
        assert(spec_step(s) == SpecStep::Halt(s.stack));
        assert(check_m1(astk, s.cont, depth) == Some(astk));
        assert(spec_stepn(s, k) == SpecStep::Halt(s.stack));
    } else {
        let w = s.cont[0];
        let rest = s.cont.subrange(1, s.cont.len() as int);
        match w {
            SpecWord::PushInt(x) => {
                let s2 = SpecState { stack: s.stack.push(SpecValue::Int(x)), cont: rest };
                assert(spec_step(s) == SpecStep::Next(s2));
                let astk2 = astk.push(AbsVal::AInt);
                assert(check_m1(astk, s.cont, depth) == check_m1(astk2, rest, depth));
                lemma_models_push(s.stack, astk, SpecValue::Int(x), AbsVal::AInt);
                lemma_check_invariant(s2, astk2, depth, (k - 1) as nat);
                assert(spec_stepn(s, k) == spec_stepn(s2, (k - 1) as nat));
            }
            SpecWord::PushQuote(q) => {
                let s2 = SpecState { stack: s.stack.push(SpecValue::Quote(q)), cont: rest };
                assert(spec_step(s) == SpecStep::Next(s2));
                let astk2 = astk.push(AbsVal::ALit(q));
                assert(check_m1(astk, s.cont, depth) == check_m1(astk2, rest, depth));
                assert(models_val(SpecValue::Quote(q), AbsVal::ALit(q)));
                lemma_models_push(s.stack, astk, SpecValue::Quote(q), AbsVal::ALit(q));
                lemma_check_invariant(s2, astk2, depth, (k - 1) as nat);
                assert(spec_stepn(s, k) == spec_stepn(s2, (k - 1) as nat));
            }
            SpecWord::Prim(SpecPrim::If) => {
                let cs = s.stack;
                let m = astk.len() as int;
                let big_n = cs.len() as int;
                // check_m1 accepted the If: extract the abstract branch shape.
                assert(m >= 3);
                assert(depth >= 1);
                match (astk[m - 3], astk[m - 2], astk[m - 1]) {
                    (AbsVal::AInt, AbsVal::ALit(t), AbsVal::ALit(f)) => {
                        let base = astk.subrange(0, m - 3);
                        let pt = check_m1(base, t, (depth - 1) as nat)->Some_0;
                        let pf = check_m1(base, f, (depth - 1) as nat)->Some_0;
                        assert(check_m1(base, t, (depth - 1) as nat) is Some);
                        assert(check_m1(base, f, (depth - 1) as nat) is Some);
                        assert(joinable(pt, pf));
                        assert(check_m1(astk, s.cont, depth)
                            == check_m1(join_stacks(pt, pf), rest, depth));
                        // Concrete top three refine (AInt, ALit(t), ALit(f)).
                        assert(cs[cs.len() - astk.len() + (m - 3)] == cs[big_n - 3]);
                        assert(cs[cs.len() - astk.len() + (m - 2)] == cs[big_n - 2]);
                        assert(cs[cs.len() - astk.len() + (m - 1)] == cs[big_n - 1]);
                        assert(models_val(cs[big_n - 3], AbsVal::AInt));
                        assert(models_val(cs[big_n - 2], AbsVal::ALit(t)));
                        assert(models_val(cs[big_n - 1], AbsVal::ALit(f)));
                        assert(cs[big_n - 3] is Int);
                        assert(cs[big_n - 2] == SpecValue::Quote(t));
                        assert(cs[big_n - 1] == SpecValue::Quote(f));
                        let c = cs[big_n - 3]->Int_0;
                        let branch = if c != 0 { t } else { f };
                        let pbr = if c != 0 { pt } else { pf };
                        // spec_step splices the selected branch before `rest`.
                        let s2 = SpecState {
                            stack: cs.subrange(0, big_n - 3),
                            cont: branch + rest,
                        };
                        assert(spec_step(s) == SpecStep::Next(s2));
                        // s2.stack refines base.
                        lemma_models_subrange(cs, astk, 3);
                        assert(models_stack(s2.stack, base));
                        // branch checks (at depth-1, hence at depth) to pbr.
                        assert(check_m1(base, branch, (depth - 1) as nat) == Some(pbr));
                        lemma_check_depth_mono(base, branch, (depth - 1) as nat, depth);
                        assert(check_m1(base, branch, depth) == Some(pbr));
                        // splice: branch + rest checks to check_m1(pbr, rest, depth).
                        lemma_check_compose(base, branch, rest, depth);
                        assert(check_m1(base, branch + rest, depth)
                            == check_m1(pbr, rest, depth));
                        // joinable => pt == pf == join, so pbr == join.
                        lemma_joinable_eq(pt, pf);
                        assert(pbr == join_stacks(pt, pf));
                        assert(check_m1(base, s2.cont, depth) == check_m1(astk, s.cont, depth));
                        // re-enter the invariant on the spliced program at the same depth.
                        lemma_check_invariant(s2, base, depth, (k - 1) as nat);
                        assert(spec_stepn(s, k) == spec_stepn(s2, (k - 1) as nat));
                    }
                    _ => { assert(false); }
                }
            }
            SpecWord::Prim(SpecPrim::Times) => {
                let cs = s.stack;
                let m = astk.len() as int;
                let big_n = cs.len() as int;
                assert(m >= 2);
                assert(depth >= 1);
                match (astk[m - 2], astk[m - 1]) {
                    (AbsVal::AInt, AbsVal::ALit(q)) => {
                        let base = astk.subrange(0, m - 2);
                        // acceptance gives the fixpoint condition + the post.
                        assert(check_m1(base, q, (depth - 1) as nat) == Some(base));
                        assert(check_m1(astk, s.cont, depth) == check_m1(base, rest, depth));
                        // concrete top two refine (AInt, ALit(q)).
                        assert(cs[cs.len() - astk.len() + (m - 2)] == cs[big_n - 2]);
                        assert(cs[cs.len() - astk.len() + (m - 1)] == cs[big_n - 1]);
                        assert(models_val(cs[big_n - 2], AbsVal::AInt));
                        assert(models_val(cs[big_n - 1], AbsVal::ALit(q)));
                        assert(cs[big_n - 2] is Int);
                        assert(cs[big_n - 1] == SpecValue::Quote(q));
                        let kcount = cs[big_n - 2]->Int_0;
                        let base_cs = cs.subrange(0, big_n - 2);
                        lemma_models_subrange(cs, astk, 2);
                        assert(models_stack(base_cs, base));
                        if kcount <= 0 {
                            let s2 = SpecState { stack: base_cs, cont: rest };
                            assert(spec_step(s) == SpecStep::Next(s2));
                            lemma_check_invariant(s2, base, depth, (k - 1) as nat);
                            assert(spec_stepn(s, k) == spec_stepn(s2, (k - 1) as nat));
                        } else {
                            let mid = seq![
                                SpecWord::PushInt(kcount - 1),
                                SpecWord::PushQuote(q),
                                SpecWord::Prim(SpecPrim::Times)
                            ];
                            let recur = q + mid;
                            let s2 = SpecState { stack: base_cs, cont: recur + rest };
                            assert(spec_step(s) == SpecStep::Next(s2));
                            // check_m1(base, recur + rest, depth) == post.
                            lemma_times_splice(base, q, kcount, mid, rest, depth);
                            assert(check_m1(base, s2.cont, depth) == check_m1(base, rest, depth));
                            lemma_check_invariant(s2, base, depth, (k - 1) as nat);
                            assert(spec_stepn(s, k) == spec_stepn(s2, (k - 1) as nat));
                        }
                    }
                    _ => { assert(false); }
                }
            }
            SpecWord::Prim(p2) => {
                let cs = s.stack;
                assert(abs_step_prim(astk, p2) is Some);
                let astk2 = abs_step_prim(astk, p2)->Some_0;
                assert(check_m1(astk, s.cont, depth) == check_m1(astk2, rest, depth));
                lemma_abs_step_prim_sl(astk, p2);
                lemma_prim_step_sound(cs, astk, p2, rest, astk2);
                assert(spec_step(s) == spec_step_prim(cs, p2, rest));
                match spec_step_prim(cs, p2, rest) {
                    SpecStep::Next(s2) => {
                        assert(s2.cont == rest);
                        assert(models_stack(s2.stack, astk2));
                        lemma_check_invariant(s2, astk2, depth, (k - 1) as nat);
                        assert(spec_stepn(s, k) == spec_stepn(s2, (k - 1) as nat));
                    }
                    SpecStep::Fault(e) => {
                        assert(e == Error::Overflow || e == Error::DivByZero);
                        assert(spec_stepn(s, k) == SpecStep::Fault(e));
                    }
                    _ => { assert(false); }
                }
            }
            SpecWord::Call(_) => {
                assert(check_m1(astk, s.cont, depth) is None);
                assert(false);
            }
        }
    }
}

/// **T-Static (full fragment, WITH If) — FULLY PROVEN (milestone-2 Part A).** Same
/// statement as `thm_static_straightline` but for programs that may contain `If`,
/// using the If-aware `check_m1`. Discharged by `lemma_check_invariant` — the
/// If-inlining correspondence is now machine-checked; the milestone-1 `assume` is
/// gone.
pub proof fn thm_static_with_if(p: Seq<SpecWord>, rho: Seq<SpecValue>, k: nat, depth: nat)
    requires
        check_m1(Seq::<AbsVal>::empty(), p, depth) is Some,
    ensures
        ({
            let s0 = SpecState { stack: rho, cont: p };
            &&& !(spec_stepn(s0, k) matches SpecStep::Fault(e)
                    && (e == Error::Underflow || e == Error::TypeMismatch))
            &&& (spec_stepn(s0, k) matches SpecStep::Halt(fin)
                    ==> models_stack(fin, check_m1(Seq::<AbsVal>::empty(), p, depth)->Some_0))
        }),
{
    let s0 = SpecState { stack: rho, cont: p };
    assert(models_stack(rho, Seq::<AbsVal>::empty()));
    assert(check_m1(Seq::<AbsVal>::empty(), s0.cont, depth) is Some);
    lemma_check_invariant(s0, Seq::<AbsVal>::empty(), depth, k);
}

} // verus!

fn main() {}
