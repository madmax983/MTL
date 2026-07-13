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
//!
//! # Correspondence to the machine-proven `exec_print`
//!
//! This is the **byte-identical production mirror** of the machine-proven
//! `exec_*` functions in `crates/mtl-syntax/proofs/p4_verus.rs`, §15
//! ("Executable printer"). The structure is a near-mechanical transcription:
//!
//! | this file            | proofs/p4_verus.rs §15    |
//! |----------------------|---------------------------|
//! | [`Cls`]              | `Cls`                     |
//! | [`needs_sep`]        | `exec_needs_sep`          |
//! | [`digit_char`]       | `exec_digit_char`         |
//! | [`digits`]           | `exec_digits`             |
//! | [`emit_token`]       | `emit_token`              |
//! | [`emit_word`]        | `exec_emit_word`          |
//! | [`print`]            | `exec_print`              |
//!
//! The production side carries `Word`/`i64`/`String`; the proof side carries
//! `EWord`/`i64`/`Vec<char>`. The one intentional superset: `exec_print`'s
//! domain is `wf_words` (non-negative `PushInt` only), whereas production must
//! also render *negative* ints (`i64::MIN`) that the interpreter can produce —
//! handled by a leading `-` plus the digits of the unsigned magnitude, keeping
//! the token's boundary class `Int` exactly as the old code did.
//!
//! The differential oracle in `tests/p4_model_twin.rs` enforces the
//! correspondence between this code and the proven model.

use crate::ast::{prim_to_glyph, Word};

/// Lexical class of an emitted token, tracked to decide token boundaries.
///
/// The pure char-class-of-the-boundary rule is insufficient: a name lexeme
/// (`[a-z][a-z0-9]*`) may END in a digit (e.g. `h0`), so the last char alone
/// cannot tell an int token from a name token. `Int` and `Name` are therefore
/// distinguished here even though both can end in a digit.
///
/// Mirrors `Cls` (`CInt`/`CName`/`CPunct`) in proofs/p4_verus.rs §15.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cls {
    /// An integer literal (may be negative when printed from interpreter output).
    Int,
    /// A named word `[a-z][a-z0-9]*`.
    Name,
    /// A primitive glyph or a bracket — a single self-delimiting punctuation char.
    Punct,
}

/// The boundary rule. `left` is the class of the token already emitted; `b` is
/// the first char of the piece about to be emitted.
///
/// * if `b` is an ascii digit: separate iff `left` is `Int` or `Name` — either
///   alphanumeric lexeme would absorb the digit.
/// * else if `b` is `[a-z]`: separate iff `left` is `Name` — a name would
///   absorb the letter; an int stops at a letter, so `5a` needs no space.
/// * else (`b` is punctuation/bracket): never separate.
///
/// Mirror of `exec_needs_sep` (proofs/p4_verus.rs §15).
fn needs_sep(left: Cls, b: char) -> bool {
    if b.is_ascii_digit() {
        left == Cls::Int || left == Cls::Name
    } else if b.is_ascii_lowercase() {
        left == Cls::Name
    } else {
        false
    }
}

/// Decimal glyph for a single digit `k < 10`. Mirror of `exec_digit_char`.
fn digit_char(k: u64) -> char {
    match k {
        0 => '0',
        1 => '1',
        2 => '2',
        3 => '3',
        4 => '4',
        5 => '5',
        6 => '6',
        7 => '7',
        8 => '8',
        _ => '9',
    }
}

/// Emit the decimal representation of `n`, most-significant digit first.
///
/// Mirror of `exec_digits` (proofs/p4_verus.rs §15): the same recursive
/// `n < 10` base case / `digits(n / 10) ++ [digit_char(n % 10)]` step, using
/// manual digit emission instead of `format!`/`to_string`.
fn digits(n: u64) -> Vec<char> {
    if n < 10 {
        let mut v: Vec<char> = Vec::new();
        v.push(digit_char(n));
        v
    } else {
        let mut v = digits(n / 10);
        v.push(digit_char(n % 10));
        v
    }
}

/// Append a printed token `piece` (of class `class`) to `out`, inserting one
/// separating space first iff the boundary rule demands it, then updating the
/// running boundary class `last`.
///
/// Mirror of `emit_token` (proofs/p4_verus.rs §15). `out` is a `Vec<char>` and
/// growth is index-based, matching the exec loop.
fn emit_token(out: &mut Vec<char>, last: &mut Option<Cls>, piece: &[char], class: Cls) {
    // Empty pieces cannot occur (every token has ≥1 char), but guard anyway for
    // totality — matches the old printer's early return.
    if piece.is_empty() {
        return;
    }
    let b = piece[0];
    let need = match *last {
        None => false,
        Some(l) => needs_sep(l, b),
    };
    if need {
        out.push(' ');
    }
    let mut j = 0;
    while j < piece.len() {
        out.push(piece[j]);
        j += 1;
    }
    *last = Some(class);
}

/// Emit one word into `out`, threading `last` so nested quotations share the
/// same boundary bookkeeping.
///
/// Mirror of `exec_emit_word` (proofs/p4_verus.rs §15), arm-for-arm. The one
/// production-only extension is negative-int rendering (see module docs).
fn emit_word(out: &mut Vec<char>, last: &mut Option<Cls>, w: &Word) {
    match w {
        Word::PushInt(n) => {
            // Non-negative ints mirror `exec_emit_word`'s `exec_digits(i)`.
            // Negative ints (e.g. i64::MIN) print a leading `-` then the digits
            // of the unsigned magnitude — `unsigned_abs` is total on i64::MIN.
            // As a LEFT token their class is still `Int`; as a RIGHT token their
            // first char `-` is punctuation, handled by `needs_sep`.
            let mut piece: Vec<char> = Vec::new();
            if *n < 0 {
                piece.push('-');
            }
            piece.extend(digits(n.unsigned_abs()));
            emit_token(out, last, &piece, Cls::Int);
        }
        Word::Prim(p) => {
            let g = prim_to_glyph(*p);
            let piece = [g];
            emit_token(out, last, &piece, Cls::Punct);
        }
        Word::Call(chars) => {
            emit_token(out, last, chars, Cls::Name);
        }
        Word::PushQuote(inner) => {
            emit_token(out, last, &['['], Cls::Punct);
            for iw in inner {
                emit_word(out, last, iw);
            }
            emit_token(out, last, &[']'], Cls::Punct);
        }
    }
}

/// Pretty-print a program into its canonical textual form.
///
/// Mirror of `exec_print` (proofs/p4_verus.rs §15): accumulate into a
/// `Vec<char>` threading a `None` initial boundary class, then materialize the
/// `String`.
pub fn print(program: &[Word]) -> String {
    let mut out: Vec<char> = Vec::new();
    // Class of the most-recently emitted token (None before the first).
    let mut last: Option<Cls> = None;
    for w in program {
        emit_word(&mut out, &mut last, w);
    }
    out.into_iter().collect()
}
