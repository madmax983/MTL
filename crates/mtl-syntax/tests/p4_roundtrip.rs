//! P4 round-trip property tests.
//!
//! **P4** — parse and print are mutually consistent over the *well-formed*
//! domain. A program is well-formed iff every `PushInt(n)` has `n >= 0`: this
//! is precisely the image of the parser, since integer literals are unsigned
//! (`[0-9]+`). P4 has two directions:
//!
//! * (A) AST round-trip: for all well-formed programs `p`,
//!   `parse(&print(&p)) == Ok(p)`.
//! * (B) Text canonicalization / idempotence: for any `src` that parses,
//!   `print(parse(src))` is canonical — re-parsing yields the same AST and
//!   re-printing is idempotent.
//!
//! Plus totality: `parse` and `print` never panic on any input.

use mtl_syntax::ast::Prim;
use mtl_syntax::{parse, print, Word};
use proptest::prelude::*;

/// All 23 primitives, for the generator.
const ALL_PRIMS: [Prim; 23] = [
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

/// A name `[a-z][a-z0-9]*` of 1..=3 chars.
fn name_strategy() -> impl Strategy<Value = Vec<char>> {
    proptest::collection::vec(
        prop_oneof![
            (b'a'..=b'z').prop_map(|b| b as char),
            (b'0'..=b'9').prop_map(|b| b as char),
        ],
        0..=2,
    )
    .prop_perturb(|rest, mut rng| {
        // First char must be [a-z].
        let first = (b'a' + (rng.next_u32() % 26) as u8) as char;
        let mut v = Vec::with_capacity(rest.len() + 1);
        v.push(first);
        v.extend(rest);
        v
    })
}

/// A well-formed `Word` strategy: leaves are non-negative ints, any prim, or a
/// name; recursive nodes are quotations of up to 4 words, bounded depth.
fn word_strategy() -> impl Strategy<Value = Word> {
    let leaf = prop_oneof![
        (0i64..=i64::MAX).prop_map(Word::PushInt),
        prop::sample::select(ALL_PRIMS.to_vec()).prop_map(Word::Prim),
        name_strategy().prop_map(Word::Call),
    ];
    leaf.prop_recursive(4, 32, 4, |inner| {
        proptest::collection::vec(inner, 0..=4).prop_map(Word::PushQuote)
    })
}

/// A well-formed program: up to 8 words.
fn program_strategy() -> impl Strategy<Value = Vec<Word>> {
    proptest::collection::vec(word_strategy(), 0..=8)
}

proptest! {
    /// P4 (A): AST round-trip.
    #[test]
    fn p4_ast_roundtrip(p in program_strategy()) {
        let s = print(&p);
        prop_assert_eq!(parse(&s), Ok(p));
    }

    /// P4 (B): text canonicalization / print idempotence.
    #[test]
    fn p4_print_idempotent(p in program_strategy()) {
        let s = print(&p);
        // Re-parsing the canonical string yields the same AST.
        prop_assert_eq!(parse(&s), Ok(p.clone()));
        // Re-printing the re-parsed AST is idempotent.
        let reparsed = parse(&s).expect("canonical string must parse");
        prop_assert_eq!(print(&reparsed), s);
    }

    /// Totality of parse: never panics on arbitrary strings, including ones
    /// full of brackets and glyphs.
    #[test]
    fn parse_totality(s in ".*") {
        let _ = parse(&s);
    }

    /// Totality of parse on random ascii (biased toward syntactic chars).
    #[test]
    fn parse_totality_ascii(
        s in proptest::collection::vec(
            prop_oneof![
                Just('['), Just(']'), Just(':'), Just('!'), Just('-'),
                // `)` is a still-unassigned char (unlike `$`, now the xor
                // glyph) — keeps an `UnexpectedChar` case in the fuzz mix.
                Just('"'), Just('#'), Just('$'), Just(')'), Just(' '),
                (b'0'..=b'9').prop_map(|b| b as char),
                (b'a'..=b'z').prop_map(|b| b as char),
                any::<char>(),
            ],
            0..40,
        ).prop_map(|v| v.into_iter().collect::<String>())
    ) {
        let _ = parse(&s);
    }
}

/// An UNRESTRICTED word strategy including negative ints and i64::MIN, for
/// print totality only.
fn unrestricted_word_strategy() -> impl Strategy<Value = Word> {
    let leaf = prop_oneof![
        // Full i64 range including negatives and i64::MIN.
        any::<i64>().prop_map(Word::PushInt),
        Just(Word::PushInt(i64::MIN)),
        Just(Word::PushInt(i64::MAX)),
        prop::sample::select(ALL_PRIMS.to_vec()).prop_map(Word::Prim),
        name_strategy().prop_map(Word::Call),
    ];
    leaf.prop_recursive(4, 32, 4, |inner| {
        proptest::collection::vec(inner, 0..=4).prop_map(Word::PushQuote)
    })
}

proptest! {
    /// Totality of print: never panics on any Word, including negative ints and
    /// i64::MIN. We deliberately do NOT assert round-trip here — negative ints
    /// are outside the parser's image (unsigned literals only), so they do not
    /// round-trip through `parse`.
    #[test]
    fn print_totality(
        p in proptest::collection::vec(unrestricted_word_strategy(), 0..=8)
    ) {
        let _ = print(&p);
    }
}

/// Explicit BPE-merge adjacency round-trip vectors for the v0.3 glyphs `(`
/// (fold) and `$` (xor). The glyph choices were BPE-measured because the
/// tokenizer merges `](` and `[$`; those boundaries must survive a print/parse
/// round-trip byte-exact. Each source below is already canonical, so we assert
/// both `parse(print(ast)) == ast` and that the printed form is byte-identical.
#[test]
fn v03_glyph_adjacency_roundtrip() {
    fn i(n: i64) -> Word {
        Word::PushInt(n)
    }
    fn p(pr: Prim) -> Word {
        Word::Prim(pr)
    }
    fn q(ws: Vec<Word>) -> Word {
        Word::PushQuote(ws)
    }
    fn check(src: &str, ast: Vec<Word>) {
        assert_eq!(parse(src), Ok(ast.clone()), "parse({src:?})");
        assert_eq!(print(&ast), src, "canonical print of {src:?}");
        assert_eq!(parse(&print(&ast)), Ok(ast), "round-trip of {src:?}");
    }

    // `]` immediately followed by `(` — the `](` merge (fold after a quote).
    check("[+](", vec![q(vec![p(Prim::Add)]), p(Prim::Fold)]);
    // `[` immediately followed by `$` — the `[$` merge (xor inside a quote).
    check("[$]", vec![q(vec![p(Prim::Xor)])]);
    check("[$]|", vec![q(vec![p(Prim::Xor)]), p(Prim::LinRec)]);
    // `(` adjacent to a digit, both sides (fold is self-delimiting punct).
    check("3(", vec![i(3), p(Prim::Fold)]);
    check("(3", vec![p(Prim::Fold), i(3)]);
    // `(` adjacent to a quotation, both sides.
    check("[](", vec![q(vec![]), p(Prim::Fold)]);
    check("([]", vec![p(Prim::Fold), q(vec![])]);
    // `((` and `$$` sequences.
    check("((", vec![p(Prim::Fold), p(Prim::Fold)]);
    check("$$", vec![p(Prim::Xor), p(Prim::Xor)]);
}
