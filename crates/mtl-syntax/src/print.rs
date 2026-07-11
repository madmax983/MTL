//! Canonical pretty-printer for MTL.
//!
//! Produces the **canonical** textual form of a program with *minimal*
//! whitespace: exactly one space is inserted between two consecutive printed
//! tokens if and only if their bare concatenation would re-lex incorrectly.
//! Otherwise tokens are written adjacent (spec §2.2: `:!`, not `: !`).
//!
//! The printer is **total** and never panics — including on `PushInt(i64::MIN)`
//! and negative integers, which are outside the parser's image but can be
//! produced by the interpreter.

use crate::ast::{prim_to_glyph, Word};

/// Lexical class of an emitted token, tracked to decide token boundaries.
///
/// The pure char-class-of-the-boundary rule is insufficient: a name lexeme
/// (`[a-z][a-z0-9]*`) may END in a digit (e.g. `h0`), so the last char alone
/// cannot tell an int token from a name token. `Int` and `Name` are therefore
/// distinguished here even though both can end in a digit.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tok {
    /// An integer literal (may be negative when printed from interpreter output).
    Int,
    /// A named word `[a-z][a-z0-9]*`.
    Name,
    /// A primitive glyph or a bracket — a single self-delimiting punctuation char.
    Punct,
}

/// Pretty-print a program into its canonical textual form.
pub fn print(program: &[Word]) -> String {
    let mut out = String::new();
    // Class of the most-recently emitted token (None before the first).
    let mut last: Option<Tok> = None;
    for w in program {
        emit_word(&mut out, &mut last, w);
    }
    out
}

/// Emit one word into `out`, threading `last` so nested quotations share the
/// same boundary bookkeeping.
fn emit_word(out: &mut String, last: &mut Option<Tok>, w: &Word) {
    match w {
        Word::PushInt(n) => {
            // Negative ints (e.g. i64::MIN) print with a leading `-`. As a LEFT
            // token their class is still `Int`; as a RIGHT token their first
            // char `-` is punctuation, handled by `needs_separator`. Total:
            // `format!` never panics.
            let s = format!("{n}");
            append_token(out, last, &s, Tok::Int);
        }
        Word::Prim(p) => {
            let mut buf = [0u8; 4];
            let s = prim_to_glyph(*p).encode_utf8(&mut buf);
            append_token(out, last, s, Tok::Punct);
        }
        Word::Call(chars) => {
            let s: String = chars.iter().collect();
            append_token(out, last, &s, Tok::Name);
        }
        Word::PushQuote(inner) => {
            append_token(out, last, "[", Tok::Punct);
            for iw in inner {
                emit_word(out, last, iw);
            }
            append_token(out, last, "]", Tok::Punct);
        }
    }
}

/// Append a printed token `piece` (of class `class`) to `out`, inserting a
/// single separating space first iff the boundary would otherwise re-lex wrong.
fn append_token(out: &mut String, last: &mut Option<Tok>, piece: &str, class: Tok) {
    // Empty pieces cannot occur (every token has ≥1 char), but guard anyway for
    // totality.
    let Some(b) = piece.chars().next() else {
        return;
    };
    if let Some(left) = *last {
        if needs_separator(left, b) {
            out.push(' ');
        }
    }
    out.push_str(piece);
    *last = Some(class);
}

/// The boundary rule. `left` is the class of the token already emitted; `b` is
/// the first char of the piece about to be emitted.
///
/// * if `b` is an ascii digit: separate iff `left` is `Int` or `Name` — either
///   alphanumeric lexeme would absorb the digit.
/// * else if `b` is `[a-z]`: separate iff `left` is `Name` — a name would
///   absorb the letter; an int stops at a letter, so `5a` needs no space.
/// * else (`b` is punctuation/bracket): never separate.
fn needs_separator(left: Tok, b: char) -> bool {
    if b.is_ascii_digit() {
        left == Tok::Int || left == Tok::Name
    } else if b.is_ascii_lowercase() {
        left == Tok::Name
    } else {
        false
    }
}
