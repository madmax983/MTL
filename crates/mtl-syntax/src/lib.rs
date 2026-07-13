//! # mtl-syntax
//!
//! Lexer, parser, and canonical pretty-printer for **MTL**, a concatenative
//! stack language whose executable core lives in `mtl-core`.
//!
//! ## The P4 round-trip property
//!
//! **P4** is the property that parsing and printing are mutually consistent:
//! printing an AST and parsing the result recovers the original AST. Formally,
//! over the *well-formed* domain (below):
//!
//! ```text
//! for all well-formed programs p:   parse(&print(&p)) == Ok(p)
//! ```
//!
//! together with text canonicalization/idempotence: for any `src` that parses,
//! `print(parse(src))` is canonical, so re-parsing it yields the same AST and
//! re-printing is idempotent.
//!
//! ## The well-formed AST domain
//!
//! A program is **well-formed** iff every [`Word::PushInt(n)`] has `n >= 0`.
//! This is exactly the *image of the parser*: integer literals are unsigned
//! `[0-9]+` (see [`parse`]), so the parser can never produce a negative
//! `PushInt`. Negative integers exist only as interpreter-produced values and
//! are outside P4's round-trip domain; the [`print`]er still handles them
//! totally (it never panics), but they do not round-trip through [`parse`].
//!
//! [`Word::PushInt(n)`]: crate::ast::Word::PushInt

pub mod ast;
pub mod manifest;
pub mod parse;
pub mod print;

pub use ast::{Prim, Program, Word};
pub use parse::{parse, ParseError, ParseErrorKind};
pub use print::print;
