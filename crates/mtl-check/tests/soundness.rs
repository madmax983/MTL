//! Fault-corpus soundness smoke test.
//!
//! For each KNOWN-FAULTING one-liner we (a) run it on the REAL reference
//! interpreter (`mtl_core::interp::run`) from an empty stack and confirm it
//! actually faults `Underflow`/`TypeMismatch`, then (b) assert the static checker
//! is SOUND: it must NEVER return `Static` claiming the program is safe on the
//! stack it was actually run on.
//!
//! The subtlety (design §"Any pop from an empty stack"): the checker's `pre` is
//! *polymorphic* — a bare `+` is `Static` with `pre = [Int, Int]` ("safe GIVEN
//! two Int inputs"), even though it Underflows on the EMPTY stack. That is NOT
//! unsound: `Static(pre, post)` only claims safety for input stacks of shape
//! `pre`. So the real soundness invariant we check is:
//!
//!   if the checker returns `Static(effect)`, then running the interpreter on a
//!   fresh stack of shape `effect.pre` does NOT fault `Underflow`/`TypeMismatch`.
//!
//! TypeMismatch programs are provable regardless of inputs, so for those the
//! checker must additionally return `Reject`.

use mtl_check::{check, Kind, Verdict};
use mtl_core::interp::{run, Fault, Outcome, Prim as IPrim, Value, Vm, Word as IWord};
use mtl_syntax::{ast::Prim, parse, Word};

const FUEL: u64 = 100_000;

fn conv(w: &Word) -> IWord {
    match w {
        Word::PushInt(n) => IWord::PushInt(*n),
        Word::PushQuote(b) => IWord::PushQuote(b.iter().map(conv).collect()),
        Word::Call(c) => IWord::Call(c.iter().collect::<String>()),
        Word::Prim(p) => IWord::Prim(match p {
            Prim::Dup => IPrim::Dup,
            Prim::Drop => IPrim::Drop,
            Prim::Swap => IPrim::Swap,
            Prim::Rot => IPrim::Rot,
            Prim::Over => IPrim::Over,
            Prim::Apply => IPrim::Apply,
            Prim::Cat => IPrim::Cat,
            Prim::Cons => IPrim::Cons,
            Prim::Dip => IPrim::Dip,
            Prim::Add => IPrim::Add,
            Prim::Sub => IPrim::Sub,
            Prim::Mul => IPrim::Mul,
            Prim::Div => IPrim::Div,
            Prim::Mod => IPrim::Mod,
            Prim::Eq => IPrim::Eq,
            Prim::Lt => IPrim::Lt,
            Prim::If => IPrim::If,
            Prim::PrimRec => IPrim::PrimRec,
            Prim::Times => IPrim::Times,
            Prim::LinRec => IPrim::LinRec,
            Prim::Uncons => IPrim::Uncons,
            Prim::Fold => IPrim::Fold,
            Prim::Xor => IPrim::Xor,
        }),
    }
}

fn prog(src: &str) -> Vec<IWord> {
    parse(src).expect("parse").iter().map(conv).collect()
}

/// Run from empty and return the fault kind, if any.
fn fault_from_empty(src: &str) -> Option<Fault> {
    match run(Vm::new(prog(src)), FUEL) {
        Outcome::Fault(fi) => Some(fi.fault),
        _ => None,
    }
}

/// Build a concrete input stack matching a checker `pre` row (bottom..top).
fn seed(pre: &[Kind]) -> Vec<Value> {
    pre.iter()
        .map(|k| match k {
            Kind::Int | Kind::Any => Value::Int(0),
            Kind::Quote => Value::Quote(vec![]),
        })
        .collect()
}

/// The soundness invariant for one program: if `Static`, running on a
/// `pre`-shaped stack must not fault Underflow/TypeMismatch.
fn assert_sound(src: &str) {
    let verdict = check(&parse(src).expect("parse"));
    if let Verdict::Static(effect) = &verdict {
        let out = run(Vm::with_stack(seed(&effect.pre), prog(src)), FUEL);
        if let Outcome::Fault(fi) = &out {
            assert!(
                !matches!(fi.fault, Fault::Underflow | Fault::TypeMismatch),
                "UNSOUND: `{src}` checked Static {effect} but faulted {:?} on a pre-shaped stack",
                fi.fault
            );
        }
    }
}

// --------------------------------------------------------------------------
// TypeMismatch corpus: provable regardless of inputs → checker MUST Reject.
// --------------------------------------------------------------------------

#[test]
fn type_mismatch_corpus_rejected_and_faults() {
    let cases = [
        "[1]2+", // add: 2nd operand is a quote
        "1[2]+", // add: top operand is a quote
        "[1]:+", // add: both operands quotes (dup a quote)
        "1 2!",  // apply an int
        "1>",    // uncons an int
        "[+]>",  // uncons a quote whose head is a bare prim (malformed head)
        "[9][1][2]?", // if with a non-int flag
    ];
    for src in cases {
        // (a) the real interpreter faults TypeMismatch from empty.
        assert_eq!(
            fault_from_empty(src),
            Some(Fault::TypeMismatch),
            "`{src}` should fault TypeMismatch on the reference interpreter"
        );
        // (b) the checker must Reject (and hence is not Static).
        let verdict = check(&parse(src).expect("parse"));
        assert!(
            verdict.is_reject(),
            "`{src}` must be Reject, got {verdict:?}"
        );
        assert_sound(src);
    }
}

// --------------------------------------------------------------------------
// Underflow corpus: faults from EMPTY because inputs are missing. The checker
// borrows them into a non-empty `pre` (Static-given-inputs) — which is SOUND.
// --------------------------------------------------------------------------

#[test]
fn underflow_corpus_never_unsafely_static() {
    let cases = [
        "_", // drop on empty
        ":", // dup on empty
        "~", // swap on empty
        "+", // add on empty
        "1+", // add with one operand
    ];
    for src in cases {
        // (a) the real interpreter Underflows from empty.
        assert_eq!(
            fault_from_empty(src),
            Some(Fault::Underflow),
            "`{src}` should Underflow on the reference interpreter from empty"
        );
        // (b) the checker must NOT claim safety on the EMPTY stack: either it is
        // not Static, or its inferred `pre` is non-empty (demands the inputs the
        // empty run lacked).
        let verdict = check(&parse(src).expect("parse"));
        match &verdict {
            Verdict::Static(effect) => assert!(
                !effect.pre.is_empty(),
                "UNSOUND: `{src}` Static with EMPTY pre but Underflows from empty"
            ),
            Verdict::Guarded(..) | Verdict::Reject { .. } => {}
        }
        // (c) the deep soundness check: on a pre-shaped stack it does not fault.
        assert_sound(src);
    }
}
