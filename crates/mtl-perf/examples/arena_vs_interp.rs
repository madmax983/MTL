//! Arena-vs-interp COMPARATIVE measurement harness — production backend.
//!
//! Times the reference interpreter (`mtl_core::interp::run`, BEFORE) against the
//! PRODUCTION arena backend (`mtl_arena::run_arena`, AFTER) on the four
//! PERF-BASELINE stress cases, plus a fork-cost microbenchmark. Prints markdown
//! tables to stdout. Run with:
//!
//!   cargo run --release --example arena_vs_interp -p mtl-perf
//!
//! This is the production analogue of the spike's
//! `crates/mtl-arena-spike/examples/perf_arena.rs`. The four stress cases and the
//! fork microbench are the SAME; the only change is the backend under test:
//! `mtl_arena` (production hygiene: checked arithmetic, `Option`-returning
//! compile, reference-typed reification) instead of `mtl_arena_spike`.
//!
//! Timing is monotonic (`std::time::Instant`), best-of-N with one warmup. Both
//! backends run in the same process/run on the same container, so trust RATIOS
//! and growth curves, not absolute ns. Arena timings INCLUDE one-time program
//! compile/interning (a handicap, if anything); interp timings include `Vm`
//! construction. Bench/example code may unwrap/index freely.

use std::hint::black_box;
use std::time::Instant;

use mtl_arena as arena;
use mtl_core::interp::{run, Prim, Value, Vm, Word};
use mtl_perf::{drive, fold_sum, primrec_sumto, selfapp_countdown, straightline};

const FUEL: u64 = 200_000_000;

/// Which engine arms to time. The DEFAULT is [`Sel::Both`] — this is a
/// COMPARATIVE harness, so it runs both engines by default (arena is now the
/// production default engine; interp is the differential anchor "before"). The
/// `--engine=arena|interp` flag restricts to a single arm for focused timing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sel {
    Both,
    Arena,
    Interp,
}

impl Sel {
    fn runs_interp(self) -> bool {
        matches!(self, Sel::Both | Sel::Interp)
    }
    fn runs_arena(self) -> bool {
        matches!(self, Sel::Both | Sel::Arena)
    }
}

/// Parse `--engine=arena|interp` from argv; absence ⇒ [`Sel::Both`].
fn parse_sel() -> Sel {
    for a in std::env::args().skip(1) {
        if let Some(val) = a.strip_prefix("--engine=") {
            match arena::Engine::parse(val) {
                Ok(arena::Engine::Arena) => return Sel::Arena,
                Ok(arena::Engine::Interp) => return Sel::Interp,
                Err(msg) => {
                    eprintln!("arena_vs_interp: {msg}");
                    std::process::exit(2);
                }
            }
        }
    }
    Sel::Both
}

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

fn to_arena(full: &[Word]) -> Vec<arena::ProgWord> {
    full.iter().map(conv_word).collect()
}

// ---------------------------------------------------- timing
/// Best-of-`reps` total wall-ns for one full interp run (Vm construction + run).
fn time_interp(init: &[Value], prog: &[Word], reps: u32) -> (u64, f64) {
    let stack = init.to_vec();
    let steps = drive(Vm::with_stack(stack.clone(), prog.to_vec()), FUEL).steps;
    black_box(run(Vm::with_stack(stack.clone(), prog.to_vec()), FUEL)); // warmup
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

/// Best-of-`reps` total wall-ns for one full production arena run (compile + run).
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
    i_ns: f64,              // interp total ns; NaN if capped
    a_steps: u64,
    a_ns: f64,              // arena total ns
    projected: Option<f64>, // projected interp ns when capped
}

/// Time one interp arm, honouring the engine selection: when interp is not
/// selected, return the `(0, NaN)` sentinel that [`print_table`] renders as `n/a`.
fn time_interp_sel(sel: Sel, init: &[Value], prog: &[Word], reps: u32) -> (u64, f64) {
    if sel.runs_interp() {
        time_interp(init, prog, reps)
    } else {
        (0, f64::NAN)
    }
}

/// Time one arena arm, honouring the engine selection.
fn time_arena_sel(sel: Sel, full: &[arena::ProgWord], reps: u32) -> (u64, f64) {
    if sel.runs_arena() {
        time_arena(full, reps)
    } else {
        (0, f64::NAN)
    }
}

fn print_table(title: &str, note: &str, rows: &[Row]) {
    println!("### {}\n", title);
    println!("{}\n", note);
    println!("| scale | interp steps | interp total ms | interp ns/step | arena steps | arena total ms | arena ns/step | speedup |");
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|");
    for r in rows {
        if r.a_ns.is_nan() {
            // Arena arm not selected (interp-only run).
            let i_nsps = r.i_ns / r.i_steps as f64;
            println!(
                "| {} | {} | {:.4} | {:.1} | n/a | n/a | n/a | n/a |",
                r.scale, r.i_steps, ms(r.i_ns), i_nsps,
            );
            continue;
        }
        let a_nsps = r.a_ns / r.a_steps as f64;
        if r.i_ns.is_nan() {
            // Interp arm capped/projected or not selected.
            match r.projected {
                Some(proj) => println!(
                    "| {} | {} | n/a (proj ~{:.1}s) | n/a | {} | {:.4} | {:.1} | proj ~{:.0}× |",
                    r.scale, r.i_steps, proj / 1e9, r.a_steps, ms(r.a_ns), a_nsps, proj / r.a_ns,
                ),
                None => println!(
                    "| {} | n/a | n/a | n/a | {} | {:.4} | {:.1} | n/a |",
                    r.scale, r.a_steps, ms(r.a_ns), a_nsps,
                ),
            }
        } else {
            let i_nsps = r.i_ns / r.i_steps as f64;
            println!(
                "| {} | {} | {:.4} | {:.1} | {} | {:.4} | {:.1} | {:.1}× |",
                r.scale, r.i_steps, ms(r.i_ns), i_nsps, r.a_steps, ms(r.a_ns), a_nsps,
                r.i_ns / r.a_ns,
            );
        }
    }
    println!();
}

fn main() {
    let sel = parse_sel();
    println!("# MTL v0.5 arena backend — PRODUCTION arena-vs-interp measurements\n");
    println!("BEFORE = `mtl_core::interp::run` (Vec continuation).");
    println!("AFTER  = `mtl_arena::run_arena` (PRODUCTION segment-cursor arena continuation).");
    println!("Same machine, same process, monotonic timer, best-of-N with warmup.");
    println!("Shared cloud container: trust RATIOS and growth curves, not absolute ns.\n");

    // (a) flat 1 1 + _ : N steps in {64,256,1024,4096,16384} (units = N/4)
    let mut flat = Vec::new();
    for &n in &[64usize, 256, 1024, 4096, 16384] {
        let units = n / 4;
        let prog = straightline(units);
        let full = to_arena(&prog);
        let reps = if n <= 1024 { 200 } else { 20 };
        let (is, i_ns) = time_interp_sel(sel, &[], &prog, reps);
        let (as_, a_ns) = time_arena_sel(sel, &full, reps.max(50));
        flat.push(Row { scale: format!("N={}", n), i_steps: is, i_ns, a_steps: as_, a_ns, projected: None });
    }
    print_table(
        "(a) Flat straight-line `1 1 + _` × units — the `cont.remove(0)` front-pop pathology",
        "Whole program sits in `cont`; interp pays O(n) front-pop every step. Arena replaces it with an O(1) cursor bump.",
        &flat,
    );

    // (c) PrimRec sum_to: n in {1000,10000,100000}. Cap interp at 10k, project 100k.
    let mut pr = Vec::new();
    let mut interp_10k_ns = 0.0f64;
    for &n in &[1000i64, 10000, 100000] {
        let (init, prog) = primrec_sumto(n);
        let full = to_arena(&full_prog(&init, &prog));
        let (as_, a_ns) = time_arena_sel(sel, &full, 20);
        if n <= 10000 {
            let reps = if n <= 1000 { 20 } else { 3 };
            let (is, i_ns) = time_interp_sel(sel, &init, &prog, reps);
            if n == 10000 {
                interp_10k_ns = i_ns;
            }
            pr.push(Row { scale: format!("n={}", n), i_steps: is, i_ns, a_steps: as_, a_ns, projected: None });
        } else {
            // Interp step count is deterministic (sum_to(n) = 6n+4 executed
            // words); driving the O(n²) interp run at 100k just to *count* steps
            // would cost tens of seconds of pure overhead, so derive it.
            let is = (6 * n + 4) as u64;
            let _ = &init; // interp is not run at this (capped) size
            let _ = &prog;
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

    // (d) Fold sum: n in {1000,10000,100000}. Cap interp at 10k, project 100k.
    let mut fd = Vec::new();
    let mut fold_10k_ns = 0.0f64;
    for &n in &[1000usize, 10000, 100000] {
        let (init, prog) = fold_sum(n);
        let full = to_arena(&full_prog(&init, &prog));
        let (as_, a_ns) = time_arena_sel(sel, &full, 20);
        if n <= 10000 {
            let reps = if n <= 1000 { 20 } else { 5 };
            let (is, i_ns) = time_interp_sel(sel, &init, &prog, reps);
            if n == 10000 {
                fold_10k_ns = i_ns;
            }
            fd.push(Row { scale: format!("n={}", n), i_steps: is, i_ns, a_steps: as_, a_ns, projected: None });
        } else {
            fd.push(Row {
                scale: format!("n={}", n),
                i_steps: as_,
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

    // (b) deep `: !` countdown: n in {1000,10000} — linear for both (the fourth case).
    let mut dp = Vec::new();
    for &n in &[1000i64, 10000] {
        let (init, prog) = selfapp_countdown(n);
        let full = to_arena(&full_prog(&init, &prog));
        let (is, i_ns) = time_interp_sel(sel, &init, &prog, 10);
        let (as_, a_ns) = time_arena_sel(sel, &full, 20);
        dp.push(Row { scale: format!("n={}", n), i_steps: is, i_ns, a_steps: as_, a_ns, projected: None });
    }
    print_table(
        "(b) Deep `: !` self-application countdown — the baseline's NON-pathology (tail-linear)",
        "Both backends are O(n): interp's splice lands on an empty tail (cont stays ~6 words). Included to confirm the arena does not regress the already-good case.",
        &dp,
    );

    // fork-cost microbenchmark
    fork_table(sel);

    println!("---\n");
    println!("Production numbers. Compare against the spike's claimed figures in");
    println!("bench/design-v0.5/MEASUREMENTS.md and crates/mtl-perf/PERF-BASELINE.md.");
}

// ---------------------------------------------------- fork-cost microbenchmark
/// Build a depth-`d` persistent arena stack and return the machine position that
/// sits on top of it. Running `d` `PushInt`s leaves a depth-`d` `StackArena`
/// cons-list; the returned `VmState` (3×u32 = 12 bytes, `Copy`) is what a fork
/// copies — the production analogue of the spike's `build_stack(d)`.
fn arena_stack_state(d: usize) -> (arena::Vm, arena::VmState) {
    let prog: Vec<arena::ProgWord> = (0..d).map(|i| arena::ProgWord::PushInt(i as i64)).collect();
    let run = arena::run_arena(&prog, FUEL);
    (run.vm, run.state)
}

fn fork_table(sel: Sel) {
    println!("### Fork-cost microbenchmark — clone a machine position at stack depth d\n");
    println!("BEFORE: `clone()` a persistent-free `interp::Vm` holding a depth-d stack + a small cont (O(d) Vec clone). AFTER: copy an arena `VmState` (3×u32 = 12 bytes) sitting on a depth-d persistent stack (O(1), depth-independent).\n");
    println!("| stack depth d | interp Vm.clone() ns | arena VmState copy ns |");
    println!("|---:|---:|---:|");
    for &d in &[1usize, 10, 100, 1000, 10000] {
        // BEFORE: interp Vm with depth-d stack + representative cont.
        let clone_ns = if sel.runs_interp() {
            let stack: Vec<Value> = (0..d).map(|i| Value::Int(i as i64)).collect();
            let cont = vec![Word::PushInt(1), Word::Prim(Prim::Add)];
            let vm = Vm::with_stack(stack, cont);
            let iters: u64 = if d >= 1000 { 20_000 } else { 500_000 };
            black_box(vm.clone());
            let t = Instant::now();
            for _ in 0..iters {
                black_box(vm.clone());
            }
            t.elapsed().as_nanos() as f64 / iters as f64
        } else {
            f64::NAN
        };

        // AFTER: arena VmState copy at depth d (fork = 12-byte Copy).
        let copy_ns = if sel.runs_arena() {
            let (_avm, st) = arena_stack_state(d);
            let citers: u64 = 5_000_000;
            black_box(st);
            let t = Instant::now();
            for _ in 0..citers {
                let s2 = black_box(st);
                black_box(s2);
            }
            t.elapsed().as_nanos() as f64 / citers as f64
        } else {
            f64::NAN
        };

        match (clone_ns.is_nan(), copy_ns.is_nan()) {
            (false, false) => println!("| {} | {:.1} | {:.2} |", d, clone_ns, copy_ns),
            (false, true) => println!("| {} | {:.1} | n/a |", d, clone_ns),
            (true, false) => println!("| {} | n/a | {:.2} |", d, copy_ns),
            (true, true) => println!("| {} | n/a | n/a |", d),
        }
    }
    println!();
}
