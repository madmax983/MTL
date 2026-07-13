//! P4 differential oracle: pins the production parser/printer to an EXECUTABLE
//! twin of the PROVEN Verus exec model in `proofs/p4_verus.rs`.
//!
//! The Verus exec functions cannot be called from cargo, so this file is a
//! plain-Rust transcription of them — line-for-line faithful to the *proven*
//! executable functions (which are themselves machine-checked to refine the
//! `Seq<char>` spec, so twin == exec == spec on the well-formed domain):
//!
//!   * `twin_print`  mirrors `exec_print` (proofs/p4_verus.rs §15, lines
//!     ~1562), built from `exec_digits` (§15), `exec_needs_sep` (§15), and the
//!     `emit_token`/`exec_emit_word` token-flattening. `exec_print` carries
//!     `ensures r@ == spec_print(ewords_view(p@))`.
//!   * `twin_lex`    mirrors `exec_lex` + `scan_digits`/`scan_name` (§17–18),
//!     the maximal-munch tokenizer whose overflow-detection (`scan_digits`) is
//!     the i64 bound proven equivalent to spec `lex` on the in-range domain.
//!   * `twin_group`  mirrors `exec_group` (§20), the `[`/`]` quotation-stack
//!     grouper folded from the initial `[[]]` state.
//!   * `twin_parse`  mirrors `exec_parse` (§21): `exec_group(exec_lex(cs), [[]])`.
//!
//! The proven exec round-trip `exec_roundtrip` (§22) states
//! `exec_parse(exec_print(p)) == Ok(p)` for every `p : Vec<EWord>`; the
//! `production_roundtrip` proptest below is its executable analogue on real
//! production `parse`/`print`.
//!
//! This is the P2 oracle-pin pattern applied to P4: the machine-checked model
//! (proofs/p4_verus.rs) states the theorem; these tests pin the real Rust code
//! to that same PROVEN exec surface across many thousands of random cases plus
//! deterministic adversarial boundary/malformed inputs.
//!
//! NOTE on integers: the Verus spec treats a digit run as an unbounded natural
//! (round-trip holds for every n >= 0). The proven exec functions (and this
//! twin) additionally apply production's i64 bound (a digit run that overflows
//! i64 is an error — `scan_digits` returns `None`/`LOverflow`), so the twin
//! matches production byte-for-byte on ALL strings while coinciding with the
//! spec on the well-formed domain (values 0..=i64::MAX).

use mtl_syntax::ast::{glyph_to_prim, prim_to_glyph, Prim};
use mtl_syntax::{parse, print, Word};
use proptest::prelude::*;

// ------------------------------------------------------------------
// Character predicates (mirror is_digit / is_lower / is_namechar / is_ws).
// ------------------------------------------------------------------
fn is_digit(c: char) -> bool {
    ('0'..='9').contains(&c)
}
fn is_lower(c: char) -> bool {
    ('a'..='z').contains(&c)
}
fn is_namechar(c: char) -> bool {
    is_digit(c) || is_lower(c)
}
fn is_ws(c: char) -> bool {
    c == ' ' || c == '\t' || c == '\n' || c == '\r'
}

// ------------------------------------------------------------------
// Token classes and the printer twin (mirror render_h + toks_words).
// ------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cls {
    CInt,
    CName,
    CPunct,
}

/// Mirror of `needs_sep`: the h0 boundary rule.
fn needs_sep(left: Cls, b: char) -> bool {
    if is_digit(b) {
        left == Cls::CInt || left == Cls::CName
    } else if is_lower(b) {
        left == Cls::CName
    } else {
        false
    }
}

/// A flattened printer token: its printed piece plus its lexical class.
struct PTok {
    piece: String,
    class: Cls,
}

/// Mirror of `toks_word` — flatten one word into printer tokens (brackets
/// interspersed for quotations), threading into `out`.
fn toks_word(w: &Word, out: &mut Vec<PTok>) {
    match w {
        Word::PushInt(n) => out.push(PTok {
            piece: format!("{n}"),
            class: Cls::CInt,
        }),
        Word::Prim(p) => out.push(PTok {
            piece: prim_to_glyph(*p).to_string(),
            class: Cls::CPunct,
        }),
        Word::Call(cs) => out.push(PTok {
            piece: cs.iter().collect(),
            class: Cls::CName,
        }),
        Word::PushQuote(inner) => {
            out.push(PTok {
                piece: "[".to_string(),
                class: Cls::CPunct,
            });
            for iw in inner {
                toks_word(iw, out);
            }
            out.push(PTok {
                piece: "]".to_string(),
                class: Cls::CPunct,
            });
        }
    }
}

/// Mirror of `spec_print` = `render_h(None, toks_words(p))`.
fn twin_print(prog: &[Word]) -> String {
    let mut toks = Vec::new();
    for w in prog {
        toks_word(w, &mut toks);
    }
    let mut out = String::new();
    let mut prev: Option<Cls> = None;
    for t in &toks {
        let first = t.piece.chars().next().expect("token piece is non-empty");
        if let Some(p) = prev {
            if needs_sep(p, first) {
                out.push(' ');
            }
        }
        out.push_str(&t.piece);
        prev = Some(t.class);
    }
    out
}

// ------------------------------------------------------------------
// The lexer twin (mirror `lex`) and grouper twin (mirror `group_fold`).
// ------------------------------------------------------------------
#[derive(Clone, PartialEq, Eq)]
enum LTok {
    Int(i64),
    Name(Vec<char>),
    Glyph(Prim),
    Open,
    Close,
}

/// Mirror of `lex`: maximal-munch tokenizer. Returns None on any lex error
/// (unexpected char, `"`/`#` unsupported, i64 overflow), matching production's
/// error partition.
fn twin_lex(cs: &[char]) -> Option<Vec<LTok>> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < cs.len() {
        let c = cs[i];
        if is_ws(c) {
            i += 1;
        } else if is_digit(c) {
            let start = i;
            while i < cs.len() && is_digit(cs[i]) {
                i += 1;
            }
            let lit: String = cs[start..i].iter().collect();
            match lit.parse::<i64>() {
                Ok(n) => out.push(LTok::Int(n)),
                Err(_) => return None, // i64 overflow -> production errors
            }
        } else if is_lower(c) {
            let start = i;
            while i < cs.len() && is_namechar(cs[i]) {
                i += 1;
            }
            out.push(LTok::Name(cs[start..i].to_vec()));
        } else if c == '[' {
            out.push(LTok::Open);
            i += 1;
        } else if c == ']' {
            out.push(LTok::Close);
            i += 1;
        } else {
            match glyph_to_prim(c) {
                Some(p) => {
                    out.push(LTok::Glyph(p));
                    i += 1;
                }
                None => return None, // unexpected char / `"` / `#`
            }
        }
    }
    Some(out)
}

/// Mirror of `group_fold` over the token stream from the initial `[[]]` state.
/// Returns None on any unbalanced bracket, matching production's error partition.
fn twin_group(toks: &[LTok]) -> Option<Vec<Word>> {
    let mut levels: Vec<Vec<Word>> = vec![Vec::new()];
    for t in toks {
        match t {
            LTok::Open => levels.push(Vec::new()),
            LTok::Close => {
                if levels.len() <= 1 {
                    return None; // unmatched close
                }
                let inner = levels.pop().unwrap();
                levels.last_mut().unwrap().push(Word::PushQuote(inner));
            }
            LTok::Int(n) => levels.last_mut().unwrap().push(Word::PushInt(*n)),
            LTok::Name(s) => levels.last_mut().unwrap().push(Word::Call(s.clone())),
            LTok::Glyph(p) => levels.last_mut().unwrap().push(Word::Prim(*p)),
        }
    }
    if levels.len() != 1 {
        return None; // unclosed quote(s)
    }
    Some(levels.pop().unwrap())
}

/// Mirror of `spec_parse`.
fn twin_parse(s: &str) -> Result<Vec<Word>, ()> {
    let cs: Vec<char> = s.chars().collect();
    match twin_lex(&cs) {
        None => Err(()),
        Some(toks) => match twin_group(&toks) {
            Some(ws) => Ok(ws),
            None => Err(()),
        },
    }
}

// ------------------------------------------------------------------
// Generators (well-formed domain: non-negative ints, valid names).
// ------------------------------------------------------------------
const ALL_PRIMS: [Prim; 23] = [
    Prim::Dup, Prim::Drop, Prim::Swap, Prim::Rot, Prim::Over, Prim::Apply,
    Prim::Cat, Prim::Cons, Prim::Dip, Prim::Add, Prim::Sub, Prim::Mul, Prim::Div,
    Prim::Mod, Prim::Eq, Prim::Lt, Prim::If, Prim::PrimRec, Prim::Times,
    Prim::LinRec, Prim::Uncons, Prim::Fold, Prim::Xor,
];

fn name_strategy() -> impl Strategy<Value = Vec<char>> {
    proptest::collection::vec(
        prop_oneof![
            (b'a'..=b'z').prop_map(|b| b as char),
            (b'0'..=b'9').prop_map(|b| b as char),
        ],
        0..=2,
    )
    .prop_perturb(|rest, mut rng| {
        let first = (b'a' + (rng.next_u32() % 26) as u8) as char;
        let mut v = Vec::with_capacity(rest.len() + 1);
        v.push(first);
        v.extend(rest);
        v
    })
}

fn word_strategy() -> impl Strategy<Value = Word> {
    let leaf = prop_oneof![
        // Include the integer boundaries 0 and i64::MAX explicitly.
        prop_oneof![Just(0i64), Just(i64::MAX), 0i64..=i64::MAX].prop_map(Word::PushInt),
        prop::sample::select(ALL_PRIMS.to_vec()).prop_map(Word::Prim),
        name_strategy().prop_map(Word::Call),
    ];
    leaf.prop_recursive(4, 32, 4, |inner| {
        proptest::collection::vec(inner, 0..=4).prop_map(Word::PushQuote)
    })
}

fn program_strategy() -> impl Strategy<Value = Vec<Word>> {
    proptest::collection::vec(word_strategy(), 0..=8)
}

proptest! {
    // Widened coverage: run every property on many thousands of cases per test.
    // At ~20k cases the three proptests below explore far more of the
    // print/lex/group state space than the default 256 while keeping the crate's
    // `cargo test` wall-clock comfortably under the 2-minute budget.
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    /// Printer pin: production `print` equals the model twin, byte-for-byte,
    /// over the whole well-formed domain (all 23 glyphs, nested quotes, the h0
    /// separator cases, integer boundaries).
    #[test]
    fn twin_print_matches_production(p in program_strategy()) {
        prop_assert_eq!(print(&p), twin_print(&p));
    }

    /// Executable analogue of the proven `exec_roundtrip` (§22):
    /// `parse(print(p)) == Ok(p)` for every generated well-formed program.
    /// Also pins production `print` to the twin on the same program.
    #[test]
    fn production_roundtrip(p in program_strategy()) {
        let s = print(&p);
        prop_assert_eq!(s.clone(), twin_print(&p));
        prop_assert_eq!(parse(&s).map_err(|_| ()), Ok(p.clone()));
        // print is idempotent on its own canonical output: printing the parsed
        // AST reproduces the string byte-for-byte.
        let reparsed = parse(&s).expect("wf program must reparse");
        prop_assert_eq!(print(&reparsed), s);
    }

    /// Parser pin (canonical forms): the model twin recovers the AST from the
    /// production-printed string, and production recovers it from the twin's
    /// printed string. Both directions agree with the proved round-trip.
    #[test]
    fn twin_parse_roundtrip(p in program_strategy()) {
        let prod_s = print(&p);
        let twin_s = twin_print(&p);
        prop_assert_eq!(twin_s.clone(), prod_s.clone());
        prop_assert_eq!(twin_parse(&prod_s), Ok(p.clone()));
        prop_assert_eq!(parse(&twin_s).map_err(|_| ()), Ok(p.clone()));
    }

    /// Parser pin (arbitrary strings): production `parse` and the model twin
    /// `twin_parse` agree on the accept/reject partition AND on the parsed AST,
    /// over arbitrary strings biased toward syntactic characters.
    #[test]
    fn twin_parse_matches_production(
        s in proptest::collection::vec(
            prop_oneof![
                Just('['), Just(']'), Just(':'), Just('!'), Just('-'), Just('('),
                Just('$'), Just('|'), Just('&'), Just('.'), Just('>'), Just(' '),
                Just('"'), Just('#'), Just(')'),
                (b'0'..=b'9').prop_map(|b| b as char),
                (b'a'..=b'z').prop_map(|b| b as char),
                any::<char>(),
            ],
            0..40,
        ).prop_map(|v| v.into_iter().collect::<String>())
    ) {
        prop_assert_eq!(parse(&s).map_err(|_| ()), twin_parse(&s));
    }
}

/// Deterministic spot-check of the h0 adjacency case and every glyph, mirroring
/// the Verus non-vacuity audit (`p4_audit_all_glyphs`, `p4_audit_h0_adjacency`).
#[test]
fn twin_spot_checks() {
    // h0 (a Call ending in a digit) then int 0 must be separated: "h0 0".
    let prog = vec![Word::Call(vec!['h', '0']), Word::PushInt(0)];
    assert_eq!(twin_print(&prog), "h0 0");
    assert_eq!(print(&prog), twin_print(&prog));
    assert_eq!(twin_parse("h0 0"), Ok(prog.clone()));
    assert_eq!(parse("h0 0").map_err(|_| ()), Ok(prog));

    // 5 then h0 needs NO space: "5h0" (int stops at a letter).
    let prog2 = vec![Word::PushInt(5), Word::Call(vec!['h', '0'])];
    assert_eq!(twin_print(&prog2), "5h0");
    assert_eq!(print(&prog2), twin_print(&prog2));

    // Every glyph prints to its canonical char and round-trips.
    for p in ALL_PRIMS {
        let prog = vec![Word::Prim(p)];
        let s = twin_print(&prog);
        assert_eq!(print(&prog), s.clone());
        assert_eq!(twin_parse(&s), Ok(prog.clone()));
        assert_eq!(parse(&s).map_err(|_| ()), Ok(prog));
    }
}

// ==================================================================
// ADVERSARIAL / BOUNDARY suite (deterministic).
//
// Every case asserts that production and the proven-exec twin agree on BOTH the
// accept/reject partition AND the exact parsed AST / error class (`Err(())` is
// the single error class production and twin collapse to). Where a program is
// involved we additionally pin production `print` to the twin, byte-for-byte.
// ==================================================================

/// Assert production `parse` and the twin agree exactly on a string: same
/// accept/reject decision and, on accept, the identical AST.
fn agree_parse(s: &str) {
    assert_eq!(
        parse(s).map_err(|_| ()),
        twin_parse(s),
        "parse partition/AST disagreement on {s:?}"
    );
}

/// Assert production `print` and the twin agree byte-for-byte on a program.
fn agree_print(p: &[Word]) {
    assert_eq!(print(p), twin_print(p), "print disagreement on {p:?}");
}

#[test]
fn adversarial_i64_boundaries() {
    // i64::MAX and MAX-adjacent literals: the largest values that still fit.
    let max = i64::MAX; // 9_223_372_036_854_775_807  (19 digits)
    for n in [0i64, 1, 9, 10, max - 1, max] {
        let prog = vec![Word::PushInt(n)];
        agree_print(&prog);
        let s = print(&prog);
        agree_parse(&s);
        assert_eq!(parse(&s).map_err(|_| ()), Ok(prog));
    }

    // Exact-fit and just-over literals as raw source strings (maximal munch of
    // the digit run + the scan_digits overflow boundary).
    agree_parse("9223372036854775807"); // == i64::MAX  -> Ok
    agree_parse("9223372036854775808"); // == i64::MAX+1 -> overflow -> Err
    assert_eq!(parse("9223372036854775807").map_err(|_| ()), Ok(vec![Word::PushInt(max)]));
    assert!(parse("9223372036854775808").is_err());
    assert_eq!(twin_parse("9223372036854775808"), Err(()));

    // Digit runs of length 19 (may fit) and 20 (always overflows), straddling
    // the i64 bound. The all-nines 19-run overflows; the all-zeros run is 0.
    agree_parse("9999999999999999999"); // 19 nines  > i64::MAX -> Err
    agree_parse("1000000000000000000"); // 19 digits < i64::MAX -> Ok
    agree_parse("10000000000000000000"); // 20 digits -> overflow -> Err
    agree_parse("00000000000000000000"); // 20 zeros  -> value 0 fits -> Ok
    // Leading-zero padded max (still parses to i64::MAX).
    agree_parse("009223372036854775807");
    // A very long digit run cannot possibly fit.
    agree_parse(&"9".repeat(40));

    // Adjacent-in-stream literals (space-separated) around the boundary.
    agree_parse("9223372036854775807 9223372036854775808");
    agree_parse("9223372036854775806 1 2");
}

#[test]
fn adversarial_i64_min_printing() {
    // i64::MIN and negative ints are outside the parser's image but the printer
    // must render them totally (leading `-` + unsigned magnitude). Pin print,
    // then confirm re-parsing the printed form lands in the SAME partition for
    // production and twin (the `-` lexes as Sub; the magnitude of MIN overflows).
    for n in [i64::MIN, i64::MIN + 1, -1, -9, -10, -9223372036854775807] {
        let prog = vec![Word::PushInt(n)];
        agree_print(&prog);
        let s = print(&prog);
        agree_parse(&s);
    }

    // i64::MIN specifically prints as "-9223372036854775808"; re-parsing gives
    // Sub then an overflowing magnitude -> Err for both production and twin.
    let min_prog = vec![Word::PushInt(i64::MIN)];
    assert_eq!(print(&min_prog), "-9223372036854775808");
    assert_eq!(twin_print(&min_prog), "-9223372036854775808");
    assert!(parse("-9223372036854775808").is_err());
    assert_eq!(twin_parse("-9223372036854775808"), Err(()));

    // -1 prints "-1" and re-parses as [Sub, PushInt(1)] for both.
    let neg_prog = vec![Word::PushInt(-1)];
    assert_eq!(print(&neg_prog), "-1");
    agree_parse("-1");
    assert_eq!(
        twin_parse("-1"),
        Ok(vec![Word::Prim(Prim::Sub), Word::PushInt(1)])
    );
}

#[test]
fn adversarial_name_digit_adjacency() {
    // Names ending in every digit h0..h9, each followed by an int literal: the
    // h0 boundary rule (needs_sep after a Name whose successor is a digit) must
    // insert exactly one space so the round-trip is exact.
    for d in 0u8..=9 {
        let name = vec!['h', (b'0' + d) as char];
        let prog = vec![Word::Call(name.clone()), Word::PushInt(7)];
        agree_print(&prog);
        let s = print(&prog);
        // Must be separated: "hN 7".
        assert_eq!(s, format!("h{d} 7"));
        agree_parse(&s);
        assert_eq!(parse(&s).map_err(|_| ()), Ok(prog));

        // Int-then-name needs NO space (int stops at a letter): "7hN".
        let prog2 = vec![Word::PushInt(7), Word::Call(name.clone())];
        agree_print(&prog2);
        assert_eq!(print(&prog2), format!("7h{d}"));

        // Name-then-name needs a space (would otherwise merge): "hN hN".
        let prog3 = vec![Word::Call(name.clone()), Word::Call(name.clone())];
        agree_print(&prog3);
        let s3 = print(&prog3);
        agree_parse(&s3);
        assert_eq!(parse(&s3).map_err(|_| ()), Ok(prog3));
    }
}

#[test]
fn adversarial_deep_nesting() {
    // Deeply nested quotations: [[[...[]...]]] to depth 64.
    for depth in [1usize, 2, 8, 32, 64] {
        let mut prog = vec![Word::PushInt(0)];
        for _ in 0..depth {
            prog = vec![Word::PushQuote(prog)];
        }
        agree_print(&prog);
        let s = print(&prog);
        agree_parse(&s);
        assert_eq!(parse(&s).map_err(|_| ()), Ok(prog.clone()));
        // The printed form is exactly `depth` opens, then "0", then `depth` closes.
        assert_eq!(s, format!("{}0{}", "[".repeat(depth), "]".repeat(depth)));
    }

    // Empty nested quotations of increasing depth, e.g. "[[[]]]".
    for depth in [1usize, 2, 16, 48] {
        let mut prog = vec![];
        for _ in 0..depth {
            prog = vec![Word::PushQuote(prog)];
        }
        agree_print(&prog);
        let s = print(&prog);
        agree_parse(&s);
        assert_eq!(parse(&s).map_err(|_| ()), Ok(prog));
    }
}

#[test]
fn adversarial_empty_and_whitespace() {
    // Empty program prints to "" and parses back to [].
    let empty: Vec<Word> = vec![];
    agree_print(&empty);
    assert_eq!(print(&empty), "");
    agree_parse("");
    assert_eq!(parse("").map_err(|_| ()), Ok(vec![]));

    // Whitespace-only and whitespace-collapsing inputs: all forms of ws are
    // skipped identically by production and twin.
    for s in [
        " ", "   ", "\t", "\n", "\r", "\r\n", " \t\n\r ", "  1  2  ", "\t1\n2\r3",
        "  [  1  2  ]  ", "1\n\n\n2", " : ~ + ",
    ] {
        agree_parse(s);
    }
    // Collapsing: many spaces between two ints parse to the same AST as one.
    assert_eq!(
        parse("1          2").map_err(|_| ()),
        parse("1 2").map_err(|_| ())
    );
    assert_eq!(twin_parse("1          2"), twin_parse("1 2"));
}

#[test]
fn adversarial_malformed_inputs() {
    // Every malformed / rejected input: production and twin must agree it is
    // rejected (Err) — same reject partition, single error class.
    let rejects = [
        "[",            // unclosed open
        "[[",           // two unclosed opens
        "]",            // stray close
        "]]",           // two stray closes
        "1]",           // close with nothing open
        "[1",           // unclosed with content
        "[1]]",         // extra close
        "[[1]",         // missing close
        "\"",           // bare double-quote (unsupported char)
        "\"abc\"",      // quoted string (unsupported)
        "#",            // hash (unsupported char)
        "# comment",    // hash-comment (unsupported)
        "1 # 2",        // hash mid-stream
        "A",            // uppercase (not a namechar and not a glyph)
        "Abc",          // uppercase-led name
        "aBc",          // uppercase mid-name
        "\u{00e9}",     // multibyte 'é'
        "a\u{00e9}",    // name then 'é'
        "\u{1F600}",    // multibyte '😀'
        "[\u{1F600}]",  // emoji inside quote
        "1\u{00e9}2",   // 'é' between ints
        "`",            // backtick (unsupported)
        "{",            // brace (unsupported)
        "}",            // brace (unsupported)
        "\\",           // backslash (unsupported)
    ];
    for s in rejects {
        agree_parse(s);
        assert!(parse(s).is_err(), "expected production reject for {s:?}");
        assert_eq!(twin_parse(s), Err(()), "expected twin reject for {s:?}");
    }

    // A few well-formed inputs interleaved to confirm agree_parse also pins
    // ACCEPTED cases (not just rejects).
    for s in ["[]", "[1]", "1 2 3", ":~+", "[[1]2]", "abc 1", "[a][b]"] {
        agree_parse(s);
        assert!(parse(s).is_ok(), "expected production accept for {s:?}");
    }
}
