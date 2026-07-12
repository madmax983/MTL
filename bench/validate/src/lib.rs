//! Interpreter-validation harness for the MTL benchmark corpus.
//!
//! The two crates `mtl-syntax` (parser) and `mtl-core` (interpreter) share no
//! types, so this crate provides:
//!   * [`conv`] — convert a parsed `mtl_syntax::Word` into an executable
//!     `mtl_core::interp::Word` (the 21 `Prim` variants map by name).
//!   * [`load_solution`] — read a corpus `solution.mtl` file from disk, strip a
//!     single trailing newline, and parse it into an executable program.
//!
//! The integration test `tests/corpus.rs` reads the ACTUAL solution files (so
//! the token-counted artifact and the validated program are the same bytes),
//! executes each on the real interpreter against per-task input/output vectors,
//! and asserts the terminal `Outcome::Halt(expected)`.

use std::path::Path;

use mtl_core::interp::Word as IWord;
use mtl_syntax::{parse, ParseError, Prim, Word};

/// Convert one parsed syntax word into an executable interpreter word.
///
/// `PushInt` maps straight across, `PushQuote` recurses, `Call(Vec<char>)`
/// becomes `Call(String)`, and the 21 primitives map by name (both enums list
/// them in the same order).
pub fn conv(w: &Word) -> IWord {
    use mtl_core::interp::Prim as IPrim;
    match w {
        Word::PushInt(n) => IWord::PushInt(*n),
        Word::PushQuote(body) => IWord::PushQuote(body.iter().map(conv).collect()),
        Word::Call(chars) => IWord::Call(chars.iter().collect::<String>()),
        Word::Prim(p) => IWord::Prim(match p {
            Prim::Dup => IPrim::Dup,
            Prim::Drop => IPrim::Drop,
            Prim::Swap => IPrim::Swap,
            Prim::Rot => IPrim::Rot,
            Prim::Over => IPrim::Over,
            Prim::Apply => IPrim::Apply,
            Prim::Cat => IPrim::Cat,
            Prim::Cons => IPrim::Cons,
            Prim::Dip => IPrim::Dip,
            Prim::Add => IPrim::Add,
            Prim::Sub => IPrim::Sub,
            Prim::Mul => IPrim::Mul,
            Prim::Div => IPrim::Div,
            Prim::Mod => IPrim::Mod,
            Prim::Eq => IPrim::Eq,
            Prim::Lt => IPrim::Lt,
            Prim::If => IPrim::If,
            // v0.2 recursion/list primitives: parsed by mtl-syntax and executed
            // by mtl-core (both enums list them in the same order).
            Prim::PrimRec => IPrim::PrimRec,
            Prim::Times => IPrim::Times,
            Prim::LinRec => IPrim::LinRec,
            Prim::Uncons => IPrim::Uncons,
            // v0.3 sequence primitives: parsed by mtl-syntax but not yet
            // executable — mtl-core's interp variants land in the v03-core PR.
            // Explicit arms (no wildcard) so the compiler flags any future Prim.
            Prim::Fold | Prim::Xor => {
                unimplemented!("v0.3 primitive not yet executable (mtl-core support pending)")
            }
        }),
    }
}

/// Convert a whole parsed program into an executable program.
pub fn conv_program(prog: &[Word]) -> Vec<IWord> {
    prog.iter().map(conv).collect()
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
