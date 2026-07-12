//! Deterministic measurement driver — produces the numbers in PERF-BASELINE.md.
//! Run with: `cargo run --release --example perf_report -p mtl-perf`
//!
//! Not a criterion bench: it counts exact interpreter steps and peak `cont`
//! length, and reports ns/step growth so the continuation-representation cost is
//! visible. Timing uses best-of-N wall clock with clone overhead subtracted.

use std::time::Instant;

use mtl_bench_validate::load_solution;
use mtl_core::interp::{run, Outcome, Value, Vm, Word};
use mtl_perf::corpus::{corpus_root, tier2_v03_cases, tv0_cases};
use mtl_perf::{
    drive, fold_quotes, fold_sum, gen_source, linrec_countdown, primrec_sumto, selfapp_countdown,
    straightline, times_count, Ending, BIG_FUEL,
};

/// Best-of `reps` wall-ns for one full drive, minus clone overhead. Returns
/// (steps, max_cont, exec_ns).
fn measure(stack: &Vec<Value>, prog: &Vec<Word>, reps: u32) -> (u64, usize, f64) {
    let mut clone_best = f64::MAX;
    for _ in 0..reps {
        let t = Instant::now();
        let vm = Vm::with_stack(stack.clone(), prog.clone());
        let ns = t.elapsed().as_nanos() as f64;
        std::hint::black_box(&vm);
        if ns < clone_best {
            clone_best = ns;
        }
    }
    let mut best = f64::MAX;
    let mut steps = 0u64;
    let mut maxc = 0usize;
    for _ in 0..reps {
        let vm = Vm::with_stack(stack.clone(), prog.clone());
        let t = Instant::now();
        let s = drive(vm, BIG_FUEL);
        let ns = t.elapsed().as_nanos() as f64;
        steps = s.steps;
        maxc = s.max_cont;
        std::hint::black_box(s.outcome);
        if ns < best {
            best = ns;
        }
    }
    (steps, maxc, (best - clone_best).max(0.0))
}

fn reps_for(steps_hint: i64) -> u32 {
    if steps_hint >= 5000 {
        7
    } else {
        60
    }
}

struct Row {
    n: i64,
    steps: u64,
    maxc: usize,
    exec_ns: f64,
}

fn growth(title: &str, gen: impl Fn(i64) -> (Vec<Value>, Vec<Word>), sizes: &[i64]) {
    println!("### {}\n", title);
    println!("| n | steps | peak cont | exec µs | ns/step | steps/sec |");
    println!("|---:|---:|---:|---:|---:|---:|");
    let mut rows: Vec<Row> = Vec::new();
    for &n in sizes {
        let (stack, prog) = gen(n);
        let (steps, maxc, exec_ns) = measure(&stack, &prog, reps_for(n));
        let nsps = if steps > 0 { exec_ns / steps as f64 } else { 0.0 };
        let sps = if exec_ns > 0.0 { steps as f64 / (exec_ns / 1e9) } else { 0.0 };
        println!(
            "| {} | {} | {} | {:.1} | {:.1} | {:.2}M |",
            n,
            steps,
            maxc,
            exec_ns / 1000.0,
            nsps,
            sps / 1e6
        );
        rows.push(Row { n, steps, maxc, exec_ns });
    }
    if rows.len() >= 2 {
        let a = &rows[0];
        let b = rows.last().unwrap();
        let nsps_a = a.exec_ns / a.steps.max(1) as f64;
        let nsps_b = b.exec_ns / b.steps.max(1) as f64;
        let nsps_ratio = nsps_b / nsps_a.max(1e-9);
        let n_ratio = b.n as f64 / a.n as f64;
        let cont_ratio = b.maxc as f64 / (a.maxc.max(1)) as f64;
        let verdict = if nsps_ratio > n_ratio * 0.25 {
            "per-step cost scales ~linearly with n  =>  TOTAL ~O(n^2)"
        } else if nsps_ratio > 3.0 {
            "per-step cost grows sub-linearly  =>  super-linear total"
        } else {
            "per-step cost ~constant  =>  TOTAL ~O(n) (linear)"
        };
        println!(
            "\n> n grew {:.0}x, peak-cont grew {:.0}x, ns/step grew {:.2}x  =>  **{}**\n",
            n_ratio, cont_ratio, nsps_ratio, verdict
        );
    }
}

fn parser_report() {
    println!("### Parser throughput (mtl-syntax::parse)\n");
    println!("| bytes | parse µs | MiB/s |");
    println!("|---:|---:|---:|");
    for &units in &[100usize, 1000, 10000, 50000] {
        let src = gen_source(units);
        let bytes = src.len();
        let reps = 30u32;
        // warm
        std::hint::black_box(mtl_syntax::parse(&src).unwrap().len());
        let mut best = f64::MAX;
        for _ in 0..reps {
            let t = Instant::now();
            let n = mtl_syntax::parse(&src).unwrap().len();
            let ns = t.elapsed().as_nanos() as f64;
            std::hint::black_box(n);
            if ns < best {
                best = ns;
            }
        }
        let mibs = (bytes as f64) / (best / 1e9) / (1024.0 * 1024.0);
        println!("| {} | {:.1} | {:.1} |", bytes, best / 1000.0, mibs);
    }
    println!();
}

fn corpus_report(title: &str, cases: Vec<mtl_perf::corpus::CorpusCase>) {
    println!("### {}\n", title);
    println!("| task | vectors | max steps | peak cont | total µs |");
    println!("|---|---:|---:|---:|---:|");
    let mut ok = 0usize;
    let mut total = 0usize;
    for case in &cases {
        let path = corpus_root().join(case.task).join(case.version).join("solution.mtl");
        let prog = load_solution(&path).unwrap();
        let mut max_steps = 0u64;
        let mut peak_cont = 0usize;
        for (input, expected) in case.inputs.iter().zip(case.expected.iter()) {
            total += 1;
            let s = drive(Vm::with_stack(input.clone(), prog.clone()), 100_000);
            let pass = s.outcome == Ending::Halt && &s.final_stack == expected;
            if pass {
                ok += 1;
            } else {
                println!(
                    "> MISMATCH {} input={:?}: got {:?} ({:?})",
                    case.task, input, s.final_stack, s.outcome
                );
            }
            max_steps = max_steps.max(s.steps);
            peak_cont = peak_cont.max(s.max_cont);
        }
        // timing: all vectors once through
        let reps = 200u32;
        let mut best = f64::MAX;
        for _ in 0..reps {
            let t = Instant::now();
            for input in &case.inputs {
                std::hint::black_box(run(Vm::with_stack(input.clone(), prog.clone()), 100_000));
            }
            let ns = t.elapsed().as_nanos() as f64;
            if ns < best {
                best = ns;
            }
        }
        println!(
            "| {} | {} | {} | {} | {:.2} |",
            case.task,
            case.inputs.len(),
            max_steps,
            peak_cont,
            best / 1000.0
        );
    }
    println!("\n> correctness: {}/{} vectors halted with expected output\n", ok, total);
}

fn main() {
    // sanity: confirm one solution runs before measuring
    let f = corpus_root().join("factorial").join("mtl").join("solution.mtl");
    let prog = load_solution(&f).unwrap();
    match run(Vm::with_stack(vec![Value::Int(6)], prog), 100_000) {
        Outcome::Halt(s) => assert_eq!(s, vec![Value::Int(720)], "factorial(6) sanity"),
        other => panic!("factorial sanity failed: {:?}", other),
    }

    println!("# MTL runtime perf — raw measurements\n");
    println!("(cloud container; numbers are RELATIVE references, not absolutes)\n");

    println!("## (a) Dispatch — flat straight-line program (`1 1 + _` x units)\n");
    growth("flat: whole program sits in cont; every step does cont.remove(0)", |u| (vec![], straightline(u as usize)), &[16, 64, 256, 1024, 4096]);

    println!("## (a) Dispatch — steady-state loop (`0 n [1 +] .`, cont stays ~constant)\n");
    growth("steady loop: peak dispatch throughput", times_count, &[1000, 10000, 100000, 1000000]);

    println!("## (b) `: !` self-application recursion (countdown to depth n)\n");
    growth("selfapp countdown", selfapp_countdown, &[10, 100, 1000, 10000]);

    println!("## (c) PrimRec — `[0] [+] &` sum to n\n");
    growth("primrec sum_to", primrec_sumto, &[10, 100, 1000, 10000]);

    println!("## (c) LinRec — `[: 0 =] [_] [1 -] [] |` countdown\n");
    growth("linrec countdown", linrec_countdown, &[10, 100, 1000, 10000]);

    println!("## (c) Fold — `0 [+] (` over n ints\n");
    growth("fold sum ints", |n| fold_sum(n as usize), &[10, 100, 1000, 10000]);

    println!("## (d) Fold — `[] [_] (` over n quotation elements\n");
    growth("fold over quote-list", |n| fold_quotes(n as usize), &[10, 100, 1000, 10000]);

    println!("## (e) Parser\n");
    parser_report();

    println!("## (f) Corpus end-to-end (FUEL = 100_000)\n");
    corpus_report("T_v0 (v0.1 gate set)", tv0_cases());
    corpus_report("Tier-2 v0.3 (fold/xor set)", tier2_v03_cases());
}
