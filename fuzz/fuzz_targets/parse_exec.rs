#![no_main]
//! Full production pipeline fuzz target: source bytes -> parse -> execute.
//!
//! Interprets the fuzzer bytes as MTL source, parses them, and (on a successful
//! parse) runs the program through BOTH engines behind the Engine seam under a
//! fuel bound, asserting they agree. This wires the parser to the interpreter/
//! arena so adversarial *textual* inputs — not just AST-level ones — exercise
//! the proof-to-production boundary end to end.
//!
//! Any panic in parse or in either engine is a totality bug; any engine
//! disagreement is a refinement bug.

use libfuzzer_sys::fuzz_target;
use mtl_fuzz::differential;
use mtl_syntax::parse;

fuzz_target!(|data: &[u8]| {
    let src = String::from_utf8_lossy(data);
    let prog = match parse(&src) {
        Ok(p) => p,
        Err(_) => return,
    };
    // Bridge the syntax AST to the core interp AST (structurally identical: the
    // parser only ever emits non-negative PushInt, any prim, Call, or PushQuote).
    let core_prog = to_core(&prog);
    if let Err(msg) = differential(&core_prog) {
        panic!("ENGINE DIVERGENCE on parsed program\nsource: {:?}\n{}", src, msg);
    }
});

fn to_core(prog: &[mtl_syntax::Word]) -> Vec<mtl_core::interp::Word> {
    prog.iter().map(word_to_core).collect()
}

fn word_to_core(w: &mtl_syntax::Word) -> mtl_core::interp::Word {
    use mtl_core::interp as itp;
    use mtl_syntax::Word as SW;
    match w {
        SW::PushInt(n) => itp::Word::PushInt(*n),
        SW::PushQuote(q) => itp::Word::PushQuote(q.iter().map(word_to_core).collect()),
        SW::Call(name) => itp::Word::Call(name.iter().collect()),
        SW::Prim(p) => itp::Word::Prim(prim_to_core(*p)),
    }
}

fn prim_to_core(p: mtl_syntax::Prim) -> mtl_core::interp::Prim {
    use mtl_core::interp::Prim as I;
    use mtl_syntax::Prim as S;
    match p {
        S::Dup => I::Dup,
        S::Drop => I::Drop,
        S::Swap => I::Swap,
        S::Rot => I::Rot,
        S::Over => I::Over,
        S::Apply => I::Apply,
        S::Cat => I::Cat,
        S::Cons => I::Cons,
        S::Dip => I::Dip,
        S::Add => I::Add,
        S::Sub => I::Sub,
        S::Mul => I::Mul,
        S::Div => I::Div,
        S::Mod => I::Mod,
        S::Eq => I::Eq,
        S::Lt => I::Lt,
        S::If => I::If,
        S::PrimRec => I::PrimRec,
        S::Times => I::Times,
        S::LinRec => I::LinRec,
        S::Uncons => I::Uncons,
        S::Fold => I::Fold,
        S::Xor => I::Xor,
    }
}
