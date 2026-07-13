//! P5 executable validation — concrete two-counter Minsky machines compiled to
//! MTL and executed on the REAL interpreter (`mtl_core::interp::run`).
//!
//! In the abstract Verus model, counter quotations are unbounded Seqs.
//! In this executable validation they are finite Vecs, bounded by the host machine as all concrete implementations necessarily are.
//!
//! This is the executable counterpart of the Verus proof in
//! `crates/mtl-core/src/p5_universality.rs`. It implements the SAME construction
//! the proof reasons about (unary quotation counters + a single self-applying
//! `:!` dispatch-loop quote `U`) as plain `Vec<Word>` programs, then runs them
//! through the fuel-bounded interpreter and decodes the resulting counter
//! quotations. It is concrete evidence that the construction actually computes.
//! These terminating machines are a transcription CHECK of the construction —
//! they do NOT themselves prove universality; the universal quantification is
//! the load-bearing Verus theorem (`p5_simulation` + halting correspondence).
//!
//! Out-of-range-PC model (matches `minsky_step` and the proof): a `finite-code
//! Minsky machine with implicit halt outside the code domain` — `pc >= prog.len()`
//! (including a jump target past the code) is a legal way to HALT, not a fault.
//!
//! Layout invariant between loop iterations (top at right):
//!   stack = [ Quote(unary(c1)), Quote(unary(c2)), Int(pc), Quote(U) ]
//!   cont  = [ Dup, Apply ]
//! Counters live in quotation LENGTH (unbounded Seq at the spec level; finite Vec
//! here), never in an Int — the P5 honesty pivot. (The `i64` bound still caps the
//! PC and marker VALUE, not the counter magnitude.)

use mtl_core::interp::build::*;
use mtl_core::interp::{exec_step, run, Outcome, Step, Value, Vm, Word};
use proptest::prelude::*;

// ---- Minsky machine (mirror of the ghost `MInstr`/`MProg` in the proof) ----

#[derive(Clone, Copy, Debug)]
enum MInstr {
    /// Inc(reg=false→c1 / true→c2, next pc)
    Inc(bool, usize),
    /// DecJz(reg, jz target, nz target)
    DecJz(bool, usize, usize),
    Halt,
}

// ---- checked index/PC conversion (item b: no silent usize->i64 wrap) ----

/// Convert a program index / jump target / PC (`usize`) to the `i64` an MTL
/// `Int` word carries. Checked: `usize` values above `i64::MAX` would silently
/// wrap under a bare `as i64`, corrupting the encoded PC. This is the EXECUTABLE
/// counterpart of the spec's ghost `nat -> int` cast, which cannot truncate;
/// here the host `Int` is `i64`, so the conversion must be validated.
fn pc_int(x: usize) -> i64 {
    i64::try_from(x).expect("program index/PC exceeds i64::MAX (host address-space bound)")
}

/// Validate a program before compiling/running it (item b). Enforces only the
/// EXECUTABLE address-space bound — that every index the compiler will emit as an
/// MTL `Int` fits in `i64`:
///   * `prog.len() <= i64::MAX`   (every instruction index is representable),
///   * `pc0 <= i64::MAX`          (the initial PC is representable),
///   * every `Inc`/`DecJz` jump target `<= i64::MAX`.
///
/// It does NOT reject out-of-range targets: under the `finite-code Minsky machine
/// with implicit halt outside the code domain` model, a target `>= prog.len()` is
/// a legal HALT, so range-checking targets would contradict the chosen model.
/// The check bounds only the instruction-ADDRESS space, never counter magnitude
/// (counters live in quotation length, never in an `Int`).
fn validate_prog(prog: &[MInstr], pc0: usize) {
    assert!(u64::try_from(prog.len()).map_or(false, |n| n <= i64::MAX as u64),
        "program length exceeds i64::MAX");
    let _ = pc_int(pc0);
    for instr in prog {
        match *instr {
            MInstr::Inc(_, j) => { let _ = pc_int(j); }
            MInstr::DecJz(_, jz, nz) => { let _ = pc_int(jz); let _ = pc_int(nz); }
            MInstr::Halt => {}
        }
    }
}

// ---- unary counter encoding ----

fn unary(n: usize) -> Vec<Word> {
    // n marker words PushInt(0); length == n, magnitude carried by length only.
    (0..n).map(|_| int(0)).collect()
}
fn counter(n: usize) -> Value {
    Value::Quote(unary(n))
}
fn decode_counter(v: &Value) -> usize {
    match v {
        Value::Quote(q) => {
            // every element must be the marker PushInt(0)
            assert!(q.iter().all(|w| matches!(w, Word::PushInt(0))), "non-marker in counter");
            q.len()
        }
        _ => panic!("counter slot is not a Quote"),
    }
}

// ---- the increment fragment `0 swap cons` (operates on top-of-stack counter) ----
fn inc_frag() -> Vec<Word> {
    vec![int(0), swap(), cons()]
}

// ---- counter access wrappers for the [c1, c2, U] sub-layout ----
// operate on c2 (slot 1, just under U):  [FRAG] dip
fn on_c2(frag: Vec<Word>) -> Vec<Word> {
    vec![quote(frag), dip()]
}
// operate on c1 (slot 0, under c2 and U): [ [FRAG] dip ] dip
fn on_c1(frag: Vec<Word>) -> Vec<Word> {
    vec![quote(vec![quote(frag), dip()]), dip()]
}

// ---- per-instruction body (entry stack [c1, c2, U, pc], cont = BODY) ----
fn body(instr: MInstr) -> Vec<Word> {
    match instr {
        MInstr::Halt => {
            // drop pc, drop U -> [c1, c2]; continuation drains -> Halt.
            vec![drop(), drop()]
        }
        MInstr::Inc(reg, j) => {
            let mut b = vec![drop()]; // remove pc -> [c1, c2, U]
            b.extend(if reg { on_c2(inc_frag()) } else { on_c1(inc_frag()) });
            // -> [c1', c2', U]; install pc := j under U, then re-enter loop.
            b.extend(vec![int(pc_int(j)), swap(), dup(), apply()]);
            b
        }
        MInstr::DecJz(reg, jz, nz) => {
            let mut b = vec![drop()]; // -> [c1, c2, U]
            if reg {
                // c2: swap -> [c1, U, c2]; counter on top.
                b.push(swap());
                // THEN (nonzero): [c1,U,junk,c2'] -> restore, pc:=nz, loop.
                let then_q = vec![swap(), drop(), swap(), int(pc_int(nz)), swap(), dup(), apply()];
                // ELSE (zero): [c1,U] -> re-push empty c2, pc:=jz, loop.
                let else_q = vec![quote(vec![]), swap(), int(pc_int(jz)), swap(), dup(), apply()];
                b.extend(vec![uncons(), quote(then_q), quote(else_q), iff()]);
            } else {
                // c1: rot -> [c2, U, c1]; counter on top.
                b.push(rot());
                // THEN (nonzero): [c2,U,junk,c1'] -> restore, pc:=nz, loop.
                let then_q =
                    vec![swap(), drop(), rot(), rot(), int(pc_int(nz)), swap(), dup(), apply()];
                // ELSE (zero): [c2,U] -> re-push empty c1, pc:=jz, loop.
                let else_q =
                    vec![quote(vec![]), rot(), rot(), int(pc_int(jz)), swap(), dup(), apply()];
                b.extend(vec![uncons(), quote(then_q), quote(else_q), iff()]);
            }
            b
        }
    }
}

// ---- the dispatch cascade DISP and the loop quote U ----
// ITERATIVE reverse fold (item e): the cascade is the same finite If-chain the
// recursive `disp(prog, i)` built — `disp(prog, 0)` — but assembled bottom-up
// from the out-of-range tail inward, so a large program cannot overflow the
// Rust host call stack the way the naive self-recursion would.
//
// Cost note (item e): selecting arm `pc` walks `pc` comparison misses then one
// hit, so ONE simulated Minsky step costs O(program length) MTL steps.
fn disp(prog: &[MInstr]) -> Vec<Word> {
    // base = DISP at index == prog.len(): pc out of range -> drop pc, drop U ->
    // [c1,c2]; drain -> Halt (the `implicit halt outside the code domain` model).
    let mut acc = vec![drop(), drop()];
    for i in (0..prog.len()).rev() {
        // [c1,c2,U,pc] -> dup pc, compare to i; if equal splice BODY_i else DISP(i+1)
        acc = vec![
            dup(),
            int(pc_int(i)),
            eq(),
            quote(body(prog[i])),
            quote(acc),
            iff(),
        ];
    }
    acc
}
fn compile_u(prog: &[MInstr]) -> Vec<Word> {
    // entry to U: [c1,c2,Int(pc),U]; swap brings pc to top -> [c1,c2,U,pc].
    let mut u = vec![swap()];
    u.extend(disp(prog));
    u
}

/// Run `prog` from (pc0, c1_0, c2_0); return decoded (c1, c2) at Halt.
fn run_minsky(prog: &[MInstr], pc0: usize, c1_0: usize, c2_0: usize, fuel: u64) -> (usize, usize) {
    validate_prog(prog, pc0); // item b: reject any index that would not fit i64
    let u = compile_u(prog);
    let init_stack = vec![
        counter(c1_0),
        counter(c2_0),
        Value::Int(pc_int(pc0)),
        Value::Quote(u),
    ];
    let program = vec![dup(), apply()]; // the `:!` loop driver
    let vm = Vm::with_stack(init_stack, program);
    match run(vm, fuel) {
        Outcome::Halt(stack) => {
            assert!(stack.len() >= 2, "halt stack too short: {:?}", stack);
            let c1 = decode_counter(&stack[0]);
            let c2 = decode_counter(&stack[1]);
            (c1, c2)
        }
        other => panic!("expected Halt, got {:?}", other),
    }
}

// A tiny reference two-counter Minsky interpreter, to cross-check the MTL run.
// Same model as `minsky_step` and the compiler: a `finite-code Minsky machine
// with implicit halt outside the code domain` — `pc >= prog.len()` HALTS.
fn ref_minsky(prog: &[MInstr], mut pc: usize, mut c1: usize, mut c2: usize) -> (usize, usize) {
    for _ in 0..1_000_000 {
        if pc >= prog.len() {
            return (c1, c2); // implicit halt outside the code domain
        }
        match prog[pc] {
            MInstr::Halt => return (c1, c2),
            MInstr::Inc(false, j) => {
                c1 += 1;
                pc = j;
            }
            MInstr::Inc(true, j) => {
                c2 += 1;
                pc = j;
            }
            MInstr::DecJz(false, jz, nz) => {
                if c1 == 0 {
                    pc = jz;
                } else {
                    c1 -= 1;
                    pc = nz;
                }
            }
            MInstr::DecJz(true, jz, nz) => {
                if c2 == 0 {
                    pc = jz;
                } else {
                    c2 -= 1;
                    pc = nz;
                }
            }
        }
    }
    panic!("reference machine did not halt");
}

// ============================================================
// Machine 1: two increments of c1, then Halt.
//   pc0: Inc(c1, 1) ; pc1: Inc(c1, 2) ; pc2: Halt
// ============================================================
#[test]
fn minsky_double_inc_c1() {
    let prog = [
        MInstr::Inc(false, 1),
        MInstr::Inc(false, 2),
        MInstr::Halt,
    ];
    for start in 0..5usize {
        let got = run_minsky(&prog, 0, start, 0, 100_000);
        let expect = ref_minsky(&prog, 0, start, 0);
        assert_eq!(got, expect, "start c1={}", start);
        assert_eq!(got, (start + 2, 0));
    }
}

// ============================================================
// Machine 2: addition c1 := c1 + c2 (drains c2 into c1).
//   pc0: DecJz(c2, jz=2, nz=1)   // c2==0 -> halt; else c2--, goto 1
//   pc1: Inc(c1, 0)              // c1++, jump back to head
//   pc2: Halt
// ============================================================
#[test]
fn minsky_addition() {
    let prog = [
        MInstr::DecJz(true, 2, 1),
        MInstr::Inc(false, 0),
        MInstr::Halt,
    ];
    for (a, b) in [(0, 0), (0, 3), (2, 0), (2, 3), (5, 4), (1, 7)] {
        let got = run_minsky(&prog, 0, a, b, 1_000_000);
        let expect = ref_minsky(&prog, 0, a, b);
        assert_eq!(got, expect, "a={} b={}", a, b);
        assert_eq!(got, (a + b, 0), "a={} b={}", a, b);
    }
}

// ============================================================
// Machine 3: clear c1 (decrement-to-zero loop), leaving c2 untouched.
//   pc0: DecJz(c1, jz=1, nz=0)   // c1==0 -> halt; else c1--, loop
//   pc1: Halt
// ============================================================
#[test]
fn minsky_clear_c1() {
    let prog = [MInstr::DecJz(false, 1, 0), MInstr::Halt];
    for (a, b) in [(0, 0), (4, 2), (7, 1), (10, 5)] {
        let got = run_minsky(&prog, 0, a, b, 1_000_000);
        let expect = ref_minsky(&prog, 0, a, b);
        assert_eq!(got, expect, "a={} b={}", a, b);
        assert_eq!(got, (0, b), "a={} b={}", a, b);
    }
}

// ============================================================
// Machine 4: larger addition operands, exercising both registers' Inc and DecJz
// repeatedly over bigger counts.
// Confirms that counter magnitude is carried by quotation length rather than by
// the i64 payload, and exercises growth beyond the tiny smoke cases.
//   (A finite run at 57 does NOT confirm unboundedness — the concrete interpreter
//   uses Vec<Word>, bounded by usize/memory; unboundedness is a property of the
//   idealized semantics, proved in Verus, not of any finite executable run.)
// ============================================================
#[test]
fn minsky_addition_large() {
    let prog = [
        MInstr::DecJz(true, 2, 1),
        MInstr::Inc(false, 0),
        MInstr::Halt,
    ];
    let got = run_minsky(&prog, 0, 20, 37, 5_000_000);
    assert_eq!(got, (57, 0));
}

// ============================================================
// Item (h): boundary-at-a-time differential layer.
//
// Instead of only checking the FINAL decoded counters, step the MTL interpreter
// one dispatch-loop iteration at a time (`run_to_next_boundary`) and compare
// against ONE reference Minsky step, re-checking the representation invariant
// `R` at every `[Dup, Apply]` boundary. This exercises every instruction form
// across many counter values and jump targets — including the executable analog
// of the proof's `p5_stutter_step` (one Minsky step <-> one bounded MTL segment)
// and its boundary-preservation / no-spurious-halt content.
// ============================================================

#[derive(Debug, PartialEq, Eq)]
enum BoundaryStep {
    /// Reached the next loop boundary with decoded config `(pc, c1, c2)`.
    Boundary { pc: usize, c1: usize, c2: usize },
    /// The MTL machine halted; decoded output counters `(c1, c2)`.
    Halted { c1: usize, c2: usize },
}

/// One reference Minsky step (single transition; `prog[pc]` must be a running
/// instruction — never `Halt`, `pc < prog.len()`). Mirrors `minsky_step`.
fn ref_step(prog: &[MInstr], pc: usize, c1: usize, c2: usize) -> (usize, usize, usize) {
    match prog[pc] {
        MInstr::Inc(false, j) => (j, c1 + 1, c2),
        MInstr::Inc(true, j) => (j, c1, c2 + 1),
        MInstr::DecJz(false, jz, nz) => {
            if c1 == 0 { (jz, 0, c2) } else { (nz, c1 - 1, c2) }
        }
        MInstr::DecJz(true, jz, nz) => {
            if c2 == 0 { (jz, c1, 0) } else { (nz, c1, c2 - 1) }
        }
        MInstr::Halt => unreachable!("ref_step called on Halt"),
    }
}

/// True when `cont` is exactly the loop-driver `[Dup, Apply]` — the only place a
/// length-2 `[Dup, Apply]` continuation arises is a genuine loop boundary.
fn is_boundary_cont(cont: &[Word]) -> bool {
    cont.len() == 2 && cont[0] == dup() && cont[1] == apply()
}

/// Build the initial VM at the first loop boundary for `(pc0, c1_0, c2_0)`.
fn init_vm(prog: &[MInstr], pc0: usize, c1_0: usize, c2_0: usize) -> Vm {
    let u = compile_u(prog);
    Vm::with_stack(
        vec![counter(c1_0), counter(c2_0), Value::Int(pc_int(pc0)), Value::Quote(u)],
        vec![dup(), apply()],
    )
}

/// Assert the representation invariant `R` holds at a loop boundary: stack is
/// `[Quote(unary c1), Quote(unary c2), Int(pc), Quote(U)]` and cont `[Dup, Apply]`.
fn assert_rep(vm: &Vm, pc: usize, c1: usize, c2: usize) {
    assert!(is_boundary_cont(&vm.cont), "rep: cont not [Dup, Apply], got {:?}", vm.cont);
    assert_eq!(vm.stack.len(), 4, "rep: stack must be [c1,c2,pc,U], got {:?}", vm.stack);
    assert_eq!(decode_counter(&vm.stack[0]), c1, "rep: c1 mismatch");
    assert_eq!(decode_counter(&vm.stack[1]), c2, "rep: c2 mismatch");
    match &vm.stack[2] {
        Value::Int(p) => assert_eq!(*p, pc_int(pc), "rep: pc mismatch"),
        other => panic!("rep: pc slot is not Int: {:?}", other),
    }
    assert!(matches!(&vm.stack[3], Value::Quote(_)), "rep: U slot is not a Quote");
}

/// Step the MTL interpreter from one loop boundary to the NEXT boundary (one
/// simulated Minsky step) or to `Halt`, re-checking `R`'s stack shape on arrival.
fn run_to_next_boundary(vm: &mut Vm, max_steps: usize) -> BoundaryStep {
    // Step off the current boundary first (consume `Dup`), then advance until the
    // next boundary or Halt. A single boundary-to-boundary segment is a fixed,
    // finite number of steps (~6*pc + entry + handler), so this always terminates.
    for _ in 0..max_steps {
        match exec_step(vm) {
            Step::Next => {
                if is_boundary_cont(&vm.cont) {
                    assert_eq!(vm.stack.len(), 4, "boundary rep: stack len, got {:?}", vm.stack);
                    let c1 = decode_counter(&vm.stack[0]);
                    let c2 = decode_counter(&vm.stack[1]);
                    let pc = match &vm.stack[2] {
                        Value::Int(p) => usize::try_from(*p).expect("boundary pc negative"),
                        other => panic!("boundary pc slot not Int: {:?}", other),
                    };
                    assert!(matches!(&vm.stack[3], Value::Quote(_)), "boundary U slot not Quote");
                    return BoundaryStep::Boundary { pc, c1, c2 };
                }
            }
            Step::Halt => {
                assert!(vm.stack.len() >= 2, "halt stack too short: {:?}", vm.stack);
                let c1 = decode_counter(&vm.stack[0]);
                let c2 = decode_counter(&vm.stack[1]);
                return BoundaryStep::Halted { c1, c2 };
            }
            other => panic!("unexpected step outcome mid-loop: {:?}", other),
        }
    }
    panic!("run_to_next_boundary exceeded {} steps (no boundary/halt)", max_steps);
}

/// Boundary-at-a-time differential check, bounded to `max_msteps` Minsky steps:
/// at every boundary the MTL config must equal the reference config, `R` must
/// hold, and the two must agree on WHEN to halt (no spurious early halt, no
/// missed halt). If neither side halts within the bound, they still agreed at
/// every boundary — a genuine partial-run cross-check.
fn differential_bounded(
    prog: &[MInstr], pc0: usize, c1_0: usize, c2_0: usize, max_msteps: usize,
) {
    validate_prog(prog, pc0);
    let (mut rpc, mut rc1, mut rc2) = (pc0, c1_0, c2_0);
    let mut vm = init_vm(prog, pc0, c1_0, c2_0);
    assert_rep(&vm, rpc, rc1, rc2); // R at the initial boundary
    for _ in 0..max_msteps {
        let ref_halted = rpc >= prog.len() || matches!(prog[rpc], MInstr::Halt);
        match run_to_next_boundary(&mut vm, 500_000) {
            BoundaryStep::Halted { c1, c2 } => {
                // no-spurious-halt: MTL only halts where the reference does.
                assert!(ref_halted, "spurious MTL halt at running ref config pc={} c1={} c2={}", rpc, rc1, rc2);
                assert_eq!((c1, c2), (rc1, rc2), "halt output mismatch");
                return;
            }
            BoundaryStep::Boundary { pc, c1, c2 } => {
                // halt-preservation: MTL kept running only where the reference does.
                assert!(!ref_halted, "MTL continued past a reference halt at pc={}", rpc);
                let (npc, nc1, nc2) = ref_step(prog, rpc, rc1, rc2);
                assert_eq!((pc, c1, c2), (npc, nc1, nc2), "boundary config mismatch");
                assert_rep(&vm, npc, nc1, nc2); // R re-established at the new boundary
                rpc = npc;
                rc1 = nc1;
                rc2 = nc2;
            }
        }
    }
    // Reached the step bound without halting — every boundary agreed. OK.
}

// ---- boundary-differential on the four named machines, over many inputs ----
#[test]
fn boundary_differential_named_machines() {
    let double_inc = vec![MInstr::Inc(false, 1), MInstr::Inc(false, 2), MInstr::Halt];
    let addition = vec![MInstr::DecJz(true, 2, 1), MInstr::Inc(false, 0), MInstr::Halt];
    let clear = vec![MInstr::DecJz(false, 1, 0), MInstr::Halt];
    for start in 0..6usize {
        differential_bounded(&double_inc, 0, start, 0, 200);
    }
    for a in 0..6usize {
        for b in 0..6usize {
            differential_bounded(&addition, 0, a, b, 400);
            differential_bounded(&clear, 0, a, b, 400);
        }
    }
    // out-of-range initial PC (implicit halt outside the code domain): immediate halt.
    differential_bounded(&clear, 5, 3, 4, 10);
}

// ---- proptest generated coverage over bounded machines ----
fn instr_strategy(len: usize) -> impl Strategy<Value = MInstr> {
    // jump targets range over 0..=len — `len` is the legal implicit-halt target.
    prop_oneof![
        (any::<bool>(), 0..len + 1).prop_map(|(r, j)| MInstr::Inc(r, j)),
        (any::<bool>(), 0..len + 1, 0..len + 1).prop_map(|(r, jz, nz)| MInstr::DecJz(r, jz, nz)),
        Just(MInstr::Halt),
    ]
}

fn prog_strategy() -> impl Strategy<Value = Vec<MInstr>> {
    (1usize..6).prop_flat_map(|len| proptest::collection::vec(instr_strategy(len), len))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// Every generated bounded machine: the MTL construction agrees with the
    /// reference Minsky evaluator at EVERY loop boundary (config + halt timing),
    /// with `R` preserved, for up to 40 simulated steps across all instruction
    /// forms and jump targets (including out-of-range = implicit halt).
    #[test]
    fn prop_boundary_differential(
        prog in prog_strategy(),
        pc0 in 0usize..6,
        c1 in 0usize..6,
        c2 in 0usize..6,
    ) {
        differential_bounded(&prog, pc0, c1, c2, 40);
    }
}
