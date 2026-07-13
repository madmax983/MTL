//! The checked primitive manifest — the single source of truth for the 23 v0.x
//! primitives' canonical order, glyphs, arities, and stack effects.
//!
//! This module declares **data only**: it generates none of the six mirrors
//! (parser `Prim`, Verus `SpecPrim`, interp `Prim`, `GLYPHS`, and the two `conv`
//! functions). Each mirror keeps its own hand-written declaration; the
//! `mtl-conformance` crate's test suite *compares* every mirror against this
//! manifest and fails loudly on any drift.
//!
//! Two drift guards live right here at compile time:
//!   * [`meta_of`] is an exhaustive match over [`crate::ast::Prim`] with **no
//!     wildcard** — adding a `syntax::Prim` variant without a manifest entry
//!     fails to compile here.
//!   * [`PRIMITIVES`] / [`ALL_PRIMS`] are fixed-size `[_; 23]` arrays — changing
//!     the count without updating the manifest fails to compile.

use crate::ast::Prim;

/// Metadata for one MTL primitive.
pub struct PrimMeta {
    /// Canonical index, 0..=22, matching the position in every mirror's enum.
    pub index: u8,
    /// The enum-variant identifier used verbatim in every mirror (e.g. `"Dup"`).
    /// This is what `format!("{:?}", prim)` produces for each mirror's variant.
    pub name: &'static str,
    /// The canonical single-character glyph from `GLYPHS`.
    pub glyph: char,
    /// The minimum stack depth below which the interpreter faults `Underflow`
    /// for this primitive (derived from the `if stack.len() < K` guards in
    /// `mtl-core`'s `exec_prim`).
    pub arity: u8,
    /// The spec stack-effect string. Informational.
    pub stack_effect: &'static str,
}

/// The 23 v0.x primitives in canonical order, indices 0..=22.
///
/// Arities are derived from `mtl-core/src/interp.rs`'s `exec_prim` underflow
/// guards; `mtl-conformance`'s `arity_matches_interp_underflow` test pins each
/// value to the real interpreter behavior.
pub const PRIMITIVES: [PrimMeta; 23] = [
    PrimMeta { index: 0,  name: "Dup",     glyph: ':',  arity: 1, stack_effect: "( a -- a a )" },
    PrimMeta { index: 1,  name: "Drop",    glyph: '_',  arity: 1, stack_effect: "( a -- )" },
    PrimMeta { index: 2,  name: "Swap",    glyph: '~',  arity: 2, stack_effect: "( a b -- b a )" },
    PrimMeta { index: 3,  name: "Rot",     glyph: '@',  arity: 3, stack_effect: "( a b c -- b c a )" },
    PrimMeta { index: 4,  name: "Over",    glyph: '^',  arity: 2, stack_effect: "( a b -- a b a )" },
    PrimMeta { index: 5,  name: "Apply",   glyph: '!',  arity: 1, stack_effect: "( [q] -- ... )" },
    PrimMeta { index: 6,  name: "Cat",     glyph: ',',  arity: 2, stack_effect: "( [a] [b] -- [a b] )" },
    PrimMeta { index: 7,  name: "Cons",    glyph: ';',  arity: 2, stack_effect: "( v [q] -- [v q] )" },
    PrimMeta { index: 8,  name: "Dip",     glyph: '\'', arity: 2, stack_effect: "( a [q] -- ... a )" },
    PrimMeta { index: 9,  name: "Add",     glyph: '+',  arity: 2, stack_effect: "( a b -- a+b )" },
    PrimMeta { index: 10, name: "Sub",     glyph: '-',  arity: 2, stack_effect: "( a b -- a-b )" },
    PrimMeta { index: 11, name: "Mul",     glyph: '*',  arity: 2, stack_effect: "( a b -- a*b )" },
    PrimMeta { index: 12, name: "Div",     glyph: '/',  arity: 2, stack_effect: "( a b -- a/b )" },
    PrimMeta { index: 13, name: "Mod",     glyph: '%',  arity: 2, stack_effect: "( a b -- a%b )" },
    PrimMeta { index: 14, name: "Eq",      glyph: '=',  arity: 2, stack_effect: "( a b -- c )" },
    PrimMeta { index: 15, name: "Lt",      glyph: '<',  arity: 2, stack_effect: "( a b -- c )" },
    PrimMeta { index: 16, name: "If",      glyph: '?',  arity: 3, stack_effect: "( c [t] [f] -- ... )" },
    PrimMeta { index: 17, name: "PrimRec", glyph: '&',  arity: 3, stack_effect: "( n [I] [C] -- r )" },
    PrimMeta { index: 18, name: "Times",   glyph: '.',  arity: 2, stack_effect: "( n [Q] -- ... )" },
    PrimMeta { index: 19, name: "LinRec",  glyph: '|',  arity: 4, stack_effect: "( [P] [T] [R1] [R2] -- ... )" },
    PrimMeta { index: 20, name: "Uncons",  glyph: '>',  arity: 1, stack_effect: "( [w ...] -- w [...] 1 ) | ( [] -- 0 )" },
    PrimMeta { index: 21, name: "Fold",    glyph: '(',  arity: 3, stack_effect: "( [seq] init [C] -- r )" },
    PrimMeta { index: 22, name: "Xor",     glyph: '$',  arity: 2, stack_effect: "( a b -- a^b )" },
];

/// Every [`crate::ast::Prim`] in canonical order, indices 0..=22.
pub const ALL_PRIMS: [Prim; 23] = [
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

/// Map a [`crate::ast::Prim`] to its manifest entry.
///
/// This is an **exhaustive** match with no wildcard arm: it is the compile-time
/// drift guard. Adding a `syntax::Prim` variant without a corresponding manifest
/// entry fails to compile here.
pub const fn meta_of(p: Prim) -> &'static PrimMeta {
    match p {
        Prim::Dup => &PRIMITIVES[0],
        Prim::Drop => &PRIMITIVES[1],
        Prim::Swap => &PRIMITIVES[2],
        Prim::Rot => &PRIMITIVES[3],
        Prim::Over => &PRIMITIVES[4],
        Prim::Apply => &PRIMITIVES[5],
        Prim::Cat => &PRIMITIVES[6],
        Prim::Cons => &PRIMITIVES[7],
        Prim::Dip => &PRIMITIVES[8],
        Prim::Add => &PRIMITIVES[9],
        Prim::Sub => &PRIMITIVES[10],
        Prim::Mul => &PRIMITIVES[11],
        Prim::Div => &PRIMITIVES[12],
        Prim::Mod => &PRIMITIVES[13],
        Prim::Eq => &PRIMITIVES[14],
        Prim::Lt => &PRIMITIVES[15],
        Prim::If => &PRIMITIVES[16],
        Prim::PrimRec => &PRIMITIVES[17],
        Prim::Times => &PRIMITIVES[18],
        Prim::LinRec => &PRIMITIVES[19],
        Prim::Uncons => &PRIMITIVES[20],
        Prim::Fold => &PRIMITIVES[21],
        Prim::Xor => &PRIMITIVES[22],
    }
}
