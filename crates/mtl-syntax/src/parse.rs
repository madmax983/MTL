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

/// Parse MTL source text into a [`Program`].
///
/// Total: returns `Err(ParseError)` for any malformed input rather than
/// panicking.
pub fn parse(src: &str) -> Result<Program, ParseError> {
    // A stack of open quotation levels. The bottom (index 0) is the top-level
    // program. Each open `[` pushes a level, remembering its byte offset.
    let mut levels: Vec<Vec<Word>> = vec![Vec::new()];
    // Byte offsets of the currently-open `[`s, parallel to `levels[1..]`.
    let mut open_offsets: Vec<usize> = Vec::new();

    // char_indices gives (byte_offset, char); collect so we can do maximal
    // munch by indexing.
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (offset, c) = chars[i];
        match c {
            // whitespace: skip.
            ' ' | '\t' | '\n' | '\r' => {
                i += 1;
            }
            // open quotation.
            '[' => {
                levels.push(Vec::new());
                open_offsets.push(offset);
                i += 1;
            }
            // close quotation: pop innermost into a PushQuote.
            ']' => {
                if open_offsets.is_empty() {
                    return Err(ParseError {
                        pos: offset,
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
            // integer literal: maximal run of ascii digits.
            '0'..='9' => {
                let start = i;
                let start_off = offset;
                let mut j = i;
                while j < chars.len() && chars[j].1.is_ascii_digit() {
                    j += 1;
                }
                let literal: String = chars[start..j].iter().map(|&(_, ch)| ch).collect();
                match literal.parse::<i64>() {
                    Ok(n) => {
                        if let Some(level) = levels.last_mut() {
                            level.push(Word::PushInt(n));
                        }
                    }
                    Err(_) => {
                        return Err(ParseError {
                            pos: start_off,
                            kind: ParseErrorKind::IntOverflow { literal },
                        });
                    }
                }
                i = j;
            }
            // string literal: unsupported.
            '"' => {
                return Err(ParseError {
                    pos: offset,
                    kind: ParseErrorKind::StringUnsupported,
                });
            }
            // definition: unsupported.
            '#' => {
                return Err(ParseError {
                    pos: offset,
                    kind: ParseErrorKind::DefinitionUnsupported,
                });
            }
            // named word: [a-z][a-z0-9]*
            'a'..='z' => {
                let start = i;
                let mut j = i;
                while j < chars.len()
                    && (chars[j].1.is_ascii_lowercase() || chars[j].1.is_ascii_digit())
                {
                    j += 1;
                }
                let name: Vec<char> = chars[start..j].iter().map(|&(_, ch)| ch).collect();
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
                        pos: offset,
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
