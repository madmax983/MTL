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
