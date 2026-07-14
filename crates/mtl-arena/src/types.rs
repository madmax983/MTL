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

/// Every arena [`Prim`] in canonical order, indices 0..=22.
///
/// This is the arena's **policed reflection surface** (design §5): the
/// `mtl-conformance` crate iterates this array and asserts variant names, order,
/// and arity agree with `mtl_syntax::manifest` — the single source of truth — so
/// the arena mirror cannot silently drift. It is deliberately a fixed-size
/// `[Prim; 23]`: changing the count without updating the mirror fails to compile.
pub const ARENA_PRIMS: [Prim; 23] = [
    Prim::Dup,
    Prim::Drop,
    Prim::Swap,
    Prim::Rot,
    Prim::Over,
    Prim::Apply,
    Prim::Cat,
    Prim::Cons,
    Prim::Dip,
    Prim::Add,
    Prim::Sub,
    Prim::Mul,
    Prim::Div,
    Prim::Mod,
    Prim::Eq,
    Prim::Lt,
    Prim::If,
    Prim::PrimRec,
    Prim::Times,
    Prim::LinRec,
    Prim::Uncons,
    Prim::Fold,
    Prim::Xor,
];

/// The canonical variant name of an arena [`Prim`] (the same string
/// `format!("{:?}", p)` produces, and the same string the manifest records in
/// `PrimMeta::name`). This is an **exhaustive** match with no wildcard arm — the
/// compile-time drift guard: adding a `Prim` variant without a name here fails to
/// compile.
pub const fn arena_prim_name(p: Prim) -> &'static str {
    match p {
        Prim::Dup => "Dup",
        Prim::Drop => "Drop",
        Prim::Swap => "Swap",
        Prim::Rot => "Rot",
        Prim::Over => "Over",
        Prim::Apply => "Apply",
        Prim::Cat => "Cat",
        Prim::Cons => "Cons",
        Prim::Dip => "Dip",
        Prim::Add => "Add",
        Prim::Sub => "Sub",
        Prim::Mul => "Mul",
        Prim::Div => "Div",
        Prim::Mod => "Mod",
        Prim::Eq => "Eq",
        Prim::Lt => "Lt",
        Prim::If => "If",
        Prim::PrimRec => "PrimRec",
        Prim::Times => "Times",
        Prim::LinRec => "LinRec",
        Prim::Uncons => "Uncons",
        Prim::Fold => "Fold",
        Prim::Xor => "Xor",
    }
}

/// The minimum operand-stack depth below which the arena faults `Underflow` for
/// an arena [`Prim`] — mirrored from the `popN` guards in [`crate::vm`]'s
/// `exec_prim` and pinned to `mtl_syntax::manifest`'s arities by the conformance
/// crate. Exhaustive, no wildcard: a new variant must declare its arity here.
pub const fn arena_prim_arity(p: Prim) -> usize {
    match p {
        Prim::Dup => 1,
        Prim::Drop => 1,
        Prim::Swap => 2,
        Prim::Rot => 3,
        Prim::Over => 2,
        Prim::Apply => 1,
        Prim::Cat => 2,
        Prim::Cons => 2,
        Prim::Dip => 2,
        Prim::Add => 2,
        Prim::Sub => 2,
        Prim::Mul => 2,
        Prim::Div => 2,
        Prim::Mod => 2,
        Prim::Eq => 2,
        Prim::Lt => 2,
        Prim::If => 3,
        Prim::PrimRec => 3,
        Prim::Times => 2,
        Prim::LinRec => 4,
        Prim::Uncons => 1,
        Prim::Fold => 3,
        Prim::Xor => 2,
    }
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
