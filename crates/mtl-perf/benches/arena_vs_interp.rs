//! Arena-vs-interp COMPARATIVE criterion benches (production backend).
//!
//! Runs the four PERF-BASELINE stress cases plus a fork microbench on BOTH
//! backends at matched sizes, so the speedup ratio is directly readable from the
//! two `interp/<n>` vs `arena/<n>` rows in each group:
//!
//!   * (a) flat straight-line `1 1 + _`      — `cont.remove(0)` front-pop O(n²)
//!   * (c) PrimRec `sum_to(n)`               — combinator re-emit O(n²)
//!   * (d) Fold `0 [+] (` over n ints        — spine re-clone O(n²)
//!   * (b) `: !` self-application countdown   — the tail-linear NON-pathology
//!   * fork: `interp::Vm::clone()` vs `arena::VmState` copy at stack depth d
//!
//! BEFORE = `mtl_core::interp::run` (via the instrumented `mtl_perf::drive`).
//! AFTER  = `mtl_arena::run_arena` (production arena backend).
//!
//! For clean tabular numbers prefer `cargo run --release --example
//! arena_vs_interp -p mtl-perf`; this criterion harness is the statistical
//! cross-check. Run e.g.:
//!   cargo bench -p mtl-perf --bench arena_vs_interp -- --warm-up-time 1 --measurement-time 3

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use mtl_arena as arena;
use mtl_core::interp::{Prim, Value, Vm, Word};
use mtl_perf::{drive, fold_sum, primrec_sumto, selfapp_countdown, straightline, BIG_FUEL};

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

/// Full program = initial stack (as leading pushes) ++ program, in interp Words.
fn full_prog(init: &[Value], prog: &[Word]) -> Vec<Word> {
    let mut v: Vec<Word> = init.iter().map(value_to_word).collect();
    v.extend(prog.iter().cloned());
    v
}

fn to_arena(full: &[Word]) -> Vec<arena::ProgWord> {
    full.iter().map(conv_word).collect()
}

// ---------------------------------------------------- comparative driver
/// Bench one scenario (given by its interp `init` stack + `prog`) on both
/// backends at each size, tagging rows `interp/<n>` and `arena/<n>` so the ratio
/// is read directly. `gen` returns `(init, prog)` for a size `n`.
fn compare_group(
    c: &mut Criterion,
    name: &str,
    sizes: &[i64],
    gen: impl Fn(i64) -> (Vec<Value>, Vec<Word>),
) {
    let mut g = c.benchmark_group(name);
    g.sample_size(10);
    for &n in sizes {
        let (init, prog) = gen(n);
        let full = full_prog(&init, &prog);
        let arena_prog = to_arena(&full);
        let steps = drive(Vm::with_stack(init.clone(), prog.clone()), FUEL).steps;
        g.throughput(Throughput::Elements(steps));

        g.bench_with_input(BenchmarkId::new("interp", n), &(init, prog), |b, (init, prog)| {
            b.iter(|| black_box(drive(Vm::with_stack(init.clone(), prog.clone()), FUEL).steps));
        });
        g.bench_with_input(BenchmarkId::new("arena", n), &arena_prog, |b, ap| {
            b.iter(|| black_box(arena::run_arena(ap, FUEL).steps));
        });
    }
    g.finish();
}

// ---------------------------------------------------- the four stress cases
fn bench_flat_frontpop(c: &mut Criterion) {
    // (a) flat `1 1 + _` × units; size n = number of STEPS (units = n/4).
    compare_group(c, "arena_vs_interp/flat_frontpop", &[256, 1024, 4096, 16384], |n| {
        (vec![], straightline((n / 4) as usize))
    });
}

fn bench_primrec_sumto(c: &mut Criterion) {
    // (c) `[0] [+] &` sum_to(n) — the 223 ms O(n²) case.
    compare_group(c, "arena_vs_interp/primrec_sumto", &[100, 1000, 10000], primrec_sumto);
}

fn bench_fold_sum(c: &mut Criterion) {
    // (d) `0 [+] (` over n ints — the hidden spine-reclone O(n²).
    compare_group(c, "arena_vs_interp/fold_sum", &[100, 1000, 10000], |n| fold_sum(n as usize));
}

fn bench_selfapp_countdown(c: &mut Criterion) {
    // (b) `: !` countdown — the tail-linear NON-pathology (no regression check).
    compare_group(c, "arena_vs_interp/selfapp_countdown", &[100, 1000, 10000], selfapp_countdown);
}

// ---------------------------------------------------- fork microbenchmark
/// Build a depth-`d` persistent arena stack; the returned `VmState` (12 bytes,
/// `Copy`) is what a fork copies (production analogue of the spike's `build_stack`).
fn arena_stack_state(d: usize) -> (arena::Vm, arena::VmState) {
    let prog: Vec<arena::ProgWord> = (0..d).map(|i| arena::ProgWord::PushInt(i as i64)).collect();
    let r = arena::run_arena(&prog, FUEL);
    (r.vm, r.state)
}

fn bench_fork(c: &mut Criterion) {
    let mut g = c.benchmark_group("arena_vs_interp/fork");
    g.sample_size(10);
    for &d in &[1usize, 100, 1000, 10000] {
        // BEFORE: clone an interp Vm holding a depth-d stack + small cont (O(d)).
        let stack: Vec<Value> = (0..d).map(|i| Value::Int(i as i64)).collect();
        let cont = vec![Word::PushInt(1), Word::Prim(Prim::Add)];
        let vm = Vm::with_stack(stack, cont);
        g.bench_with_input(BenchmarkId::new("interp_clone", d), &vm, |b, vm| {
            b.iter(|| black_box(vm.clone()));
        });
        // AFTER: copy a 12-byte arena VmState sitting on a depth-d stack (O(1)).
        let (_avm, st) = arena_stack_state(d);
        g.bench_with_input(BenchmarkId::new("arena_copy", d), &st, |b, st| {
            b.iter(|| black_box(*st));
        });
    }
    g.finish();
}

// Keep the BIG_FUEL import meaningful across mtl-perf versions.
const _: u64 = BIG_FUEL;

criterion_group!(
    benches,
    bench_flat_frontpop,
    bench_primrec_sumto,
    bench_fold_sum,
    bench_selfapp_countdown,
    bench_fork,
);
criterion_main!(benches);
