//! Arena mirror conformance police (design §5: "the arena must not become a
//! seventh un-policed mirror").
//!
//! The production arena backend (`mtl-arena`) keeps its OWN `Prim`/`Word`/`Value`
//! mirrors rather than re-exporting `mtl_core::interp`'s. These tests make that
//! mirror a first-class policed one:
//!
//!   * name / order / count / arity of the arena `Prim` enum are asserted against
//!     `mtl_syntax::manifest` — the single source of truth — exactly as the
//!     interp mirror is (`interp_prim_names_match_manifest`,
//!     `arity_matches_interp_underflow`);
//!   * a compile-time no-wildcard exhaustiveness guard (`_arena_prim_exhaustive`)
//!     turns "added a prim without updating the mirror" into a COMPILE error;
//!   * the `Call(u32)`↔`Call(String)` intern divergence is round-trip checked;
//!   * a PERMANENT differential oracle runs the full validated corpus + a fault
//!     corpus through BOTH `mtl_core::interp::run` and `mtl_arena::run_arena` and
//!     asserts bit-identical `Outcome` (final stack, `FaultInfo`, cont, terminal
//!     kind);
//!   * a `proptest` differential test does the same over random well-formed
//!     programs.
//!
//! Every assertion fails LOUDLY, naming the mirror, the primitive, and
//! expected-vs-actual, so a drift is diagnosable from the panic alone.

use mtl_arena as arena;
use mtl_core::interp as itp;
use mtl_perf as perf;
use mtl_syntax::manifest::PRIMITIVES;

use proptest::prelude::*;

// ============================================================
// Conversions (ported from the spike oracle, tests/oracle.rs)
// ============================================================

/// interp `Prim` -> arena `Prim` (both mirrors of the manifest).
fn conv_prim(p: itp::Prim) -> arena::Prim {
    use arena::Prim as A;
    use itp::Prim as I;
    match p {
        I::Dup => A::Dup,
        I::Drop => A::Drop,
        I::Swap => A::Swap,
        I::Rot => A::Rot,
        I::Over => A::Over,
        I::Apply => A::Apply,
        I::Cat => A::Cat,
        I::Cons => A::Cons,
        I::Dip => A::Dip,
        I::Add => A::Add,
        I::Sub => A::Sub,
        I::Mul => A::Mul,
        I::Div => A::Div,
        I::Mod => A::Mod,
        I::Eq => A::Eq,
        I::Lt => A::Lt,
        I::If => A::If,
        I::PrimRec => A::PrimRec,
        I::Times => A::Times,
        I::LinRec => A::LinRec,
        I::Uncons => A::Uncons,
        I::Fold => A::Fold,
        I::Xor => A::Xor,
    }
}

/// arena `Prim` -> interp `Prim`.
fn unconv_prim(p: arena::Prim) -> itp::Prim {
    use arena::Prim as A;
    use itp::Prim as I;
    match p {
        A::Dup => I::Dup,
        A::Drop => I::Drop,
        A::Swap => I::Swap,
        A::Rot => I::Rot,
        A::Over => I::Over,
        A::Apply => I::Apply,
        A::Cat => I::Cat,
        A::Cons => I::Cons,
        A::Dip => I::Dip,
        A::Add => I::Add,
        A::Sub => I::Sub,
        A::Mul => I::Mul,
        A::Div => I::Div,
        A::Mod => I::Mod,
        A::Eq => I::Eq,
        A::Lt => I::Lt,
        A::If => I::If,
        A::PrimRec => I::PrimRec,
        A::Times => I::Times,
        A::LinRec => I::LinRec,
        A::Uncons => I::Uncons,
        A::Fold => I::Fold,
        A::Xor => I::Xor,
    }
}

/// interp `Word` tree -> arena `ProgWord` tree (the input the arena compiles).
fn conv_word(w: &itp::Word) -> arena::ProgWord {
    match w {
        itp::Word::PushInt(n) => arena::ProgWord::PushInt(*n),
        itp::Word::PushQuote(q) => arena::ProgWord::PushQuote(q.iter().map(conv_word).collect()),
        itp::Word::Prim(p) => arena::ProgWord::Prim(conv_prim(*p)),
        itp::Word::Call(name) => arena::ProgWord::Call(name.clone()),
    }
}

/// arena `ProgWord` tree -> interp `Word` tree (for the proptest generator, which
/// builds arena programs and needs the interp twin to run the oracle of truth).
fn progword_to_itp(pw: &arena::ProgWord) -> itp::Word {
    match pw {
        arena::ProgWord::PushInt(n) => itp::Word::PushInt(*n),
        arena::ProgWord::PushQuote(b) => itp::Word::PushQuote(b.iter().map(progword_to_itp).collect()),
        arena::ProgWord::Prim(p) => itp::Word::Prim(unconv_prim(*p)),
        arena::ProgWord::Call(n) => itp::Word::Call(n.clone()),
    }
}

fn value_to_word(v: &itp::Value) -> itp::Word {
    match v {
        itp::Value::Int(n) => itp::Word::PushInt(*n),
        itp::Value::Quote(q) => itp::Word::PushQuote(q.clone()),
    }
}

// ============================================================
// 1. arena_prim_names_match_manifest  (mirror of #4 for the arena)
// ============================================================
#[test]
fn arena_prim_names_match_manifest() {
    for i in 0..23 {
        let p = arena::ARENA_PRIMS[i];
        // Debug name (the format the manifest records) must match.
        assert_eq!(
            format!("{:?}", p),
            PRIMITIVES[i].name,
            "arena::Prim drift: index {} expected name {:?} got Debug {:?}",
            i,
            PRIMITIVES[i].name,
            format!("{:?}", p)
        );
        // The reflection accessor must agree with the manifest too.
        assert_eq!(
            arena::arena_prim_name(p),
            PRIMITIVES[i].name,
            "arena::Prim drift: index {} arena_prim_name yields {:?} but manifest says {:?}",
            i,
            arena::arena_prim_name(p),
            PRIMITIVES[i].name
        );
    }
}

// ============================================================
// 2. arena_prim_count_matches_manifest
// ============================================================
#[test]
fn arena_prim_count_matches_manifest() {
    assert_eq!(
        arena::ARENA_PRIMS.len(),
        23,
        "arena::Prim drift: ARENA_PRIMS.len() expected 23 got {}",
        arena::ARENA_PRIMS.len()
    );
    assert_eq!(
        arena::ARENA_PRIMS.len(),
        PRIMITIVES.len(),
        "arena::Prim drift: ARENA_PRIMS.len() ({}) != manifest PRIMITIVES.len() ({})",
        arena::ARENA_PRIMS.len(),
        PRIMITIVES.len()
    );
    // All 23 arena prims distinct.
    for i in 0..23 {
        for j in (i + 1)..23 {
            assert_ne!(
                arena::ARENA_PRIMS[i], arena::ARENA_PRIMS[j],
                "arena::Prim drift: ARENA_PRIMS[{}] and ARENA_PRIMS[{}] are the same variant {:?}",
                i, j, arena::ARENA_PRIMS[i]
            );
        }
    }
}

// ============================================================
// 3. arena_arity_matches_manifest  (mirror of #8, applied to the ARENA)
// ============================================================

/// Static half: the arena's declared arity accessor must equal the manifest arity.
#[test]
fn arena_arity_matches_manifest() {
    for i in 0..23 {
        let p = arena::ARENA_PRIMS[i];
        assert_eq!(
            arena::arena_prim_arity(p),
            PRIMITIVES[i].arity as usize,
            "arena arity drift: primitive {} declared arena arity {} but manifest arity {}",
            PRIMITIVES[i].name,
            arena::arena_prim_arity(p),
            PRIMITIVES[i].arity
        );
    }
}

/// Dynamic half (the #32-style perturbation, applied to the arena): for each
/// prim, a program feeding arity-1 ints MUST underflow-fault in the arena, and
/// arity ints MUST NOT. This pins the arena's real underflow threshold to exactly
/// the manifest arity, not just its declared accessor.
#[test]
fn arena_arity_matches_underflow() {
    const FUEL: u64 = 10_000;
    for i in 0..23 {
        let meta = &PRIMITIVES[i];
        let a = meta.arity as usize;
        if a < 1 {
            continue;
        }
        let p = arena::ARENA_PRIMS[i];

        // Lower bound: a-1 ints MUST fault Underflow (arity checked before type).
        let mut below: Vec<arena::ProgWord> = (0..(a - 1))
            .map(|k| arena::ProgWord::PushInt(k as i64))
            .collect();
        below.push(arena::ProgWord::Prim(p));
        match arena::run_arena(&below, FUEL).outcome() {
            arena::Outcome::Fault(fi) => assert_eq!(
                fi.fault,
                itp::Fault::Underflow,
                "arena arity drift: primitive {} declared arity {} — with {} ints expected Fault::Underflow got {:?}",
                meta.name,
                a,
                a - 1,
                fi.fault
            ),
            other => panic!(
                "arena arity drift: primitive {} declared arity {} — with {} ints expected Fault::Underflow but got {:?}",
                meta.name,
                a,
                a - 1,
                other
            ),
        }

        // Upper bound: a ints MUST NOT fault Underflow (pins threshold to exactly a).
        let mut at: Vec<arena::ProgWord> =
            (0..a).map(|k| arena::ProgWord::PushInt(k as i64)).collect();
        at.push(arena::ProgWord::Prim(p));
        if let arena::Outcome::Fault(fi) = arena::run_arena(&at, FUEL).outcome() {
            assert_ne!(
                fi.fault,
                itp::Fault::Underflow,
                "arena arity drift: primitive {} declared arity {} — with {} ints it still faulted Underflow (arena guard demands more than {} operands)",
                meta.name,
                a,
                a,
                a
            );
        }
    }
}

// ============================================================
// 4. compile-time exhaustiveness guard (no wildcard)
// ============================================================

/// If an `arena::Prim` variant is added or removed, this match stops being
/// exhaustive (no wildcard arm) and the conformance crate fails to COMPILE —
/// forcing the mirror + manifest to be updated in lockstep. Mirrors
/// `interp_prim_exhaustive`'s `_exhaustive`.
#[allow(dead_code)]
fn _arena_prim_exhaustive(p: arena::Prim) {
    use arena::Prim::*;
    match p {
        Dup => (),
        Drop => (),
        Swap => (),
        Rot => (),
        Over => (),
        Apply => (),
        Cat => (),
        Cons => (),
        Dip => (),
        Add => (),
        Sub => (),
        Mul => (),
        Div => (),
        Mod => (),
        Eq => (),
        Lt => (),
        If => (),
        PrimRec => (),
        Times => (),
        LinRec => (),
        Uncons => (),
        Fold => (),
        Xor => (),
    }
}

// ============================================================
// 5. arena_call_intern_roundtrip  (Call(u32) <-> Call(String))
// ============================================================

/// The arena interns call targets to `u32` (`Word::Call(u32)`), diverging from
/// interp's `Word::Call(String)` — a policed mirror (design §5). Interning a set
/// of names (with duplicates) and reifying them back MUST recover the original
/// names exactly: `reify ∘ intern == identity`.
#[test]
fn arena_call_intern_roundtrip() {
    let names = [
        "read", "write", "read", "emit", "log", "write", "read", "commit",
    ];
    let prog: Vec<arena::ProgWord> = names
        .iter()
        .map(|n| arena::ProgWord::Call((*n).to_string()))
        .collect();

    let mut vm = arena::Vm::new();
    let qid = match vm.compile(&prog) {
        Some(id) => id,
        None => panic!(
            "arena call-intern drift: compiling {} Call words overflowed the tape (unreachable)",
            prog.len()
        ),
    };
    let reified = vm.reify_quote(qid);

    assert_eq!(
        reified, prog,
        "arena call-intern drift: reify ∘ intern is not identity\n  in:  {:?}\n  out: {:?}",
        prog, reified
    );

    // Spot-check that each reified word is a Call carrying the original name (i.e.
    // the u32 index really resolved back through the intern table).
    for (i, w) in reified.iter().enumerate() {
        match w {
            arena::ProgWord::Call(n) => assert_eq!(
                n, names[i],
                "arena call-intern drift: index {} interned name {:?} reified to {:?}",
                i, names[i], n
            ),
            other => panic!(
                "arena call-intern drift: index {} expected Call({:?}) got {:?}",
                i, names[i], other
            ),
        }
    }
}

// ============================================================
// 6. arena_differential_oracle  (promoted from the spike, PERMANENT + in CI)
// ============================================================

const FUEL: u64 = 50_000_000;

struct Case {
    name: String,
    init: Vec<itp::Value>,
    prog: Vec<itp::Word>,
}

fn from_perf(name: &str, pair: (Vec<itp::Value>, Vec<itp::Word>)) -> Case {
    Case { name: name.to_string(), init: pair.0, prog: pair.1 }
}

fn prog(name: &str, ws: Vec<itp::Word>) -> Case {
    Case { name: name.to_string(), init: vec![], prog: ws }
}

/// Assert the interp `Outcome` and the arena `Outcome` are bit-identical. Both
/// carry `interp`-typed payloads (the arena reifies at the generation boundary),
/// so equality is a direct structural comparison — final stack, `FaultInfo`
/// (fault + stack + cont), fuel/invoke state, and terminal kind.
fn outcomes_agree(name: &str, i: &itp::Outcome, a: &arena::Outcome) -> Result<(), String> {
    use arena::Outcome as AO;
    use itp::Outcome as IO;
    match (i, a) {
        (IO::Halt(si), AO::Halt(sa)) => {
            if si == sa {
                Ok(())
            } else {
                Err(format!(
                    "{}: HALT stacks differ\n  interp: {:?}\n  arena:  {:?}",
                    name, si, sa
                ))
            }
        }
        (IO::Fault(fi), AO::Fault(fa)) => {
            if fi == fa {
                Ok(())
            } else {
                Err(format!(
                    "{}: FaultInfo differs\n  interp: {:?}\n  arena:  {:?}",
                    name, fi, fa
                ))
            }
        }
        (
            IO::FuelExhausted { stack: si, cont: ci },
            AO::FuelExhausted { stack: sa, cont: ca },
        ) => {
            if si == sa && ci == ca {
                Ok(())
            } else {
                Err(format!(
                    "{}: FuelExhausted state differs\n  interp: stack {:?} cont {:?}\n  arena:  stack {:?} cont {:?}",
                    name, si, ci, sa, ca
                ))
            }
        }
        (
            IO::Invoke { name: ni, stack: si, cont: ci },
            AO::Invoke { name: na, stack: sa, cont: ca },
        ) => {
            if ni == na && si == sa && ci == ca {
                Ok(())
            } else {
                Err(format!(
                    "{}: Invoke state differs\n  interp: {:?} stack {:?} cont {:?}\n  arena:  {:?} stack {:?} cont {:?}",
                    name, ni, si, ci, na, sa, ca
                ))
            }
        }
        (i, a) => Err(format!(
            "{}: terminal kind differs\n  interp: {:?}\n  arena:  {:?}",
            name, i, a
        )),
    }
}

fn check(case: &Case) -> Result<(), String> {
    // full = <init as pushes> ++ prog, run on an empty stack in both backends.
    let mut full: Vec<itp::Word> = case.init.iter().map(value_to_word).collect();
    full.extend(case.prog.iter().cloned());

    let itp_out = itp::run(itp::Vm::new(full.clone()), FUEL);
    let prog_arena: Vec<arena::ProgWord> = full.iter().map(conv_word).collect();
    let arena_out = arena::run_arena(&prog_arena, FUEL).outcome();

    outcomes_agree(&case.name, &itp_out, &arena_out)
}

/// The validated corpus: the `mtl-perf` scenario builders (all 7 shapes, several
/// sizes) + hand-built prim-coverage programs + the 5-case fault corpus. Ported
/// verbatim from the spike oracle so the shapes match PERF-BASELINE.
fn corpus() -> Vec<Case> {
    use itp::build::*;
    use itp::Prim;
    use itp::Value;
    use itp::Word;

    let mut cases = Vec::new();

    // ---- 4 stress cases at small n (exact PERF-BASELINE builders) ----
    for k in [4usize, 16, 64] {
        cases.push(Case {
            name: format!("flat_1_1_add_drop_x{}", k),
            init: vec![],
            prog: perf::straightline(k),
        });
    }
    for n in [5i64, 20, 100] {
        cases.push(from_perf(&format!("primrec_sumto_{}", n), perf::primrec_sumto(n)));
    }
    for n in [5usize, 20, 100] {
        cases.push(from_perf(&format!("fold_sum_{}", n), perf::fold_sum(n)));
    }
    for n in [5i64, 20, 100] {
        cases.push(from_perf(&format!("selfapp_countdown_{}", n), perf::selfapp_countdown(n)));
    }

    // ---- other canonical perf shapes (LinRec / Times / quote-payload Fold) ----
    for n in [5i64, 20] {
        cases.push(from_perf(&format!("linrec_countdown_{}", n), perf::linrec_countdown(n)));
    }
    for n in [5i64, 20] {
        cases.push(from_perf(&format!("times_count_{}", n), perf::times_count(n)));
    }
    for n in [5usize, 20] {
        cases.push(from_perf(&format!("fold_quotes_{}", n), perf::fold_quotes(n)));
    }

    // ---- hand-built prim-coverage programs ----
    cases.push(prog("arith_mix", vec![int(3), int(4), add(), int(2), mul(), int(10), sub()]));
    cases.push(prog("div_pos", vec![int(17), int(5), div()]));
    cases.push(prog("mod_pos", vec![int(17), int(5), modulo()]));
    cases.push(prog("div_neg", vec![int(-17), int(5), div()]));
    cases.push(prog("mod_neg", vec![int(-17), int(5), modulo()]));
    cases.push(prog("cmp_lt", vec![int(3), int(7), lt()]));
    cases.push(prog("cmp_eq", vec![int(9), int(9), eq()]));
    cases.push(prog("xor_bits", vec![int(12), int(10), xor()]));
    cases.push(prog(
        "if_true",
        vec![int(1), quote(vec![int(111)]), quote(vec![int(222)]), iff()],
    ));
    cases.push(prog(
        "if_false",
        vec![int(0), quote(vec![int(111)]), quote(vec![int(222)]), iff()],
    ));
    cases.push(prog("shuffles", vec![int(1), int(2), int(3), rot(), over(), swap(), dup(), drop()]));
    cases.push(prog("cons", vec![int(5), quote(vec![int(1), int(2)]), cons()]));
    cases.push(prog(
        "cat",
        vec![quote(vec![int(1), int(2)]), quote(vec![int(3), int(4)]), cat()],
    ));
    cases.push(prog("uncons_nonempty", vec![quote(vec![int(7), int(8), int(9)]), uncons()]));
    cases.push(prog("uncons_empty", vec![quote(vec![]), uncons()]));
    cases.push(prog(
        "uncons_quote_head",
        vec![quote(vec![Word::PushQuote(vec![int(1)]), int(2)]), uncons()],
    ));
    cases.push(prog("dip", vec![int(1), int(2), quote(vec![int(10), add()]), dip()]));
    cases.push(prog("apply", vec![int(3), quote(vec![int(4), mul()]), apply()]));
    for n in [0i64, 1, 5, 6] {
        cases.push(Case {
            name: format!("primrec_factorial_{}", n),
            init: vec![Value::Int(n)],
            prog: vec![
                Word::PushQuote(vec![int(1)]),
                Word::PushQuote(vec![Word::Prim(Prim::Mul)]),
                Word::Prim(Prim::PrimRec),
            ],
        });
    }
    cases.push(prog(
        "fold_reverse",
        vec![
            quote(vec![int(1), int(2), int(3), int(4)]),
            quote(vec![]),
            quote(vec![swap(), cons()]),
            fold(),
        ],
    ));
    cases.push(prog(
        "nested_apply",
        vec![int(2), quote(vec![quote(vec![int(3), add()]), apply()]), apply()],
    ));

    // ---- fault cases (FaultInfo bit-identical; fault-order parity) ----
    cases.push(prog("fault_underflow", vec![int(1), add()]));
    cases.push(prog("fault_type_add", vec![int(1), quote(vec![int(2)]), add()]));
    cases.push(prog("fault_divzero", vec![int(5), int(0), div()]));
    cases.push(prog("fault_apply_type", vec![int(7), apply()]));
    cases.push(prog("fault_if_type", vec![int(1), int(2), int(3), iff()]));

    cases
}

#[test]
fn arena_differential_oracle() {
    let cases = corpus();
    let total = cases.len();
    let mut passed = 0usize;
    let mut failures = Vec::new();
    for c in &cases {
        match check(c) {
            Ok(()) => passed += 1,
            Err(e) => failures.push(e),
        }
    }
    println!("arena differential oracle: {}/{} programs bit-identical", passed, total);
    if !failures.is_empty() {
        panic!(
            "arena/interp DRIFT: {} / {} programs DIVERGED:\n{}",
            failures.len(),
            total,
            failures.join("\n")
        );
    }
    assert_eq!(passed, total, "arena differential oracle: {} of {} agreed", passed, total);
}

// ============================================================
// 7. arena_oracle_proptest  (randomized differential oracle)
// ============================================================

/// A well-formed arena program word: an int, a prim (any of the 23), or a nested
/// quote of the same. Bounded depth/size so CI stays fast. No `Call` (the intern
/// round-trip is covered separately; a bare Call just yields `Invoke` in both).
fn arb_progword() -> impl Strategy<Value = arena::ProgWord> {
    let leaf = prop_oneof![
        (-8i64..8i64).prop_map(arena::ProgWord::PushInt),
        (0usize..23usize).prop_map(|i| arena::ProgWord::Prim(arena::ARENA_PRIMS[i])),
    ];
    // depth <= 3, up to 32 total nodes, quotes hold 0..6 children.
    leaf.prop_recursive(3, 32, 6, |inner| {
        prop::collection::vec(inner, 0..6).prop_map(arena::ProgWord::PushQuote)
    })
}

/// A random program: 0..12 top-level words.
fn arb_prog() -> impl Strategy<Value = Vec<arena::ProgWord>> {
    prop::collection::vec(arb_progword(), 0..12)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// For every generated well-formed program, `interp::run` and `run_arena`
    /// must agree on the reified `Outcome` (Halt stack / FaultInfo / fuel / invoke).
    #[test]
    fn arena_oracle_proptest(program in arb_prog()) {
        // Bounded fuel keeps pathological random loops cheap; both backends count
        // steps identically, so a shared ceiling yields the same terminal.
        const PFUEL: u64 = 200_000;
        let itp_prog: Vec<itp::Word> = program.iter().map(progword_to_itp).collect();

        let itp_out = itp::run(itp::Vm::new(itp_prog), PFUEL);
        let arena_out = arena::run_arena(&program, PFUEL).outcome();

        if let Err(e) = outcomes_agree("proptest", &itp_out, &arena_out) {
            prop_assert!(false, "arena/interp DRIFT on {:?}:\n{}", program, e);
        }
    }
}
