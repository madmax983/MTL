//! Core value/word/fault types for the arena engine.
//!
//! These are the arena's **own** mirrors of the exec AST (`mtl_core::interp`
//! `Prim`/`Word`/`Value`/`Fault`). They are deliberately *not* re-exports of the
//! reference types (design §5: the arena is a **policed mirror**, kept in lockstep
//! by the conformance crate — it must not become a seventh un-policed mirror).
//!
//! Two deliberate divergences from the reference AST, both policed:
//!   * [`Word::PushQuote`] holds a [`QuoteId`] (a `{start,len}` slice into the
//!     interned tape) instead of an owned `Vec<Word>`.
//!   * [`Word::Call`] holds a `u32` intern index instead of an owned `String`.
//!
//! Reference types are only produced at the reification boundary (see
//! [`crate::vm`]'s reify helpers), which is where the arena hands owned data back
//! to the rest of the system.

use mtl_core::interp as itp;

/// The primitive set: 17 v0.1 + 4 v0.2 recursion + 2 v0.3 sequence primitives.
///
/// **Mirror invariant** (design §5, policed by the conformance crate): the
/// variants, their order, and their arity must match `mtl_syntax::manifest`
/// (and transitively `mtl_core::interp::Prim`). Adding/removing/reordering a
/// variant here without updating the manifest is a drift bug.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prim {
    Dup,
    Drop,
    Swap,
    Rot,
    Over,
    Apply,
    Cat,
    Cons,
    Dip,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Lt,
    If,
    PrimRec,
    Times,
    LinRec,
    Uncons,
    Fold,
    Xor,
}

/// A source program word (owned tree form). Dependency-free mirror of
/// `mtl_core::interp::Word`; this is the *input* representation that
/// [`crate::Vm::compile`] interns into the tape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgWord {
    PushInt(i64),
    PushQuote(Vec<ProgWord>),
    Prim(Prim),
    Call(String),
}

/// A runtime fault kind. Mirrors `mtl_core::interp::Fault`. Faults are terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Underflow,
    TypeMismatch,
    Overflow,
    DivByZero,
}

/// An interned program word living in the [`crate::QuoteArena`] tape. `Copy` and
/// small. Nested quotes are referenced by [`QuoteId`]; call names are interned to
/// a `u32` index.
#[derive(Clone, Copy, Debug)]
pub enum Word {
    PushInt(i64),
    PushQuote(QuoteId),
    Prim(Prim),
    Call(u32),
}

/// A quote body: a contiguous `[start, start+len)` slice of the tape. `Copy`.
///
/// Sub-slicing a list tail (`{start+1, len-1}`) is O(1) and shares structure with
/// the parent quote — this is why `uncons`/`fold` tails are free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteId {
    pub start: u32,
    pub len: u32,
}

impl QuoteId {
    /// One-past-the-end tape index. `saturating_add` is exact here because the
    /// allocation helpers (`try_alloc`/`try_cat`/`try_cons`/…) guarantee
    /// `start + len <= u32::MAX` — they fault on overflow rather than wrapping.
    #[inline]
    pub fn end(self) -> u32 {
        self.start.saturating_add(self.len)
    }
}

/// A first-class value. `Copy`. Mirrors `mtl_core::interp::Value`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Quote(QuoteId),
}

/// `value_to_word`: reify a value back to a tape word (used by `cons`/`dip`/`fold`).
/// Mirrors `value_to_word` in `interp.rs`.
#[inline]
pub(crate) fn value_to_word(v: Value) -> Word {
    match v {
        Value::Int(i) => Word::PushInt(i),
        Value::Quote(id) => Word::PushQuote(id),
    }
}

/// Map an arena [`Fault`] to the reference `interp::Fault`. The two enums have
/// identical variants; this is the only place the mapping lives.
#[inline]
pub(crate) fn to_itp_fault(f: Fault) -> itp::Fault {
    match f {
        Fault::Underflow => itp::Fault::Underflow,
        Fault::TypeMismatch => itp::Fault::TypeMismatch,
        Fault::Overflow => itp::Fault::Overflow,
        Fault::DivByZero => itp::Fault::DivByZero,
    }
}
