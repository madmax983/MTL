//! # mtl-host — the v0.4 "host half"
//!
//! The **unverified host runner** that sits *above* the pure, verified MTL core
//! (`mtl-core`). It implements the two-machine split of the v0.4 effects design
//! ([`docs/design/v0.4-effects.md`]): the pure core suspends at every capability
//! `Call(name)` and yields an `Invoke` event; this crate services that event —
//! grant-checks it, meters it, runs the capability, and resumes the core — while
//! keeping every impure concern (host state, resource caps, cancellation) strictly
//! host-side of the single narrow `Invoke` channel.
//!
//! ## Module map
//!
//! * [`handle`] — opaque `i64` string handles (design §5: no `Value::Str`).
//! * [`meter`] — per-capability call budgets + output-byte cap (design §6).
//! * [`host`] — [`host::HostCtx`], [`host::HostResult`], [`host::HostFault`].
//! * [`capability`] — [`capability::Capability`] / [`capability::Registry`] (the grant set, §3).
//! * [`core_bridge`] — the ADAPTER SEAM sourcing `Invoke` events from today's core.
//! * [`driver`] — [`driver::drive`], the impure loop with clean-cancel semantics (§7).
//! * [`caps`] — the standard Tier-3 capability set + fixtures (design §8).

pub mod capability;
pub mod caps;
pub mod core_bridge;
pub mod driver;
pub mod handle;
pub mod host;
pub mod meter;

use std::path::Path;

use mtl_core::interp::Word as IWord;
use mtl_syntax::{parse, ParseError, Prim, Word};

/// Convert one parsed `mtl-syntax` word into an executable `mtl-core` word.
///
/// `PushInt` maps straight across, `PushQuote` recurses, `Call(Vec<char>)`
/// becomes `Call(String)`, and the 23 primitives map by name (both enums list
/// them in the same order). Mirrors `bench/validate`'s `conv`.
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
            Prim::PrimRec => IPrim::PrimRec,
            Prim::Times => IPrim::Times,
            Prim::LinRec => IPrim::LinRec,
            Prim::Uncons => IPrim::Uncons,
            Prim::Fold => IPrim::Fold,
            Prim::Xor => IPrim::Xor,
        }),
    }
}

/// Convert a whole parsed program.
pub fn conv_program(prog: &[Word]) -> Vec<IWord> {
    prog.iter().map(conv).collect()
}

/// Read a `solution.mtl` file, strip a single trailing newline, parse with
/// `mtl-syntax`, and convert into an executable `mtl-core` program (so the
/// token-counted artifact and the executed program are the same bytes).
pub fn load_solution(path: impl AsRef<Path>) -> Result<Vec<IWord>, ParseError> {
    let raw = std::fs::read_to_string(path.as_ref())
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.as_ref().display()));
    let src = raw.strip_suffix('\n').unwrap_or(&raw);
    let prog = parse(src)?;
    Ok(conv_program(&prog))
}
