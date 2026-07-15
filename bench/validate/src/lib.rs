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
            mtl_arena::run_arena(&arena_prog, fuel).outcome().into_interp()
        }
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
