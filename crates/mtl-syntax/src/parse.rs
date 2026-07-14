//! Lexer + parser for MTL surface syntax.
//!
//! The whole pipeline is **total**: it never panics and never unwraps on user
//! input. Errors are values ([`ParseError`]), returned via [`Result`].
//!
//! ## Integer literal rule (supersedes the spec's `-?[0-9]+`)
//!
//! Per the spec-review decision, integer literals are **unsigned** `[0-9]+`
//! with maximal munch on digits. `-` is *always* the [`Prim::Sub`] primitive;
//! negatives are constructed operationally. Consequences:
//!
//! * `1-2`  → `[PushInt(1), Prim(Sub), PushInt(2)]`
//! * `-7`   → `[Prim(Sub), PushInt(7)]`
//!
//! This means the parser's *image* contains only non-negative `PushInt` values
//! — the basis of the P4 well-formedness domain (see crate docs).
//!
//! # Correspondence to the machine-proven `exec_parse`
//!
//! This is the **byte-identical production mirror** of the machine-proven
//! `exec_*` functions in `crates/mtl-syntax/proofs/p4_verus.rs`, §17–§21. The
//! scanning core is a near-mechanical transcription:
//!
//! | this file        | proofs/p4_verus.rs        |
//! |------------------|---------------------------|
//! | [`scan_digits`]  | `scan_digits` (§17)       |
//! | [`scan_name`]    | `scan_name`   (§17)       |
//! | the `[`/`]` stack | `exec_group`  (§20)      |
//! | maximal-munch loop | `exec_lex`  (§18)       |
//!
//! Two production concerns constrain the mirror away from the exec side's exact
//! shape, and both are deliberate:
//!
//! * **Error positions/kinds.** `exec_parse` returns only a coarse `EErr`; the
//!   production parser must report an exact byte offset and a classified
//!   [`ParseErrorKind`]. Because every valid MTL lexeme is ASCII (one byte), any
//!   char the parser actually reaches lies after only ASCII, so its byte offset
//!   equals its `Vec<char>` index — index-based scanning is byte-faithful.
//! * **Single left-to-right pass.** `exec_lex` and `exec_group` are two passes;
//!   splitting production likewise would change *which* error is reported first
//!   (e.g. `]#` reports the `]` at pos 0, not the `#`). To keep byte-identical
//!   error order, lexing and grouping stay interleaved in one driver loop while
//!   reusing the exec scanners and the exec grouper's level-stack shape.
//!
//! Integer accumulation uses the same checked `v*10 + d` overflow test as
//! `scan_digits` (against `i64::MAX`), replacing `str::parse::<i64>()`. The
//! differential oracle in `tests/p4_model_twin.rs` enforces the correspondence.

use crate::ast::{glyph_to_prim, Program, Word};
use core::fmt;

/// A parse error: a byte position plus a classified [`ParseErrorKind`].
///
/// Kept structured (position + kind) rather than a flat string so a future
/// validator can consume exact positions and expected/found information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// Byte offset into the source where the error was detected.
    pub pos: usize,
    /// The classified reason for the error.
    pub kind: ParseErrorKind,
}

/// The classified reason a parse failed. Extensible on purpose — do not
/// collapse into a single string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// A `[0-9]+` literal exceeds `i64::MAX`.
    IntOverflow {
        /// The offending digit run, verbatim.
        literal: String,
    },
    /// A character that begins no valid token.
    UnexpectedChar {
        /// The character found.
        found: char,
    },
    /// A `]` with no matching open `[`.
    UnexpectedCloseBracket,
    /// A `[` that is never closed.
    UnclosedQuote {
        /// Byte offset of the outermost unclosed `[`.
        opened_at: usize,
    },
    /// A `"` — the spec §2 lists string literals, but the exec core has no
    /// string variant, so they are unsupported here.
    StringUnsupported,
    /// A `#` — `#f[...]` definitions are deferred out of v0.
    DefinitionUnsupported,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at byte {}: {}", self.pos, self.kind)
    }
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseErrorKind::IntOverflow { literal } => write!(
                f,
                "integer literal `{literal}` exceeds the i64 range (max {})",
                i64::MAX
            ),
            ParseErrorKind::UnexpectedChar { found } => {
                write!(
                    f,
                    "unexpected character `{found}`; expected a digit, glyph, name, `[`, or `]`"
                )
            }
            ParseErrorKind::UnexpectedCloseBracket => {
                write!(f, "unexpected `]` with no matching open `[`")
            }
            ParseErrorKind::UnclosedQuote { opened_at } => {
                write!(
                    f,
                    "unclosed quotation: `[` opened at byte {opened_at} was never closed"
                )
            }
            ParseErrorKind::StringUnsupported => write!(
                f,
                "string literals are unsupported: the executable core has no string value"
            ),
            ParseErrorKind::DefinitionUnsupported => {
                write!(
                    f,
                    "word definitions (`#f[...]`) are unsupported in this version"
                )
            }
        }
    }
}

/// Maximal-munch digit scanner. Mirror of `scan_digits` (proofs §17).
///
/// Starting at `cs[i]` (assumed a digit), consume the maximal run of ascii
/// digits and return `(end_index, value)`: `Some(v)` if the run's value fits
/// `i64`, or `None` on overflow. Overflow is detected with the same checked
/// `v*10 + d` test the proof uses (`i64::MAX == 922_337_203_685_477_580*10 + 7`),
/// replacing `str::parse::<i64>()`.
fn scan_digits(cs: &[char], i: usize) -> (usize, Option<i64>) {
    let mut j = i;
    let mut acc: Option<i64> = Some(0i64);
    while j < cs.len() && cs[j].is_ascii_digit() {
        // Digit value 0..=9 (cs[j] is an ascii digit).
        let d = (cs[j] as u64) - ('0' as u64);
        acc = match acc {
            None => None,
            Some(v) => {
                if v > 922_337_203_685_477_580 || (v == 922_337_203_685_477_580 && d > 7) {
                    None
                } else {
                    Some(v * 10 + d as i64)
                }
            }
        };
        j += 1;
    }
    (j, acc)
}

/// Maximal-munch name scanner. Mirror of `scan_name` (proofs §17).
///
/// Starting at `cs[i]` (assumed `[a-z]`), consume the maximal run of
/// `[a-z0-9]` and return the end index.
fn scan_name(cs: &[char], i: usize) -> usize {
    let mut j = i;
    while j < cs.len() && (cs[j].is_ascii_lowercase() || cs[j].is_ascii_digit()) {
        j += 1;
    }
    j
}

/// Parse MTL source text into a [`Program`].
///
/// Total: returns `Err(ParseError)` for any malformed input rather than
/// panicking.
pub fn parse(src: &str) -> Result<Program, ParseError> {
    // A stack of open quotation levels. The bottom (index 0) is the top-level
    // program. Each open `[` pushes a level, remembering its position. This is
    // the level-stack of `exec_group` (proofs §20).
    let mut levels: Vec<Vec<Word>> = vec![Vec::new()];
    // Positions of the currently-open `[`s, parallel to `levels[1..]`.
    let mut open_offsets: Vec<usize> = Vec::new();

    // Index-based `Vec<char>` scanning (mirror of the exec `cs: Vec<char>`).
    // Every valid lexeme is ASCII, so any position the loop reaches sits after
    // only ASCII: its char index equals its byte offset, so `i` is a faithful
    // byte position for every reported error (see module docs).
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            // whitespace: skip.
            ' ' | '\t' | '\n' | '\r' => {
                i += 1;
            }
            // open quotation: push a fresh level (exec_group ETOpen arm).
            '[' => {
                levels.push(Vec::new());
                open_offsets.push(i);
                i += 1;
            }
            // close quotation: pop innermost into a PushQuote (ETClose arm).
            ']' => {
                if open_offsets.is_empty() {
                    return Err(ParseError {
                        pos: i,
                        kind: ParseErrorKind::UnexpectedCloseBracket,
                    });
                }
                // Safe: levels always has one more entry than open_offsets.
                let inner = levels.pop().unwrap_or_default();
                open_offsets.pop();
                if let Some(parent) = levels.last_mut() {
                    parent.push(Word::PushQuote(inner));
                }
                i += 1;
            }
            // integer literal: maximal run of ascii digits (exec `scan_digits`).
            '0'..='9' => {
                let start = i;
                let (j, value) = scan_digits(&chars, i);
                match value {
                    Some(n) => {
                        if let Some(level) = levels.last_mut() {
                            level.push(Word::PushInt(n));
                        }
                    }
                    None => {
                        let literal: String = chars[start..j].iter().collect();
                        return Err(ParseError {
                            pos: start,
                            kind: ParseErrorKind::IntOverflow { literal },
                        });
                    }
                }
                i = j;
            }
            // string literal: unsupported.
            '"' => {
                return Err(ParseError {
                    pos: i,
                    kind: ParseErrorKind::StringUnsupported,
                });
            }
            // definition: unsupported.
            '#' => {
                return Err(ParseError {
                    pos: i,
                    kind: ParseErrorKind::DefinitionUnsupported,
                });
            }
            // named word: [a-z][a-z0-9]* (exec `scan_name`).
            'a'..='z' => {
                let start = i;
                let j = scan_name(&chars, i);
                let name: Vec<char> = chars[start..j].iter().copied().collect();
                if let Some(level) = levels.last_mut() {
                    level.push(Word::Call(name));
                }
                i = j;
            }
            // glyph (single char, self-delimiting) or unexpected.
            _ => {
                if let Some(p) = glyph_to_prim(c) {
                    if let Some(level) = levels.last_mut() {
                        level.push(Word::Prim(p));
                    }
                    i += 1;
                } else {
                    return Err(ParseError {
                        pos: i,
                        kind: ParseErrorKind::UnexpectedChar { found: c },
                    });
                }
            }
        }
    }

    // EOF with any unclosed `[` → error, reported at the OUTERMOST open `[`.
    if let Some(&opened_at) = open_offsets.first() {
        return Err(ParseError {
            pos: opened_at,
            kind: ParseErrorKind::UnclosedQuote { opened_at },
        });
    }

    // levels[0] is the top-level program; the stack is otherwise empty.
    Ok(levels.pop().unwrap_or_default())
}
