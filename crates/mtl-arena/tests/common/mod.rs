//! Shared conversion helpers for the arena differential tests. Ported from the
//! spike's `tests/oracle.rs` (conv_prim / unconv_prim / conv_word /
//! progword_to_itp) so both the oracle and the fault-parity corpus build arena
//! programs from `interp` ASTs and reify arena values back to `interp` values.
#![allow(dead_code)]

use mtl_arena as arena;
use mtl_core::interp as itp;

pub fn conv_prim(p: itp::Prim) -> arena::Prim {
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

pub fn unconv_prim(p: arena::Prim) -> itp::Prim {
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

pub fn conv_word(w: &itp::Word) -> arena::ProgWord {
    match w {
        itp::Word::PushInt(n) => arena::ProgWord::PushInt(*n),
        itp::Word::PushQuote(q) => arena::ProgWord::PushQuote(q.iter().map(conv_word).collect()),
        itp::Word::Prim(p) => arena::ProgWord::Prim(conv_prim(*p)),
        itp::Word::Call(name) => arena::ProgWord::Call(name.clone()),
    }
}

pub fn progword_to_itp(pw: &arena::ProgWord) -> itp::Word {
    match pw {
        arena::ProgWord::PushInt(n) => itp::Word::PushInt(*n),
        arena::ProgWord::PushQuote(b) => itp::Word::PushQuote(b.iter().map(progword_to_itp).collect()),
        arena::ProgWord::Prim(p) => itp::Word::Prim(unconv_prim(*p)),
        arena::ProgWord::Call(n) => itp::Word::Call(n.clone()),
    }
}

/// Reify one arena `Value` (via the arena's tape) back to an `itp::Value`.
pub fn arena_value_to_itp(vm: &arena::Vm, v: arena::Value) -> itp::Value {
    match v {
        arena::Value::Int(n) => itp::Value::Int(n),
        arena::Value::Quote(id) => {
            itp::Value::Quote(vm.reify_quote(id).iter().map(progword_to_itp).collect())
        }
    }
}

pub fn fault_eq(i: itp::Fault, a: arena::Fault) -> bool {
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

pub fn value_to_word(v: &itp::Value) -> itp::Word {
    match v {
        itp::Value::Int(n) => itp::Word::PushInt(*n),
        itp::Value::Quote(q) => itp::Word::PushQuote(q.clone()),
    }
}

/// A differential test case: an initial stack (encoded as leading pushes) and a
/// program body.
pub struct Case {
    pub name: String,
    pub init: Vec<itp::Value>,
    pub prog: Vec<itp::Word>,
}

pub fn from_perf(name: &str, pair: (Vec<itp::Value>, Vec<itp::Word>)) -> Case {
    Case { name: name.to_string(), init: pair.0, prog: pair.1 }
}

pub fn prog(name: &str, ws: Vec<itp::Word>) -> Case {
    Case { name: name.to_string(), init: vec![], prog: ws }
}

/// The 5 canonical fault cases (fault-order parity): underflow, type mismatch on
/// add, div-by-zero, apply-on-int, if-with-non-quote-branches. Ported from the
/// spike oracle (`oracle.rs:319-323`).
pub fn fault_cases() -> Vec<Case> {
    use itp::build::*;
    vec![
        prog("fault_underflow", vec![int(1), add()]), // needs 2, has 1
        prog("fault_type_add", vec![int(1), quote(vec![int(2)]), add()]), // Int op Quote
        prog("fault_divzero", vec![int(5), int(0), div()]),
        prog("fault_apply_type", vec![int(7), apply()]), // apply on Int
        prog("fault_if_type", vec![int(1), int(2), int(3), iff()]), // branches not quotes
    ]
}
