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

/// Generate the parser/printer primitive mirror — the [`Prim`] enum and the
/// canonical [`GLYPHS`] table — from the checked manifest's canonical rows
/// (`crate::manifest::for_each_primitive!`). This is the codegen from issue #46:
/// the enum variants and the glyph table are no longer hand-written, so they
/// cannot drift from the manifest.
macro_rules! define_syntax_prim {
    ( $( ($idx:expr, $name:ident, $glyph:literal, $arity:literal, $eff:literal) ),* $(,)? ) => {
        /// A primitive operation. Mirrors `SpecPrim` in the mtl-core ghost/exec model.
        ///
        /// Each primitive has a canonical single-character glyph; see
        /// [`glyph_to_prim`] / [`prim_to_glyph`] and [`GLYPHS`].
        ///
        /// Generated from `crate::manifest::for_each_primitive!` — the single
        /// source of truth — so it cannot drift from the manifest.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Prim {
            $( $name ),*
        }

        /// The canonical glyph ↔ [`Prim`] table.
        ///
        /// This is the shared print/parse mirror used by both the lexer
        /// ([`crate::parse`]) and the pretty-printer ([`crate::print`]). Brackets
        /// `[` and `]` are quotation delimiters, not primitives, so they do not
        /// appear here.
        ///
        /// Generated from `crate::manifest::for_each_primitive!` — the single
        /// source of truth — so it cannot drift from the manifest.
        pub const GLYPHS: &[(char, Prim)] = &[
            $( ($glyph, Prim::$name) ),*
        ];
    };
}

crate::for_each_primitive!(define_syntax_prim);

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

/// Map a glyph character to its primitive, if any.
pub fn glyph_to_prim(c: char) -> Option<Prim> {
    GLYPHS.iter().find(|&&(g, _)| g == c).map(|&(_, p)| p)
}

/// Map a primitive to its canonical glyph. Total: every [`Prim`] has a glyph.
///
/// Sourced from the checked [`crate::manifest`], whose [`meta_of`] is an
/// exhaustive match with no wildcard — so this is compile-time total with no
/// silent `\u{FFFD}` fallback. The manifest is the single glyph source; the
/// conformance suite asserts `GLYPHS` agrees with it.
///
/// [`meta_of`]: crate::manifest::meta_of
pub fn prim_to_glyph(p: Prim) -> char {
    crate::manifest::meta_of(p).glyph
}
