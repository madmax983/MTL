//! P4 differential oracle: pins the production parser/printer to an EXECUTABLE
//! twin of the proved Verus `Seq<char>` model in `proofs/p4_verus.rs`.
//!
//! The Verus spec functions cannot be called from cargo, so this file is a
//! plain-Rust mirror of them — line-for-line faithful to the model's structure:
//!
//!   * `twin_print`  mirrors `spec_print` = `render_h(None, toks_words(p))`
//!     (the token-flattening + the h0 separator rule `needs_sep`).
//!   * `twin_parse`  mirrors `spec_parse` = `group_fold(lex(cs), [[]])`
//!     (maximal-munch lexer + the `[`/`]` quotation-stack grouper).
//!
//! The proptests then assert that PRODUCTION `print`/`parse` agree with this
//! independent oracle. This is the P2 oracle-pin pattern applied to P4: the
//! machine-checked model (proofs/p4_verus.rs) states the theorem; these tests
//! pin the real Rust code to that same model.
//!
//! NOTE on integers: the Verus model treats a digit run as an unbounded natural
//! (round-trip holds for every n >= 0). The executable twin additionally applies
//! production's i64 bound (a digit run that overflows i64 is an error), so the
//! twin matches production byte-for-byte on ALL strings while coinciding with
//! the model on the well-formed domain (values 0..=i64::MAX).

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
    /// Printer pin: production `print` equals the model twin, byte-for-byte,
    /// over the whole well-formed domain (all 23 glyphs, nested quotes, the h0
    /// separator cases, integer boundaries).
    #[test]
    fn twin_print_matches_production(p in program_strategy()) {
        prop_assert_eq!(print(&p), twin_print(&p));
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
