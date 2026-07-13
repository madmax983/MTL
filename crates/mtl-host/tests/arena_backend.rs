//! OPT-IN arena backend parity — proves that `mtl_arena::host::arena_drive` is a
//! behaviourally-identical, explicitly-selected alternative to the default
//! `mtl_core::host::drive`, driven through this crate's REAL `HostShim` (grant →
//! meter → effect → record), not a toy host.
//!
//! ## This is an opt-in demonstration, not a substitution
//!
//! The production entry point `mtl_host::driver::drive` is UNCHANGED: it still
//! delegates to `mtl_core::host::drive` (the interpreter backend). Nothing here
//! swaps that default. The arena backend is reached only by a caller who names
//! `mtl_arena::host::arena_drive` explicitly — exactly what these tests do. Each
//! test runs the SAME program through BOTH backends against the SAME real host
//! setup and asserts identical `RunResult` AND identical host-visible effects
//! (emitted output), proving interchangeability at the real host layer.

use mtl_core::host::{drive as itp_drive, RunResult};
use mtl_core::interp::build::call;
use mtl_core::interp::{Vm, Word};

use mtl_arena::host::arena_drive;
use mtl_arena::ProgWord;
use mtl_host::caps;
use mtl_host::core_bridge::HostShim;
use mtl_host::host::TaskFixture;

const FUEL: u64 = 100_000;

// ---- reference AST -> arena input AST (dependency-free local converter) ----

fn to_progword(w: &Word) -> ProgWord {
    match w {
        Word::PushInt(n) => ProgWord::PushInt(*n),
        Word::PushQuote(b) => ProgWord::PushQuote(b.iter().map(to_progword).collect()),
        Word::Prim(p) => ProgWord::Prim(prim_to_arena(*p)),
        Word::Call(name) => ProgWord::Call(name.clone()),
    }
}

fn prim_to_arena(p: mtl_core::interp::Prim) -> mtl_arena::Prim {
    use mtl_arena::Prim as A;
    use mtl_core::interp::Prim as I;
    match p {
        I::Dup => A::Dup,
        I::Drop => A::Drop,
        I::Swap => A::Swap,
        I::Rot => A::Rot,
        I::Over => A::Over,
        I::Apply => A::Apply,
        I::Cat => A::Cat,
        I::Cons => A::Cons,
        I::Dip => A::Dip,
        I::Add => A::Add,
        I::Sub => A::Sub,
        I::Mul => A::Mul,
        I::Div => A::Div,
        I::Mod => A::Mod,
        I::Eq => A::Eq,
        I::Lt => A::Lt,
        I::If => A::If,
        I::PrimRec => A::PrimRec,
        I::Times => A::Times,
        I::LinRec => A::LinRec,
        I::Uncons => A::Uncons,
        I::Fold => A::Fold,
        I::Xor => A::Xor,
    }
}

/// Run `prog` through the DEFAULT interpreter driver against a fresh real host,
/// returning `(result, emitted_output)`.
fn via_interp(prog: &[Word], fixture: TaskFixture) -> (RunResult, Vec<u8>) {
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(fixture);
    let result = {
        let mut shim = HostShim::new(&mut reg, &mut ctx);
        itp_drive(Vm::new(prog.to_vec()), FUEL, &mut shim)
    };
    (result, ctx.output_bytes().to_vec())
}

/// Run `prog` through the OPT-IN arena driver against a fresh real host,
/// returning `(result, emitted_output)`. Same `HostShim`, same registry/ctx
/// construction — only the driver differs.
fn via_arena(prog: &[Word], fixture: TaskFixture) -> (RunResult, Vec<u8>) {
    let prog_arena: Vec<ProgWord> = prog.iter().map(to_progword).collect();
    let (mut reg, mut ctx) = caps::standard_registry_and_ctx(fixture);
    let result = {
        let mut shim = HostShim::new(&mut reg, &mut ctx);
        arena_drive(&prog_arena, FUEL, &mut shim)
    };
    (result, ctx.output_bytes().to_vec())
}

#[test]
fn arena_driver_matches_interp_through_the_real_host_shim() {
    // `readline emit`: readline interns a line handle, emit writes it. Through the
    // REAL grant/meter/effect shim this Dones and emits "hello world\n".
    let prog = vec![call("readline"), call("emit")];

    let (itp_res, itp_out) = via_interp(&prog, caps::fixture_echo_line());
    let (arena_res, arena_out) = via_arena(&prog, caps::fixture_echo_line());

    assert_eq!(arena_res, itp_res, "arena RunResult diverged from interp at the real host layer");
    assert_eq!(arena_out, itp_out, "arena host-visible output diverged from interp");
    // Concrete expectation (not just self-consistency): the granted effect ran.
    assert_eq!(arena_res, RunResult::Done(vec![]));
    assert_eq!(String::from_utf8_lossy(&arena_out), "hello world\n");
}

#[test]
fn arena_driver_matches_interp_hostfault_through_the_real_host_shim() {
    // An unregistered capability is NotGranted host-side. Both backends must
    // surface the identical HostFault and produce NO output (no torn effect).
    let prog = vec![call("nope")];

    let (itp_res, itp_out) = via_interp(&prog, caps::fixture_echo_line());
    let (arena_res, arena_out) = via_arena(&prog, caps::fixture_echo_line());

    assert_eq!(arena_res, itp_res, "arena HostFault diverged from interp");
    assert_eq!(arena_out, itp_out);
    assert_eq!(arena_res, RunResult::HostFaulted(mtl_core::host::HostCode::NotGranted));
    assert!(arena_out.is_empty(), "a refused capability must produce no output");
}

#[test]
fn arena_driver_matches_interp_global_budget_cancellation() {
    // A never-terminating agent_loop: its `done?` threshold is unreachable, so it
    // yields a capability every iteration forever. Only the single GLOBAL fuel
    // budget can stop it — and it must stop identically on both backends
    // (Cancelled, no torn effect).
    let prog = mtl_host::load_solution(format!(
        "{}/../../bench/tier3/tasks/agent_loop/solution.mtl",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("parse agent_loop solution");

    // Never-terminating: `done_threshold = i64::MAX` is unreachable from state 0.
    let fixture = || TaskFixture { initial_state: 0, done_threshold: i64::MAX, ..Default::default() };
    const BUDGET: u64 = 200;

    let (mut reg_i, mut ctx_i) = caps::standard_registry_and_ctx(fixture());
    let itp_res = {
        let mut shim = HostShim::new(&mut reg_i, &mut ctx_i);
        itp_drive(Vm::new(prog.clone()), BUDGET, &mut shim)
    };

    let prog_arena: Vec<ProgWord> = prog.iter().map(to_progword).collect();
    let (mut reg_a, mut ctx_a) = caps::standard_registry_and_ctx(fixture());
    let arena_res = {
        let mut shim = HostShim::new(&mut reg_a, &mut ctx_a);
        arena_drive(&prog_arena, BUDGET, &mut shim)
    };

    assert_eq!(arena_res, itp_res, "arena cancellation diverged from interp");
    assert_eq!(arena_res, RunResult::Cancelled);
    assert!(ctx_a.output_bytes().is_empty(), "a between-steps cancel must leave no torn effect");
    assert_eq!(ctx_a.output_bytes(), ctx_i.output_bytes());
}
