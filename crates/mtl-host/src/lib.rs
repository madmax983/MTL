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
//! * [`core_bridge`] — the SERVICING SEAM: a [`core_bridge::HostShim`] implementing
//!   `mtl_core::host::Host` (grant + metering-before-effect + at-most-once).
//! * [`driver`] — [`driver::drive`], a thin wrapper that fully delegates the loop
//!   to `mtl_core::host::drive` (which owns the global fuel budget / clean-cancel, §7).
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
use mtl_syntax::{parse, ParseError, Word};

/// Generate the syntax-`Prim` → interp-`Prim` opcode map from the checked
/// manifest's canonical rows (`mtl_syntax::for_each_primitive!`). This is the
/// codegen from issue #46: the 23-arm match is no longer hand-written here, so
/// the `conv` opcode map cannot drift from the manifest. Mirrors
/// `bench/validate`'s generated map.
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

/// Convert one parsed `mtl-syntax` word into an executable `mtl-core` word.
///
/// `PushInt` maps straight across, `PushQuote` recurses, `Call(Vec<char>)`
/// becomes `Call(String)`, and the 23 primitives map by name via the
/// manifest-generated [`prim_to_iprim`] opcode map. Mirrors `bench/validate`'s `conv`.
pub fn conv(w: &Word) -> IWord {
    match w {
        Word::PushInt(n) => IWord::PushInt(*n),
        Word::PushQuote(body) => IWord::PushQuote(body.iter().map(conv).collect()),
        Word::Call(chars) => IWord::Call(chars.iter().collect::<String>()),
        Word::Prim(p) => IWord::Prim(prim_to_iprim(*p)),
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
