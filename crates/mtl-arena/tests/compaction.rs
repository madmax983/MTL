//! Compaction unit + property tests (issue #51), complementing the forced
//! oracle arm in `oracle.rs`:
//!   * AC#6 — handle-remap parity: `reify_stack` / `reify_cont` / `fault_info` are
//!     bit-identical before and after a compaction at an interior point.
//!   * AC#7 — steady-state bound: a streaming `cat`-heavy loop that allocates and
//!     drops quotes WITHOUT a generation reset stays flat in arena bytes under
//!     compaction, versus growing linearly with the truncate-only baseline. Also
//!     reports the measured compaction pause and the memory ceiling.

use std::time::Instant;

use mtl_arena::{arena_step, compact, Mark, Prim as P, ProgWord, Step, Vm, VmState};
use mtl_core::interp as itp;

/// Cells allocated above the generation floor (the compaction-eligible set),
/// computed from a public high-water [`Mark`].
fn above_floor(m: Mark, floor: Mark) -> usize {
    (m.tape - floor.tape) + (m.stack_nodes - floor.stack_nodes) + (m.cont_nodes - floor.cont_nodes)
}

// ---------------------------------------------------------------------------
// AC#6 — reify invariance across a compaction.
// ---------------------------------------------------------------------------

/// Drive `prog` and, at each interior step, assert that compacting the live state
/// leaves `reify_stack` and `reify_cont` bit-identical (compaction is
/// observationally invisible). Returns the number of interior points checked.
fn assert_reify_invariant(prog: &[ProgWord]) -> usize {
    let mut vm = Vm::new();
    let mut st = VmState::initial();
    let pid = vm.compile(prog).expect("compile");
    vm.prepend(&mut st, pid);
    let floor = vm.mark();

    let mut checked = 0usize;
    loop {
        // Snapshot the reified observable state BEFORE compaction.
        let stack_before = vm.reify_stack(st.stack);
        let cont_before = vm.reify_cont(&st);

        // Compact the live state and re-reify from the remapped VmState.
        let (nvm, nroots, _stats) = compact(&vm, floor, core::slice::from_ref(&st));
        let nst = nroots[0];
        let stack_after = nvm.reify_stack(nst.stack);
        let cont_after = nvm.reify_cont(&nst);

        assert_eq!(stack_before, stack_after, "reify_stack changed across a compaction");
        assert_eq!(cont_before, cont_after, "reify_cont changed across a compaction");
        checked += 1;

        // Continue the real (uncompacted) run one atomic step.
        match arena_step(&mut vm, &mut st) {
            Step::Next => {}
            Step::Halt | Step::Fault(_) | Step::Invoke(_) => break,
        }
    }
    checked
}

#[test]
fn reify_invariant_across_compaction() {
    use ProgWord::*;
    // A spread of shapes that exercise stack Quote values, cat/cons tape growth,
    // and deep cont chains (primrec / times / fold / linrec setup segments).
    let progs: Vec<(&str, Vec<ProgWord>)> = vec![
        ("arith", vec![PushInt(3), PushInt(4), Prim(P::Add), PushInt(2), Prim(P::Mul)]),
        (
            "cat_cons",
            vec![
                PushInt(5),
                PushQuote(vec![PushInt(1), PushInt(2)]),
                Prim(P::Cons),
                PushQuote(vec![PushInt(9)]),
                Prim(P::Cat),
            ],
        ),
        (
            "primrec_fact_5",
            vec![
                PushInt(5),
                PushQuote(vec![PushInt(1)]),
                PushQuote(vec![Prim(P::Mul)]),
                Prim(P::PrimRec),
            ],
        ),
        (
            "times_count_10",
            vec![
                PushInt(0),
                PushInt(10),
                PushQuote(vec![PushInt(1), Prim(P::Add)]),
                Prim(P::Times),
            ],
        ),
        (
            "fold_sum",
            vec![
                PushQuote(vec![PushInt(1), PushInt(2), PushInt(3), PushInt(4)]),
                PushInt(0),
                PushQuote(vec![Prim(P::Add)]),
                Prim(P::Fold),
            ],
        ),
    ];

    let mut total_points = 0usize;
    for (name, prog) in &progs {
        let n = assert_reify_invariant(prog);
        println!("reify-invariance: {name} — {n} interior compaction points bit-identical");
        total_points += n;
    }
    assert!(total_points > 0);
}

/// AC#6 (fault arm): a program that faults produces an identical `fault_info`
/// whether or not a compaction fired at the pre-fault safe point.
#[test]
fn fault_info_invariant_across_compaction() {
    use ProgWord::*;
    // `1 [2] +` → TypeMismatch after building a Quote on the tape (above floor).
    let prog = vec![PushInt(1), PushQuote(vec![PushInt(2)]), Prim(P::Add)];

    let mut vm = Vm::new();
    let mut st = VmState::initial();
    let pid = vm.compile(&prog).expect("compile");
    vm.prepend(&mut st, pid);
    let floor = vm.mark();

    // Step until the pre-fault position (arena_step restores st to pre-step on
    // Fault), capturing fault_info both directly and after a compaction.
    loop {
        let saved = st;
        match arena_step(&mut vm, &mut st) {
            Step::Next => {}
            Step::Fault(f) => {
                let fi_direct = vm.fault_info(&st, f);

                // Compact at the pre-step safe point, then take the same step.
                let (nvm, nroots, _s) = compact(&vm, floor, core::slice::from_ref(&saved));
                let mut cvm = nvm;
                let mut cst = nroots[0];
                let cf = match arena_step(&mut cvm, &mut cst) {
                    Step::Fault(cf) => cf,
                    other => panic!("expected fault after compaction, got {other:?}"),
                };
                let fi_compacted = cvm.fault_info(&cst, cf);

                assert_eq!(fi_direct, fi_compacted, "fault_info changed across a compaction");
                assert_eq!(fi_direct.fault, itp::Fault::TypeMismatch);
                return;
            }
            other => panic!("unexpected terminal before fault: {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// AC#7 — steady-state flat-bytes under a streaming, never-reset host.
// ---------------------------------------------------------------------------

/// One streaming "turn" body: `[1] [2] cat drop` — interns a fresh `cat` quote
/// above the floor, then immediately drops it. The result is dead by the turn
/// boundary, so the live working set is O(1), but the append-only tape grows by a
/// few words every turn.
fn turn_body() -> Vec<ProgWord> {
    use ProgWord::*;
    vec![
        PushQuote(vec![PushInt(1)]),
        PushQuote(vec![PushInt(2)]),
        Prim(P::Cat),
        Prim(P::Drop),
    ]
}

/// Model an always-on / streaming host: a SINGLE long-lived [`Vm`] that processes
/// `turns` inputs back-to-back, **never resetting a generation** (so it could
/// carry interned state across turns). The turn body is compiled ONCE below the
/// floor; each turn prepends it and runs to Halt. Between turns — a
/// generation-safe point — the driver optionally compacts.
///
/// Returns `(tape_high_water, compactions, total_compaction_time_ns,
/// max_final_stack_len)`.
fn drive_streaming(turns: usize, compaction: Option<usize>) -> (usize, usize, u128, usize) {
    let mut vm = Vm::new();
    // The turn body is static code below the generation floor (immortal).
    let body = vm.compile(&turn_body()).expect("compile turn body");
    let floor = vm.mark();

    let mut tape_hw = floor.tape;
    let mut compactions = 0usize;
    let mut compaction_ns: u128 = 0;
    let mut max_stack = 0usize;

    // A dummy frontier state the driver carries across turns (empty between turns).
    let mut carried = VmState::initial();

    for _ in 0..turns {
        // Between-turn safe point: compact the (empty) live frontier if grown.
        if let Some(threshold) = compaction {
            if above_floor(vm.mark(), floor) > threshold {
                let t0 = Instant::now();
                let (nvm, nroots, _stats) = compact(&vm, floor, core::slice::from_ref(&carried));
                compaction_ns += t0.elapsed().as_nanos();
                compactions += 1;
                vm = nvm;
                carried = nroots[0];
            }
        }

        // Run one turn to completion on the long-lived Vm.
        let mut st = VmState::initial();
        vm.prepend(&mut st, body);
        loop {
            match arena_step(&mut vm, &mut st) {
                Step::Next => {}
                Step::Halt => break,
                other => panic!("turn did not halt cleanly: {other:?}"),
            }
        }
        max_stack = max_stack.max(vm.stack_values(st.stack).len());
        tape_hw = tape_hw.max(vm.mark().tape);
    }

    (tape_hw, compactions, compaction_ns, max_stack)
}

#[test]
fn steady_state_bytes_stay_flat_under_compaction() {
    // Word size for the byte reporting (each tape entry is a `Word`).
    let word_bytes = std::mem::size_of::<mtl_arena::Word>();

    // Compare two workload sizes: the truncate-only baseline must grow with N,
    // the compacting driver must stay flat (independent of N).
    let threshold = 256usize; // compact once >256 above-floor cells accumulate.

    let mut baseline_hw = [0usize; 2];
    let mut compact_hw = [0usize; 2];
    let sizes = [20_000usize, 80_000usize];

    // Aggregate pause stats from the larger run.
    let mut big_compactions = 0usize;
    let mut big_compaction_ns = 0u128;

    for (i, &n) in sizes.iter().enumerate() {
        let (base_tape, base_nc, _base_ns, base_stack) = drive_streaming(n, None);
        let (comp_tape, comp_nc, comp_ns, comp_stack) = drive_streaming(n, Some(threshold));

        assert_eq!(base_nc, 0, "baseline must not compact");
        assert_eq!(base_stack, 0, "streaming turns drop everything → empty stack");
        assert_eq!(comp_stack, 0, "compacting run must produce the same empty stack");

        baseline_hw[i] = base_tape;
        compact_hw[i] = comp_tape;

        if i == 1 {
            big_compactions = comp_nc;
            big_compaction_ns = comp_ns;
        }

        println!(
            "turns={n:>6}  truncate-only tape_hw = {base_tape:>8} words ({:>9} B)   |   compacting tape_hw = {comp_tape:>6} words ({:>7} B), {comp_nc} compactions",
            base_tape * word_bytes,
            comp_tape * word_bytes,
        );
    }

    // --- Report the measured pause + ceiling (AC#7 success metric) ---
    let avg_pause_us = if big_compactions > 0 {
        (big_compaction_ns as f64 / big_compactions as f64) / 1000.0
    } else {
        0.0
    };
    println!(
        "\ncompaction pause: {big_compactions} compactions over the 80000-turn run, \
         total {:.3} ms, avg {:.1} µs/compaction",
        big_compaction_ns as f64 / 1_000_000.0,
        avg_pause_us
    );
    println!(
        "memory ceiling: compacting tape_hw stayed at ~{} words (~{} B) for BOTH 20000 and 80000 turns; \
         truncate-only grew {} → {} words",
        compact_hw[0].max(compact_hw[1]),
        compact_hw[1] * word_bytes,
        baseline_hw[0],
        baseline_hw[1],
    );

    // --- Assertions (falsifiable) ---
    // 1. Truncate-only grows ~linearly with N (4x the work → clearly larger tape).
    assert!(
        baseline_hw[1] > baseline_hw[0] * 3,
        "truncate-only tape should grow ~linearly with N: {} vs {}",
        baseline_hw[0],
        baseline_hw[1]
    );
    // 2. Compaction keeps the ceiling FLAT: independent of N, within a small
    //    constant factor of the floor + threshold working set.
    assert!(
        compact_hw[1] <= compact_hw[0] + 8,
        "compacting ceiling must be flat across N: {} vs {}",
        compact_hw[0],
        compact_hw[1]
    );
    // 3. Compaction ceiling is DRAMATICALLY below the truncate-only ceiling.
    assert!(
        compact_hw[1] * 20 < baseline_hw[1],
        "compaction must bound memory far below the baseline: compact {} vs baseline {}",
        compact_hw[1],
        baseline_hw[1]
    );
}
