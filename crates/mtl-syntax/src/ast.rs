//! Abstract syntax for MTL, mirroring the executable core types.
//!
//! # Correspondence to `mtl-core`
//!
//! The types here mirror the *executable* (`exec`) side of
//! `crates/mtl-core/src/mtl_core.rs` one-to-one. That file is a self-contained
//! Verus artifact — it is checked by the `verus` tool, not compiled by cargo —
//! so its types cannot be imported. We re-declare them here as ordinary Rust
//! types with real derives.
//!
//! | this crate            | mtl-core (exec)          | mtl-core (ghost)        |
//! |-----------------------|--------------------------|-------------------------|
//! | [`Prim`]              | `SpecPrim`               | `SpecPrim`              |
//! | [`Word::PushInt`]     | `Word::PushInt(i64)`     | `SpecWord::PushInt(int)`|
//! | [`Word::PushQuote`]   | `Word::PushQuote(Vec<Word>)` | `SpecWord::PushQuote` |
//! | [`Word::Prim`]        | `Word::Prim(SpecPrim)`   | `SpecWord::Prim`        |
//! | [`Word::Call`]        | `Word::Call(Vec<char>)`  | `SpecWord::Call(Seq<char>)` |
//!
//! Note: the exec core's `Word` has **no string variant** (there is no
//! `PushString`), so neither does this AST — string literals are a spec-level
//! surface form the core cannot represent (see [`crate::parse`]).

/// A primitive operation. Mirrors `SpecPrim` in the mtl-core ghost/exec model.
///
/// Each primitive has a canonical single-character glyph; see
/// [`glyph_to_prim`] / [`prim_to_glyph`] and [`GLYPHS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

/// A single MTL word. Mirrors the exec `Word` in mtl-core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Word {
    /// An integer literal pushed onto the stack.
    PushInt(i64),
    /// A quotation (deferred program) pushed onto the stack.
    PushQuote(Vec<Word>),
    /// A primitive operation.
    Prim(Prim),
    /// A named word `[a-z][a-z0-9]*`. Matches exec `Call(Vec<char>)`.
    Call(Vec<char>),
}

/// A whole MTL program: a flat sequence of words.
pub type Program = Vec<Word>;

/// The canonical glyph ↔ [`Prim`] table.
///
/// This is the **single source of truth** shared by both the lexer
/// ([`crate::parse`]) and the pretty-printer ([`crate::print`]). Brackets
/// `[` and `]` are quotation delimiters, not primitives, so they do not appear
/// here.
pub const GLYPHS: &[(char, Prim)] = &[
    (':', Prim::Dup),
    ('_', Prim::Drop),
    ('~', Prim::Swap),
    ('@', Prim::Rot),
    ('^', Prim::Over),
    ('!', Prim::Apply),
    (',', Prim::Cat),
    (';', Prim::Cons),
    ('\'', Prim::Dip),
    ('+', Prim::Add),
    ('-', Prim::Sub),
    ('*', Prim::Mul),
    ('/', Prim::Div),
    ('%', Prim::Mod),
    ('=', Prim::Eq),
    ('<', Prim::Lt),
    ('?', Prim::If),
    ('&', Prim::PrimRec),
    ('.', Prim::Times),
    ('|', Prim::LinRec),
    ('>', Prim::Uncons),
];

/// Map a glyph character to its primitive, if any.
pub fn glyph_to_prim(c: char) -> Option<Prim> {
    GLYPHS.iter().find(|&&(g, _)| g == c).map(|&(_, p)| p)
}

/// Map a primitive to its canonical glyph. Total: every [`Prim`] has a glyph.
pub fn prim_to_glyph(p: Prim) -> char {
    GLYPHS
        .iter()
        .find(|&&(_, q)| q == p)
        .map(|&(g, _)| g)
        // Unreachable: GLYPHS covers all 21 primitives. Total fallback keeps
        // this panic-free regardless.
        .unwrap_or('\u{FFFD}')
}
