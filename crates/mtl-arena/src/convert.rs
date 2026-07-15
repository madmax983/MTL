//! The single production home for `interp -> arena` input conversion and the
//! user-facing [`Engine`] selector.
//!
//! Every user-facing entry point that lets a caller pick the arena backend
//! (`mtlrun`, `tier3run`, the `mtl-host` driver, the corpus gate, the perf
//! examples) needs to turn a reference-AST program (`mtl_core::interp::Word`)
//! into the arena's owned input tree (`ProgWord`). That converter used to be
//! copy-pasted into test scope (`mtl-host/tests/arena_backend.rs`,
//! `mtl-perf/examples/arena_vs_interp.rs`); it now lives here, once, so the
//! flip has a single DRY definition.

use mtl_core::interp as itp;

use crate::types::{Prim, ProgWord};

/// Map a reference `interp::Prim` to its arena mirror. Exhaustive, no wildcard:
/// a new reference primitive fails to compile here until it is mirrored, which
/// is exactly the drift guard the conformance crate also enforces.
pub fn prim_from_interp(p: itp::Prim) -> Prim {
    use itp::Prim as I;
    match p {
        I::Dup => Prim::Dup,
        I::Drop => Prim::Drop,
        I::Swap => Prim::Swap,
        I::Rot => Prim::Rot,
        I::Over => Prim::Over,
        I::Apply => Prim::Apply,
        I::Cat => Prim::Cat,
        I::Cons => Prim::Cons,
        I::Dip => Prim::Dip,
        I::Add => Prim::Add,
        I::Sub => Prim::Sub,
        I::Mul => Prim::Mul,
        I::Div => Prim::Div,
        I::Mod => Prim::Mod,
        I::Eq => Prim::Eq,
        I::Lt => Prim::Lt,
        I::If => Prim::If,
        I::PrimRec => Prim::PrimRec,
        I::Times => Prim::Times,
        I::LinRec => Prim::LinRec,
        I::Uncons => Prim::Uncons,
        I::Fold => Prim::Fold,
        I::Xor => Prim::Xor,
    }
}

/// Convert one reference-AST word into an arena input word (owned tree form;
/// `Vm::compile` interns it into the tape).
pub fn word_from_interp(w: &itp::Word) -> ProgWord {
    match w {
        itp::Word::PushInt(n) => ProgWord::PushInt(*n),
        itp::Word::PushQuote(body) => {
            ProgWord::PushQuote(body.iter().map(word_from_interp).collect())
        }
        itp::Word::Prim(p) => ProgWord::Prim(prim_from_interp(*p)),
        itp::Word::Call(name) => ProgWord::Call(name.clone()),
    }
}

/// Encode a reference-AST value as a leading push (`Int -> PushInt`,
/// `Quote -> PushQuote`). Used to seed a non-empty initial stack: since the
/// arena engine starts from `VmState::initial()` (empty stack), an initial
/// stack is threaded by prepending it to the program as pushes — exactly how
/// the corpus/oracle already encode inputs.
pub fn value_to_progword(v: &itp::Value) -> ProgWord {
    match v {
        itp::Value::Int(n) => ProgWord::PushInt(*n),
        itp::Value::Quote(body) => {
            ProgWord::PushQuote(body.iter().map(word_from_interp).collect())
        }
    }
}

/// Convert a whole reference-AST program into an arena input program.
pub fn prog_from_interp(prog: &[itp::Word]) -> Vec<ProgWord> {
    prog.iter().map(word_from_interp).collect()
}

/// Convert a reference-AST program running against `initial_stack` into an
/// arena input program: the initial stack is prepended as leading pushes, then
/// the program follows. For an empty `initial_stack` this is exactly
/// [`prog_from_interp`].
pub fn prog_from_interp_with_stack(initial_stack: &[itp::Value], prog: &[itp::Word]) -> Vec<ProgWord> {
    let mut out: Vec<ProgWord> = initial_stack.iter().map(value_to_progword).collect();
    out.extend(prog.iter().map(word_from_interp));
    out
}

/// User-facing execution-engine selector. Default is [`Engine::Arena`]: the
/// arena refinement obligation is discharged (machine-checked Verus, unconditional,
/// fault parity), so the arena is the default execution path. The reference
/// interpreter stays reachable as the differential anchor via [`Engine::Interp`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Engine {
    /// The arena backend (`run_arena` / `arena_drive`). The default.
    #[default]
    Arena,
    /// The reference interpreter (`mtl_core::interp`). The differential anchor,
    /// reachable behind an explicit `--engine=interp` / API selection.
    Interp,
}

impl Engine {
    /// Parse an `--engine` value. Accepts `arena` and `interp` (case-insensitive).
    pub fn parse(s: &str) -> Result<Engine, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "arena" => Ok(Engine::Arena),
            "interp" | "interpreter" => Ok(Engine::Interp),
            other => Err(format!("unknown engine {other:?} (expected `arena` or `interp`)")),
        }
    }

    /// The canonical lowercase name (`"arena"` / `"interp"`).
    pub fn name(self) -> &'static str {
        match self {
            Engine::Arena => "arena",
            Engine::Interp => "interp",
        }
    }
}
