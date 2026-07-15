//! Shared fuzz harness helpers for MTL.
//!
//! Two pieces of reusable machinery:
//!
//! * [`gen_program`] — an `arbitrary`-driven generator that turns raw fuzzer
//!   bytes into a well-typed `mtl_core::interp` program AST (bounded depth so a
//!   pathological input cannot blow the stack during *generation*; the
//!   interpreter itself is still driven under an explicit fuel bound). This is
//!   the AST-level analogue of the `word_strategy` proptest generator in
//!   `crates/mtl-syntax/tests/p4_roundtrip.rs` and the differential corpus in
//!   `crates/mtl-arena/tests/oracle.rs`.
//! * [`differential`] — runs a program through BOTH engines behind the Engine
//!   seam (`mtl_core::interp::run`, the reference oracle, and
//!   `mtl_arena::run_arena`, the production default) under a shared fuel bound
//!   and returns `Err` on ANY disagreement in terminal kind, fault kind, or
//!   final stack. A divergence is an arena/interp refinement bug — the exact
//!   property the machine-checked arena refinement proof
//!   (`crates/mtl-arena/proofs/arena_verus.rs`) claims to hold. The fuzzer is
//!   the continuous, adversarial net under that proof.

use arbitrary::{Arbitrary, Unstructured};
use mtl_core::interp as itp;
use mtl_arena as arena;

/// Fuel bound for both engines in the differential/exec targets. Large enough to
/// let real recursion (PrimRec/Times/LinRec/Fold) run to completion on small
/// programs, small enough that a typical fuzz iteration stays fast.
///
/// NOTE: step-fuel bounds the number of *steps*, not memory. A pathological
/// program (a quote-doubling body under a loop: `[q] N [dup cat] T`) grows its
/// structure exponentially *per step*, so no finite step-fuel bounds its memory
/// — both engines copy on `cat`. The CI fuzz job therefore ALSO passes libFuzzer
/// `-rss_limit_mb` + `-timeout` as the real safety net; those catch an
/// adversarial resource-exhaustion input (a proof-to-production / #19 concern)
/// as a saved crash artifact, distinct from a genuine panic or engine divergence.
pub const FUEL: u64 = 100_000;

/// All 23 primitives, indexable from a fuzzer byte.
const ALL_PRIMS: [itp::Prim; 23] = [
    itp::Prim::Dup,
    itp::Prim::Drop,
    itp::Prim::Swap,
    itp::Prim::Rot,
    itp::Prim::Over,
    itp::Prim::Apply,
    itp::Prim::Cat,
    itp::Prim::Cons,
    itp::Prim::Dip,
    itp::Prim::Add,
    itp::Prim::Sub,
    itp::Prim::Mul,
    itp::Prim::Div,
    itp::Prim::Mod,
    itp::Prim::Eq,
    itp::Prim::Lt,
    itp::Prim::If,
    itp::Prim::PrimRec,
    itp::Prim::Times,
    itp::Prim::LinRec,
    itp::Prim::Uncons,
    itp::Prim::Fold,
    itp::Prim::Xor,
];

/// Max quotation nesting depth during generation (guards the *generator*, not
/// the interpreter — the interpreter is fuel-bounded).
const MAX_DEPTH: u32 = 6;
/// Max words per (sub)program.
const MAX_WORDS: usize = 12;

/// Generate one `Word` from the fuzzer's unstructured bytes, bounded by `depth`.
fn gen_word(u: &mut Unstructured, depth: u32) -> arbitrary::Result<itp::Word> {
    // Choose a variant. At max depth, never pick a quotation (leaf-only).
    let variants = if depth >= MAX_DEPTH { 3u8 } else { 4u8 };
    match u8::arbitrary(u)? % variants {
        0 => {
            // Non-negative ints dominate the parser's image, but the interpreter
            // must stay total on any i64, so allow the full range here.
            let n = i64::arbitrary(u)?;
            Ok(itp::Word::PushInt(n))
        }
        1 => {
            let p = ALL_PRIMS[(u8::arbitrary(u)? as usize) % ALL_PRIMS.len()];
            Ok(itp::Word::Prim(p))
        }
        2 => {
            // A short lowercase name (unbound Call → the interpreter treats it as
            // a fault/no-op path; still must never panic).
            let len = 1 + (u8::arbitrary(u)? as usize % 3);
            let mut s = String::with_capacity(len);
            for i in 0..len {
                let b = u8::arbitrary(u)?;
                let c = if i == 0 {
                    (b'a' + b % 26) as char
                } else {
                    match b % 36 {
                        d @ 0..=25 => (b'a' + d) as char,
                        d => (b'0' + (d - 26)) as char,
                    }
                };
                s.push(c);
            }
            Ok(itp::Word::Call(s))
        }
        _ => {
            let n = u8::arbitrary(u)? as usize % (MAX_WORDS / 2 + 1);
            let mut body = Vec::with_capacity(n);
            for _ in 0..n {
                body.push(gen_word(u, depth + 1)?);
            }
            Ok(itp::Word::PushQuote(body))
        }
    }
}

/// Generate a bounded program (`Vec<Word>`) from raw fuzzer bytes.
pub fn gen_program(data: &[u8]) -> Vec<itp::Word> {
    let mut u = Unstructured::new(data);
    let n = match u8::arbitrary(&mut u) {
        Ok(b) => b as usize % (MAX_WORDS + 1),
        Err(_) => return Vec::new(),
    };
    let mut prog = Vec::with_capacity(n);
    for _ in 0..n {
        match gen_word(&mut u, 0) {
            Ok(w) => prog.push(w),
            Err(_) => break, // ran out of bytes; run what we have
        }
    }
    prog
}

// ---- interp <-> arena conversion (local copy of the test-only helpers in
//      crates/mtl-arena/tests/common/mod.rs, which are not part of the public
//      API) ----------------------------------------------------------------

fn conv_word(w: &itp::Word) -> arena::ProgWord {
    arena::word_from_interp(w)
}

fn progword_to_itp(pw: &arena::ProgWord) -> itp::Word {
    match pw {
        arena::ProgWord::PushInt(n) => itp::Word::PushInt(*n),
        arena::ProgWord::PushQuote(b) => {
            itp::Word::PushQuote(b.iter().map(progword_to_itp).collect())
        }
        arena::ProgWord::Prim(p) => itp::Word::Prim(unconv_prim(*p)),
        arena::ProgWord::Call(n) => itp::Word::Call(n.clone()),
    }
}

fn unconv_prim(p: arena::Prim) -> itp::Prim {
    // arena::Prim and itp::Prim are parallel 23-variant enums in the same order;
    // round-trip through the index in ARENA_PRIMS to stay robust.
    ALL_PRIMS[arena::ARENA_PRIMS.iter().position(|&x| x == p).unwrap()]
}

fn arena_value_to_itp(vm: &arena::Vm, v: arena::Value) -> itp::Value {
    match v {
        arena::Value::Int(n) => itp::Value::Int(n),
        arena::Value::Quote(id) => {
            itp::Value::Quote(vm.reify_quote(id).iter().map(progword_to_itp).collect())
        }
    }
}

fn fault_eq(i: itp::Fault, a: arena::Fault) -> bool {
    use arena::Fault as A;
    use itp::Fault as I;
    matches!(
        (i, a),
        (I::Underflow, A::Underflow)
            | (I::TypeMismatch, A::TypeMismatch)
            | (I::Overflow, A::Overflow)
            | (I::DivByZero, A::DivByZero)
    )
}

/// Run `prog` through both engines behind the Engine seam and assert agreement.
/// Returns `Err(description)` on ANY divergence (a refinement bug).
pub fn differential(prog: &[itp::Word]) -> Result<(), String> {
    // Run the arena FIRST. If the program exceeds the arena's u32 tape capacity
    // it faults `Overflow` — a documented boundary the reference interpreter has
    // no matching cap for (it would instead try to allocate the whole structure
    // and OOM). That capacity boundary is outside the meaningful differential
    // domain (the arena refinement proof characterizes exactly this
    // u32-capacity -> Overflow case), so skip it rather than run the interpreter
    // into an out-of-memory kill.
    let prog_arena: Vec<arena::ProgWord> = prog.iter().map(conv_word).collect();
    let run = arena::run_arena(&prog_arena, FUEL);
    if matches!(run.end, arena::ArenaEnd::Fault(arena::Fault::Overflow)) {
        return Ok(());
    }

    let itp_out = itp::run(itp::Vm::new(prog.to_vec()), FUEL);

    let arena_stack: Vec<itp::Value> = run
        .vm
        .stack_values(run.state.stack)
        .into_iter()
        .map(|v| arena_value_to_itp(&run.vm, v))
        .collect();

    match (&itp_out, &run.end) {
        (itp::Outcome::Halt(s_itp), arena::ArenaEnd::Halt) => {
            if *s_itp == arena_stack {
                Ok(())
            } else {
                Err(format!(
                    "HALT stacks differ\n  interp: {:?}\n  arena:  {:?}",
                    s_itp, arena_stack
                ))
            }
        }
        (itp::Outcome::Fault(fi), arena::ArenaEnd::Fault(f)) => {
            if fault_eq(fi.fault, *f) && fi.stack == arena_stack {
                Ok(())
            } else {
                Err(format!(
                    "FAULT differs\n  interp: {:?} stack {:?}\n  arena:  {:?} stack {:?}",
                    fi.fault, fi.stack, f, arena_stack
                ))
            }
        }
        // Both hit the fuel wall: agreement (a fuzz iteration bounds fuel low, so
        // many programs legitimately run out; only a MISMATCH in which engine ran
        // out is a bug, caught by the catch-all below).
        (itp::Outcome::FuelExhausted { .. }, arena::ArenaEnd::FuelExhausted) => Ok(()),
        // Both yielded to the host on the same unbound Call: agree on the name.
        (itp::Outcome::Invoke { name: ni, .. }, arena::ArenaEnd::Invoke(na)) => {
            if ni == na {
                Ok(())
            } else {
                Err(format!("INVOKE name differs\n  interp: {:?}\n  arena:  {:?}", ni, na))
            }
        }
        (i, a) => Err(format!(
            "terminal kind differs\n  interp: {:?}\n  arena:  {:?}",
            i, a
        )),
    }
}

/// Convenience for the `arbitrary`-derived program wrapper used by targets.
#[derive(Debug)]
pub struct Prog(pub Vec<itp::Word>);

impl<'a> Arbitrary<'a> for Prog {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Prog(gen_program(u.peek_bytes(u.len()).unwrap_or(&[]))))
    }

    fn arbitrary_take_rest(u: Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Prog(gen_program(u.take_rest())))
    }
}
