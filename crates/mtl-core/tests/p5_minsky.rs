//! P5 empirical validation — concrete two-counter Minsky machines compiled to
//! MTL and executed on the REAL interpreter (`mtl_core::interp::run`).
//!
//! This is the executable counterpart of the Verus proof in
//! `crates/mtl-core/src/p5_universality.rs`. It implements the SAME construction
//! the proof reasons about (unary quotation counters + a single self-applying
//! `:!` dispatch-loop quote `U`) as plain `Vec<Word>` programs, then runs them
//! through the fuel-bounded interpreter and decodes the resulting counter
//! quotations. It is concrete evidence that the construction actually computes.
//!
//! Layout invariant between loop iterations (top at right):
//!   stack = [ Quote(unary(c1)), Quote(unary(c2)), Int(pc), Quote(U) ]
//!   cont  = [ Dup, Apply ]
//! Counters live in quotation LENGTH (unbounded Seq), never in an Int — the P5
//! honesty pivot. (The `i64` bound still caps the PC and marker VALUE, not the
//! counter magnitude.)

use mtl_core::interp::build::*;
use mtl_core::interp::{run, Outcome, Value, Vm, Word};

// ---- Minsky machine (mirror of the ghost `MInstr`/`MProg` in the proof) ----

#[derive(Clone, Copy, Debug)]
enum MInstr {
    /// Inc(reg=false→c1 / true→c2, next pc)
    Inc(bool, usize),
    /// DecJz(reg, jz target, nz target)
    DecJz(bool, usize, usize),
    Halt,
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
            b.extend(vec![int(j as i64), swap(), dup(), apply()]);
            b
        }
        MInstr::DecJz(reg, jz, nz) => {
            let mut b = vec![drop()]; // -> [c1, c2, U]
            if reg {
                // c2: swap -> [c1, U, c2]; counter on top.
                b.push(swap());
                // THEN (nonzero): [c1,U,junk,c2'] -> restore, pc:=nz, loop.
                let then_q = vec![swap(), drop(), swap(), int(nz as i64), swap(), dup(), apply()];
                // ELSE (zero): [c1,U] -> re-push empty c2, pc:=jz, loop.
                let else_q = vec![quote(vec![]), swap(), int(jz as i64), swap(), dup(), apply()];
                b.extend(vec![uncons(), quote(then_q), quote(else_q), iff()]);
            } else {
                // c1: rot -> [c2, U, c1]; counter on top.
                b.push(rot());
                // THEN (nonzero): [c2,U,junk,c1'] -> restore, pc:=nz, loop.
                let then_q =
                    vec![swap(), drop(), rot(), rot(), int(nz as i64), swap(), dup(), apply()];
                // ELSE (zero): [c2,U] -> re-push empty c1, pc:=jz, loop.
                let else_q =
                    vec![quote(vec![]), rot(), rot(), int(jz as i64), swap(), dup(), apply()];
                b.extend(vec![uncons(), quote(then_q), quote(else_q), iff()]);
            }
            b
        }
    }
}

// ---- the dispatch cascade DISP(i) and the loop quote U ----
fn disp(prog: &[MInstr], i: usize) -> Vec<Word> {
    if i >= prog.len() {
        // pc out of range: drop pc, drop U -> [c1,c2]; drain -> Halt.
        return vec![drop(), drop()];
    }
    // [c1,c2,U,pc] -> dup pc, compare to i; if equal splice BODY_i else DISP(i+1)
    vec![
        dup(),
        int(i as i64),
        eq(),
        quote(body(prog[i])),
        quote(disp(prog, i + 1)),
        iff(),
    ]
}
fn compile_u(prog: &[MInstr]) -> Vec<Word> {
    // entry to U: [c1,c2,Int(pc),U]; swap brings pc to top -> [c1,c2,U,pc].
    let mut u = vec![swap()];
    u.extend(disp(prog, 0));
    u
}

/// Run `prog` from (pc0, c1_0, c2_0); return decoded (c1, c2) at Halt.
fn run_minsky(prog: &[MInstr], pc0: usize, c1_0: usize, c2_0: usize, fuel: u64) -> (usize, usize) {
    let u = compile_u(prog);
    let init_stack = vec![
        counter(c1_0),
        counter(c2_0),
        Value::Int(pc0 as i64),
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
fn ref_minsky(prog: &[MInstr], mut pc: usize, mut c1: usize, mut c2: usize) -> (usize, usize) {
    for _ in 0..1_000_000 {
        if pc >= prog.len() {
            return (c1, c2);
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
// Machine 4: multiply-ish stress — copy c2 into c1 twice using c1 as sink,
// exercising both registers' Inc and DecJz repeatedly over larger counts,
// confirming the unbounded (length-encoded) counters behave past small values.
//   Program: c1 += 2*c2 via nested loop is awkward with 2 counters; instead
//   just run addition on larger operands to show length grows unbounded.
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
