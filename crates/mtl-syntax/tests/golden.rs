//! Golden and adversarial tests: concrete parse results, canonical print
//! strings, and exact error kinds/positions.

use mtl_syntax::ast::Prim::*;
use mtl_syntax::parse::{ParseError, ParseErrorKind};
use mtl_syntax::{parse, print, Word};

/// Convenience: PushInt.
fn i(n: i64) -> Word {
    Word::PushInt(n)
}
/// Convenience: Prim.
fn p(pr: mtl_syntax::Prim) -> Word {
    Word::Prim(pr)
}
/// Convenience: Call from a &str.
fn call(s: &str) -> Word {
    Word::Call(s.chars().collect())
}
/// Convenience: PushQuote.
fn q(ws: Vec<Word>) -> Word {
    Word::PushQuote(ws)
}

/// Assert parse gives exactly `ast` AND print round-trips to `canon`.
fn check(src: &str, ast: Vec<Word>, canon: &str) {
    assert_eq!(parse(src), Ok(ast.clone()), "parse({src:?})");
    assert_eq!(print(&ast), canon, "print of parsed {src:?}");
    // And the canonical string must re-parse to the same AST.
    assert_eq!(parse(canon), Ok(ast), "reparse of canonical {canon:?}");
}

#[test]
fn spec_dup_apply_self_delimit() {
    // The spec's `:!` self-delimiting example.
    check(":!", vec![p(Dup), p(Apply)], ":!");
}

#[test]
fn dup_i_self_application_idiom() {
    // [:!]:!  — the dup-i self-application idiom.
    check(
        "[:!]:!",
        vec![q(vec![p(Dup), p(Apply)]), p(Dup), p(Apply)],
        "[:!]:!",
    );
}

#[test]
fn each_glyph_alone() {
    let cases: &[(&str, mtl_syntax::Prim)] = &[
        (":", Dup),
        ("_", Drop),
        ("~", Swap),
        ("@", Rot),
        ("^", Over),
        ("!", Apply),
        (",", Cat),
        (";", Cons),
        ("'", Dip),
        ("+", Add),
        ("-", Sub),
        ("*", Mul),
        ("/", Div),
        ("%", Mod),
        ("=", Eq),
        ("<", Lt),
        ("?", If),
        ("&", PrimRec),
        (".", Times),
        ("|", LinRec),
        (">", Uncons),
    ];
    for (glyph, prim) in cases {
        check(glyph, vec![p(*prim)], glyph);
    }
}

#[test]
fn adjacency() {
    check("1-2", vec![i(1), p(Sub), i(2)], "1-2");
    check("-7", vec![p(Sub), i(7)], "-7");
    check("12", vec![i(12)], "12");
    check("1 2", vec![i(1), i(2)], "1 2");
    check("ab", vec![call("ab")], "ab");
    check("a b", vec![call("a"), call("b")], "a b");
    check("a5", vec![call("a5")], "a5");
    check("a 5", vec![call("a"), i(5)], "a 5");
    check("5a", vec![i(5), call("a")], "5a");
}

#[test]
fn v02_glyph_adjacency() {
    // The KEY case from the design doc §5: `]&` is self-delimiting and must
    // round-trip (closing quote bracket immediately followed by `&`).
    check("[1][*]&", vec![q(vec![i(1)]), q(vec![p(Mul)]), p(PrimRec)], "[1][*]&");
    // `.` after a digit: integer literals are `[0-9]+` (no decimal point per
    // spec §2.3), so `3.` lexes to `[Int(3), Times]`, never a float.
    check("3.", vec![i(3), p(Times)], "3.");
    // `]|` adjacency (linrec combine form).
    check("[*]|", vec![q(vec![p(Mul)]), p(LinRec)], "[*]|");
    // `]>` adjacency (uncons on a quotation).
    check("[1]>", vec![q(vec![i(1)]), p(Uncons)], "[1]>");

    // Each new glyph directly after a digit — all self-delimiting.
    check("3&", vec![i(3), p(PrimRec)], "3&");
    check("3|", vec![i(3), p(LinRec)], "3|");
    check("3>", vec![i(3), p(Uncons)], "3>");

    // New glyphs adjacent to each other.
    check("&.|>", vec![p(PrimRec), p(Times), p(LinRec), p(Uncons)], "&.|>");
    // ...and adjacent to brackets.
    check("&[]", vec![p(PrimRec), q(vec![])], "&[]");
    check("[.]", vec![q(vec![p(Times)])], "[.]");
    // A new glyph followed by a digit needs no separator (punct is self-delim).
    check("&3", vec![p(PrimRec), i(3)], "&3");
    check(".3", vec![p(Times), i(3)], ".3");
}

#[test]
fn v02_design_doc_programs() {
    // Design doc §7 hand-traced example programs for the recursion primitives.

    // factorial `[1][*]&` (primrec, §3.1):
    // PushQuote[PushInt(1)] PushQuote[Prim(Mul)] Prim(PrimRec)
    check(
        "[1][*]&",
        vec![q(vec![i(1)]), q(vec![p(Mul)]), p(PrimRec)],
        "[1][*]&",
    );

    // gcd `[:0=][_][~^%][]|` (linrec, §3.3): four PushQuotes then LinRec.
    // P=:0= -> [Dup, Int(0), Eq]; T=_ -> [Drop]; R1=~^% -> [Swap, Over, Mod];
    // R2=[] -> empty quote.
    check(
        "[:0=][_][~^%][]|",
        vec![
            q(vec![p(Dup), i(0), p(Eq)]),
            q(vec![p(Drop)]),
            q(vec![p(Swap), p(Over), p(Mod)]),
            q(vec![]),
            p(LinRec),
        ],
        "[:0=][_][~^%][]|",
    );

    // fib (times, §3.2):
    // [Int(0), Int(1), Rot, PushQuote[Swap, Over, Add], Times, Drop]
    //
    // NOTE: the design doc §7 writes the source as `01@[~^+]._`, but under the
    // merged spec's maximal-munch integer rule (`[0-9]+`, parse.rs:149), `01`
    // lexes as the SINGLE literal `1`, not the two seeds `0` `1`. The two int
    // pushes must be separated; the canonical form has a space between them.
    check(
        "0 1@[~^+]._",
        vec![
            i(0),
            i(1),
            p(Rot),
            q(vec![p(Swap), p(Over), p(Add)]),
            p(Times),
            p(Drop),
        ],
        "0 1@[~^+]._",
    );
    // Confirm the doc's unspaced form collapses the seeds (spec behaviour, not a
    // bug in the parser): `01` == integer 1.
    assert_eq!(
        parse("01@[~^+]._"),
        Ok(vec![
            i(1),
            p(Rot),
            q(vec![p(Swap), p(Over), p(Add)]),
            p(Times),
            p(Drop),
        ])
    );
}

#[test]
fn nesting() {
    check("[]", vec![q(vec![])], "[]");
    check("[[[]]]", vec![q(vec![q(vec![q(vec![])])])], "[[[]]]");
    check(
        "[1[2]3]",
        vec![q(vec![i(1), q(vec![i(2)]), i(3)])],
        "[1[2]3]",
    );
}

#[test]
fn whitespace_collapse() {
    assert_eq!(parse("  :  !  "), Ok(vec![p(Dup), p(Apply)]));
    assert_eq!(print(&vec![p(Dup), p(Apply)]), ":!");
    // tabs and newlines too.
    assert_eq!(parse("\t:\n!\r"), Ok(vec![p(Dup), p(Apply)]));
    assert_eq!(parse("1\t\n 2"), Ok(vec![i(1), i(2)]));
    assert_eq!(print(&vec![i(1), i(2)]), "1 2");
}

#[test]
fn integer_boundaries() {
    check(
        "9223372036854775807",
        vec![i(i64::MAX)],
        "9223372036854775807",
    );
    check("0", vec![i(0)], "0");
    assert_eq!(
        parse("9223372036854775808"),
        Err(ParseError {
            pos: 0,
            kind: ParseErrorKind::IntOverflow {
                literal: "9223372036854775808".to_string()
            }
        })
    );
}

#[test]
fn errors_exact_kind_and_pos() {
    assert_eq!(
        parse("]"),
        Err(ParseError {
            pos: 0,
            kind: ParseErrorKind::UnexpectedCloseBracket
        })
    );
    assert_eq!(
        parse("["),
        Err(ParseError {
            pos: 0,
            kind: ParseErrorKind::UnclosedQuote { opened_at: 0 }
        })
    );
    assert_eq!(
        parse("[1"),
        Err(ParseError {
            pos: 0,
            kind: ParseErrorKind::UnclosedQuote { opened_at: 0 }
        })
    );
    assert_eq!(
        parse("\"hi\""),
        Err(ParseError {
            pos: 0,
            kind: ParseErrorKind::StringUnsupported
        })
    );
    assert_eq!(
        parse("#f[]"),
        Err(ParseError {
            pos: 0,
            kind: ParseErrorKind::DefinitionUnsupported
        })
    );
    assert_eq!(
        parse("$"),
        Err(ParseError {
            pos: 0,
            kind: ParseErrorKind::UnexpectedChar { found: '$' }
        })
    );
}

#[test]
fn nested_close_bracket_position() {
    // A stray `]` after some tokens reports the right byte position.
    assert_eq!(
        parse(":!]"),
        Err(ParseError {
            pos: 2,
            kind: ParseErrorKind::UnexpectedCloseBracket
        })
    );
}
