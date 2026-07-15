//! Interpreter-validation harness for the MTL benchmark corpus.
//!
//! The two crates `mtl-syntax` (parser) and `mtl-core` (interpreter) share no
//! types, so this crate provides:
//!   * [`conv`] — convert a parsed `mtl_syntax::Word` into an executable
//!     `mtl_core::interp::Word` (the 23 `Prim` variants map by name).
//!   * [`load_solution`] — read a corpus `solution.mtl` file from disk, strip a
//!     single trailing newline, and parse it into an executable program.
//!
//! The integration test `tests/corpus.rs` reads the ACTUAL solution files (so
//! the token-counted artifact and the validated program are the same bytes),
//! executes each on the real interpreter against per-task input/output vectors,
//! and asserts the terminal `Outcome::Halt(expected)`.

use std::path::Path;

use mtl_core::interp::{run as interp_run, Outcome, Value, Vm, Word as IWord};
use mtl_syntax::{parse, ParseError, Word};

pub use mtl_arena::Engine;

/// Generate the syntax-`Prim` → interp-`Prim` opcode map from the checked
/// manifest's canonical rows (`mtl_syntax::for_each_primitive!`). This is the
/// codegen from issue #46: the 23-arm match is no longer hand-written here, so
/// the `conv` opcode map cannot drift from the manifest.
macro_rules! define_prim_opcode_map {
    ( $( ($idx:expr, $name:ident, $glyph:literal, $arity:literal, $eff:literal) ),* $(,)? ) => {
        /// Map a syntax primitive to its interp opcode. Generated from the manifest.
        #[inline]
        fn prim_to_iprim(p: mtl_syntax::Prim) -> mtl_core::interp::Prim {
            match p {
                $( mtl_syntax::Prim::$name => mtl_core::interp::Prim::$name ),*
            }
        }
    };
}

mtl_syntax::for_each_primitive!(define_prim_opcode_map);

/// Convert one parsed syntax word into an executable interpreter word.
///
/// `PushInt` maps straight across, `PushQuote` recurses, `Call(Vec<char>)`
/// becomes `Call(String)`, and the 23 primitives map by name via the
/// manifest-generated [`prim_to_iprim`] opcode map.
pub fn conv(w: &Word) -> IWord {
    match w {
        Word::PushInt(n) => IWord::PushInt(*n),
        Word::PushQuote(body) => IWord::PushQuote(body.iter().map(conv).collect()),
        Word::Call(chars) => IWord::Call(chars.iter().collect::<String>()),
        Word::Prim(p) => IWord::Prim(prim_to_iprim(*p)),
    }
}

/// Convert a whole parsed program into an executable program.
pub fn conv_program(prog: &[Word]) -> Vec<IWord> {
    prog.iter().map(conv).collect()
}

/// Run `prog` against `initial_stack` on the selected [`Engine`], bounded by
/// `fuel`, and return the reference-typed [`Outcome`].
///
/// The DEFAULT engine is the arena ([`Engine::Arena`]); [`Engine::Interp`] keeps
/// the reference interpreter reachable as the differential anchor. Both paths
/// produce the identically-shaped `interp::Outcome`, so the corpus gate compares
/// the same value regardless of engine — the arena flip is byte-for-byte
/// observationally invisible here (the differential oracle proves this across
/// 148 cases).
pub fn run_program(engine: Engine, prog: &[IWord], initial_stack: &[Value], fuel: u64) -> Outcome {
    match engine {
        Engine::Interp => interp_run(Vm::with_stack(initial_stack.to_vec(), prog.to_vec()), fuel),
        Engine::Arena => {
            let arena_prog = mtl_arena::prog_from_interp_with_stack(initial_stack, prog);
            mtl_arena::run_arena(&arena_prog, fuel)
                .outcome()
                .into_interp()
        }
    }
}

/// Decode a SIGNED input spec into an initial VM stack (bottom .. top).
///
/// This is a HARNESS-LEVEL input decoder — NOT the frozen program lexer. The
/// program text is still parsed by `mtl-syntax`'s unsigned Option-A lexer
/// (`-` always lexes to the `Sub` primitive, spec §2.3); only the `--input`
/// value flows through here. Because it constructs `Value`s directly, it can
/// express negative scalars and negative list elements that the program lexer
/// cannot — exactly the constructed-stack path the Rust tests already seed via
/// `Vm::with_stack` / `run_program` (mirrors `int_list` / `cell_to_value` in
/// `bench/validate/tests/sealed.rs`).
///
/// Syntax (a SIGNED superset of the old unsigned prepended-literal input):
///   * whitespace-separated TOP-LEVEL items, each pushed in order so item `i`
///     lands at stack position `i` (bottom .. top) — matching how the sealed
///     `args` array seeds `initial_stack`;
///   * an item is either a signed integer (`5`, `-24`) or a bracketed list
///     (`[-5 -2 -8 -1]`, `[]`, `[5 2]`) whose elements are themselves items
///     (nesting is supported, though the sealed vectors only need one level).
///
/// A flat integer list decodes to `Value::Quote(vec![PushInt(..), ..])`,
/// byte-identical to `int_list`, so the constructed stack matches the
/// Rust-test path exactly. The empty string decodes to an empty stack.
pub fn parse_input_stack(s: &str) -> Result<Vec<Value>, String> {
    let toks = tokenize_input(s);
    let mut pos = 0usize;
    let mut out = Vec::new();
    while pos < toks.len() {
        let w = parse_input_word(&toks, &mut pos)?;
        out.push(word_to_value(w)?);
    }
    Ok(out)
}

/// Bracket-aware whitespace tokenizer for the input spec. `[` and `]` are
/// standalone tokens; every other whitespace-delimited run is a number token.
fn tokenize_input(s: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '[' | ']' => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
                toks.push(c.to_string());
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks
}

/// Parse one input item as an interpreter `Word`: a signed integer becomes
/// `PushInt`, a `[ .. ]` list becomes `PushQuote` of its element words.
fn parse_input_word(toks: &[String], pos: &mut usize) -> Result<IWord, String> {
    let tok = toks
        .get(*pos)
        .ok_or_else(|| "unexpected end of input spec".to_string())?;
    if tok == "[" {
        *pos += 1;
        let mut body = Vec::new();
        loop {
            match toks.get(*pos) {
                None => return Err("unterminated '[' in input spec".to_string()),
                Some(t) if t == "]" => {
                    *pos += 1;
                    break;
                }
                Some(_) => body.push(parse_input_word(toks, pos)?),
            }
        }
        Ok(IWord::PushQuote(body))
    } else if tok == "]" {
        Err("unexpected ']' in input spec".to_string())
    } else {
        let n: i64 = tok
            .parse()
            .map_err(|_| format!("invalid integer in input spec: {tok:?}"))?;
        *pos += 1;
        Ok(IWord::PushInt(n))
    }
}

/// Lift a top-level input `Word` to a stack `Value`: `PushInt` → `Value::Int`,
/// `PushQuote` → `Value::Quote`. (Only these two shapes are produced by
/// `parse_input_word`.)
fn word_to_value(w: IWord) -> Result<Value, String> {
    match w {
        IWord::PushInt(n) => Ok(Value::Int(n)),
        IWord::PushQuote(body) => Ok(Value::Quote(body)),
        other => Err(format!("input item cannot be {other:?}")),
    }
}

/// Read a corpus `solution.mtl` file, strip a single trailing newline, parse it
/// with `mtl-syntax`, and convert it into an executable `mtl-core` program.
///
/// The trailing-newline strip matches the token-counting policy in
/// `bench/tokcount` so the validated program is exactly the counted artifact.
pub fn load_solution(path: impl AsRef<Path>) -> Result<Vec<IWord>, ParseError> {
    let raw = std::fs::read_to_string(path.as_ref())
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.as_ref().display()));
    let src = raw.strip_suffix('\n').unwrap_or(&raw);
    let prog = parse(src)?;
    Ok(conv_program(&prog))
}

#[cfg(test)]
mod input_decoder_tests {
    use super::{parse_input_stack, Value};
    use mtl_core::interp::Word as IWord;

    /// Mirror the sealed-test `int_list` helper so decoded lists are asserted
    /// byte-identical to the constructed-stack path.
    fn int_list(xs: &[i64]) -> Value {
        Value::Quote(xs.iter().map(|&n| IWord::PushInt(n)).collect())
    }

    #[test]
    fn empty_spec_is_empty_stack() {
        assert_eq!(parse_input_stack("").unwrap(), Vec::<Value>::new());
        assert_eq!(parse_input_stack("   ").unwrap(), Vec::<Value>::new());
    }

    #[test]
    fn positive_scalar_backward_compatible() {
        assert_eq!(parse_input_stack("5").unwrap(), vec![Value::Int(5)]);
        assert_eq!(parse_input_stack("123").unwrap(), vec![Value::Int(123)]);
    }

    #[test]
    fn negative_scalar() {
        assert_eq!(parse_input_stack("-24").unwrap(), vec![Value::Int(-24)]);
        assert_eq!(parse_input_stack("-706").unwrap(), vec![Value::Int(-706)]);
    }

    #[test]
    fn multiple_top_level_items_preserve_order() {
        // bottom .. top
        assert_eq!(
            parse_input_stack("-5 -2 -8 -1").unwrap(),
            vec![
                Value::Int(-5),
                Value::Int(-2),
                Value::Int(-8),
                Value::Int(-1)
            ]
        );
    }

    #[test]
    fn positive_list() {
        assert_eq!(parse_input_stack("[5 2]").unwrap(), vec![int_list(&[5, 2])]);
    }

    #[test]
    fn empty_list() {
        assert_eq!(parse_input_stack("[]").unwrap(), vec![int_list(&[])]);
    }

    #[test]
    fn negative_list_elements() {
        assert_eq!(
            parse_input_stack("[-5 -2 -8 -1]").unwrap(),
            vec![int_list(&[-5, -2, -8, -1])]
        );
    }

    #[test]
    fn scalar_then_list_matches_sealed_min_running_balance_vector() {
        // seal_min_running_balance vector: args [0, [-5, -5, 20, -100]]
        assert_eq!(
            parse_input_stack("0 [-5 -5 20 -100]").unwrap(),
            vec![Value::Int(0), int_list(&[-5, -5, 20, -100])]
        );
    }

    #[test]
    fn decoded_list_is_byte_identical_to_int_list() {
        // The exact seal_running_max all-negative vector from sealed.rs.
        assert_eq!(
            parse_input_stack("[-5 -2 -8 -1]").unwrap(),
            vec![int_list(&[-5, -2, -8, -1])]
        );
    }

    #[test]
    fn nested_list_supported() {
        assert_eq!(
            parse_input_stack("[[1 2] [-3]]").unwrap(),
            vec![Value::Quote(vec![
                IWord::PushQuote(vec![IWord::PushInt(1), IWord::PushInt(2)]),
                IWord::PushQuote(vec![IWord::PushInt(-3)]),
            ])]
        );
    }

    #[test]
    fn errors_are_reported() {
        assert!(parse_input_stack("[1 2").is_err());
        assert!(parse_input_stack("1 2]").is_err());
        assert!(parse_input_stack("1 abc").is_err());
    }
}
