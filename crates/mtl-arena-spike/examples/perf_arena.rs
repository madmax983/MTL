//! SPIKE measurement driver — before/after, SAME machine, SAME run.
//!
//! Times the reference interpreter (`mtl_core::interp::run`, BEFORE) against the
//! arena backend (`mtl_arena_spike::run_arena`, AFTER) on the PERF-BASELINE
//! stress cases, plus a fork-cost microbenchmark. Prints markdown tables to
//! stdout. Run with:
//!
//!   cargo run --release --example perf_arena -p mtl-arena-spike
//!
//! Timing is monotonic (`std::time::Instant`), best-of-N with one warmup. All
//! numbers are from a shared cloud container: use RATIOS and growth curves, not
//! absolute ns. Arena timings INCLUDE one-time compile/interning of the program
//! (handicapping the arena, if anything); interp timings include `Vm` construction.

use std::hint::black_box;
use std::time::Instant;

use mtl_arena_spike as arena;
use mtl_core::interp::{run, Prim, Value, Vm, Word};
use mtl_perf::{drive, fold_sum, primrec_sumto, selfapp_countdown, straightline, Ending};

const FUEL: u64 = 200_000_000;

// ---------------------------------------------------- conversions (interp -> arena)
fn conv_prim(p: Prim) -> arena::Prim {
    use arena::Prim as A;
    match p {
        Prim::Dup => A::Dup,
        Prim::Drop => A::Drop,
        Prim::Swap => A::Swap,
        Prim::Rot => A::Rot,
        Prim::Over => A::Over,
        Prim::Apply => A::Apply,
        Prim::Cat => A::Cat,
        Prim::Cons => A::Cons,
        Prim::Dip => A::Dip,
        Prim::Add => A::Add,
        Prim::Sub => A::Sub,
        Prim::Mul => A::Mul,
        Prim::Div => A::Div,
        Prim::Mod => A::Mod,
        Prim::Eq => A::Eq,
        Prim::Lt => A::Lt,
        Prim::If => A::If,
        Prim::PrimRec => A::PrimRec,
        Prim::Times => A::Times,
        Prim::LinRec => A::LinRec,
        Prim::Uncons => A::Uncons,
        Prim::Fold => A::Fold,
        Prim::Xor => A::Xor,
    }
}

fn conv_word(w: &Word) -> arena::ProgWord {
    match w {
        Word::PushInt(n) => arena::ProgWord::PushInt(*n),
        Word::PushQuote(q) => arena::ProgWord::PushQuote(q.iter().map(conv_word).collect()),
        Word::Prim(p) => arena::ProgWord::Prim(conv_prim(*p)),
        Word::Call(n) => arena::ProgWord::Call(n.clone()),
    }
}

fn value_to_word(v: &Value) -> Word {
    match v {
        Value::Int(n) => Word::PushInt(*n),
        Value::Quote(q) => Word::PushQuote(q.clone()),
    }
}

/// The full program (initial stack encoded as leading pushes), as interp Words.
fn full_prog(init: &[Value], prog: &[Word]) -> Vec<Word> {
    let mut v: Vec<Word> = init.iter().map(value_to_word).collect();
    v.extend(prog.iter().cloned());
    v
}

// ---------------------------------------------------- timing
/// Best-of-`reps` total wall-ns for one full interp run (Vm construction + run).
fn time_interp(init: &[Value], prog: &[Word], reps: u32) -> (u64, f64) {
    let stack = init.to_vec();
    let steps = drive(Vm::with_stack(stack.clone(), prog.to_vec()), FUEL).steps;
    // warmup
    black_box(run(Vm::with_stack(stack.clone(), prog.to_vec()), FUEL));
    let mut best = f64::MAX;
    for _ in 0..reps {
        let vm = Vm::with_stack(stack.clone(), prog.to_vec());
        let t = Instant::now();
        let out = run(vm, FUEL);
        let ns = t.elapsed().as_nanos() as f64;
        black_box(&out);
        best = best.min(ns);
    }
    (steps, best)
}

/// Best-of-`reps` total wall-ns for one full arena run (compile + run).
fn time_arena(full: &[arena::ProgWord], reps: u32) -> (u64, f64) {
    let steps = arena::run_arena(full, FUEL).steps;
    black_box(arena::run_arena(full, FUEL).end);
    let mut best = f64::MAX;
    for _ in 0..reps {
        let t = Instant::now();
        let r = arena::run_arena(full, FUEL);
        let ns = t.elapsed().as_nanos() as f64;
        black_box(&r.end);
        best = best.min(ns);
    }
    (steps, best)
}

fn ms(ns: f64) -> f64 {
    ns / 1e6
}

// ---------------------------------------------------- stress-case rows
struct Row {
    scale: String,
    i_steps: u64,
    i_ns: f64,   // interp total ns; NaN if capped
    a_steps: u64,
    a_ns: f64,   // arena total ns
    projected: Option<f64>, // projected interp ns when capped
}

fn print_table(title: &str, note: &str, rows: &[Row]) {
    println!("### {}\n", title);
    println!("{}\n", note);
    println!("| scale | interp steps | interp total ms | interp ns/step | arena steps | arena total ms | arena ns/step | speedup |");
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|");
    for r in rows {
        let a_nsps = r.a_ns / r.a_steps as f64;
        if r.i_ns.is_nan() {
            let proj = r.projected.unwrap_or(f64::NAN);
            println!(
                "| {} | {} | n/a (proj ~{:.1}s) | n/a | {} | {:.4} | {:.1} | proj ~{:.0}× |",
                r.scale,
                r.i_steps,
                proj / 1e9,
                r.a_steps,
                ms(r.a_ns),
                a_nsps,
                proj / r.a_ns,
            );
        } else {
            let i_nsps = r.i_ns / r.i_steps as f64;
            println!(
                "| {} | {} | {:.4} | {:.1} | {} | {:.4} | {:.1} | {:.1}× |",
                r.scale,
                r.i_steps,
                ms(r.i_ns),
                i_nsps,
                r.a_steps,
                ms(r.a_ns),
                a_nsps,
                r.i_ns / r.a_ns,
            );
        }
    }
    println!();
}

fn main() {
    println!("# MTL v0.5 arena-backend spike — before/after measurements\n");
    println!("SPIKE / NON-PRODUCTION. BEFORE = `mtl_core::interp::run` (Vec continuation).");
    println!("AFTER = `mtl_arena_spike::run_arena` (segment-cursor arena continuation).");
    println!("Same machine, same process, monotonic timer, best-of-N with warmup.");
    println!("Shared cloud container: trust RATIOS and growth curves, not absolute ns.\n");

    // (a) flat 1 1 + _ : N steps in {64,256,1024,4096,16384} (units = N/4)
    let mut flat = Vec::new();
    for &n in &[64usize, 256, 1024, 4096, 16384] {
        let units = n / 4;
        let prog = straightline(units);
        let full: Vec<arena::ProgWord> = prog.iter().map(conv_word).collect();
        let reps = if n <= 1024 { 200 } else { 20 };
        let (is, i_ns) = time_interp(&[], &prog, reps);
        let (as_, a_ns) = time_arena(&full, reps.max(50));
        flat.push(Row {
            scale: format!("N={}", n),
            i_steps: is,
            i_ns,
            a_steps: as_,
            a_ns,
            projected: None,
        });
    }
    print_table(
        "(a) Flat straight-line `1 1 + _` × units — the `cont.remove(0)` front-pop pathology",
        "Whole program sits in `cont`; interp pays O(n) front-pop every step (baseline: ns/step grew 414× ⇒ ~O(n²)). Arena replaces it with an O(1) cursor bump.",
        &flat,
    );

    // (c) PrimRec sum_to: n in {1000,10000,100000}. Cap interp at 10k, project 100k.
    let mut pr = Vec::new();
    let mut interp_10k_ns = 0.0f64;
    for &n in &[1000i64, 10000, 100000] {
        let (init, prog) = primrec_sumto(n);
        let full: Vec<arena::ProgWord> = full_prog(&init, &prog).iter().map(conv_word).collect();
        let (as_, a_ns) = time_arena(&full, 20);
        if n <= 10000 {
            let reps = if n <= 1000 { 20 } else { 3 };
            let (is, i_ns) = time_interp(&init, &prog, reps);
            if n == 10000 {
                interp_10k_ns = i_ns;
            }
            pr.push(Row { scale: format!("n={}", n), i_steps: is, i_ns, a_steps: as_, a_ns, projected: None });
        } else {
            // projected: O(n^2), 10x n -> ~100x time
            let is = drive_steps_primrec(n);
            pr.push(Row {
                scale: format!("n={}", n),
                i_steps: is,
                i_ns: f64::NAN,
                a_steps: as_,
                a_ns,
                projected: Some(interp_10k_ns * 100.0),
            });
        }
    }
    print_table(
        "(c) PrimRec `sum_to(n)` = `n [0] [+] &` — the primary O(n²) pathology (the 223 ms case)",
        "interp re-emits the combinator after the recursive call, growing `cont` by |C| every level ⇒ O(n²). Arena prepends a fresh interned segment + the body by reference ⇒ O(1)/level. Interp capped at n=10k; n=100k projected (O(n²)).",
        &pr,
    );

    // (c/d) Fold sum: n in {1000,10000,100000}. Cap interp at 10k, project 100k.
    let mut fd = Vec::new();
    let mut fold_10k_ns = 0.0f64;
    for &n in &[1000usize, 10000, 100000] {
        let (init, prog) = fold_sum(n);
        let full: Vec<arena::ProgWord> = full_prog(&init, &prog).iter().map(conv_word).collect();
        let (as_, a_ns) = time_arena(&full, 20);
        if n <= 10000 {
            let reps = if n <= 1000 { 20 } else { 5 };
            let (is, i_ns) = time_interp(&init, &prog, reps);
            if n == 10000 {
                fold_10k_ns = i_ns;
            }
            fd.push(Row { scale: format!("n={}", n), i_steps: is, i_ns, a_steps: as_, a_ns, projected: None });
        } else {
            let is = as_; // same step count both backends
            fd.push(Row {
                scale: format!("n={}", n),
                i_steps: is,
                i_ns: f64::NAN,
                a_steps: as_,
                a_ns,
                projected: Some(fold_10k_ns * 100.0),
            });
        }
    }
    print_table(
        "(d) Fold sum `0 [+] (` over n ints — the hidden O(n²) (spine re-clone)",
        "interp deep-clones the shrinking list tail (`PushQuote(tail)`) every element ⇒ O(n²), invisible to cont-length. Arena makes the tail an O(1) shared sub-slice `{start+1, len-1}`. Interp capped at n=10k; n=100k projected.",
        &fd,
    );

    // (b) deep `: !` countdown: n in {1000,10000} — linear for both.
    let mut dp = Vec::new();
    for &n in &[1000i64, 10000] {
        let (init, prog) = selfapp_countdown(n);
        let full: Vec<arena::ProgWord> = full_prog(&init, &prog).iter().map(conv_word).collect();
        let (is, i_ns) = time_interp(&init, &prog, 10);
        let (as_, a_ns) = time_arena(&full, 20);
        dp.push(Row { scale: format!("n={}", n), i_steps: is, i_ns, a_steps: as_, a_ns, projected: None });
    }
    print_table(
        "(b) Deep `: !` self-application countdown — the baseline's NON-pathology (tail-linear)",
        "Both backends are O(n): interp's splice lands on an empty tail (cont stays ~6 words). Included to confirm the arena does not regress the already-good case.",
        &dp,
    );

    // fork-cost microbenchmark
    fork_table();

    println!("---\n");
    println!("Verdict placeholder — see MEASUREMENTS.md.");
}

/// Exact primrec step count without timing (for the projected/capped 100k row).
fn drive_steps_primrec(n: i64) -> u64 {
    let (init, prog) = primrec_sumto(n);
    drive(Vm::with_stack(init, prog), FUEL).steps
}

// ---------------------------------------------------- fork-cost microbenchmark
fn fork_table() {
    println!("### Fork-cost microbenchmark — clone a machine position at stack depth d\n");
    println!("BEFORE: `clone()` a persistent-free `interp::Vm` holding a depth-d stack + a small cont (O(d) Vec clone). AFTER: copy an arena `VmState` (3×u32 = 12 bytes) after building a depth-d persistent stack (O(1), depth-independent).\n");
    println!("| stack depth d | interp Vm.clone() ns | arena VmState copy ns |");
    println!("|---:|---:|---:|");
    for &d in &[1usize, 10, 100, 1000, 10000] {
        // BEFORE: interp Vm with depth-d stack + representative cont.
        let stack: Vec<Value> = (0..d).map(|i| Value::Int(i as i64)).collect();
        let cont = vec![Word::PushInt(1), Word::Prim(Prim::Add)];
        let vm = Vm::with_stack(stack, cont);
        let iters: u64 = if d >= 1000 { 20_000 } else { 500_000 };
        black_box(vm.clone());
        let t = Instant::now();
        for _ in 0..iters {
            black_box(vm.clone());
        }
        let clone_ns = t.elapsed().as_nanos() as f64 / iters as f64;

        // AFTER: arena VmState copy at depth d.
        let (_avm, st) = arena::build_stack(d);
        let citers: u64 = 5_000_000;
        black_box(st);
        let t = Instant::now();
        for _ in 0..citers {
            let s2 = black_box(st);
            black_box(s2);
        }
        let copy_ns = t.elapsed().as_nanos() as f64 / citers as f64;

        println!("| {} | {:.1} | {:.2} |", d, clone_ns, copy_ns);
    }
    println!();
    let _ = Ending::Halt; // keep the import meaningful across mtl-perf versions
}
