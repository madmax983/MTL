use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use mtl_core::interp::{Value, Vm, Word};
use mtl_perf::{drive, linrec_countdown, primrec_sumto, selfapp_countdown, BIG_FUEL};

fn depth_group(c: &mut Criterion, name: &str, gen: fn(i64) -> (Vec<Value>, Vec<Word>), sizes: &[i64]) {
    let mut g = c.benchmark_group(name);
    g.sample_size(10);
    for &n in sizes {
        let (stack, prog) = gen(n);
        let steps = drive(Vm::with_stack(stack.clone(), prog.clone()), BIG_FUEL).steps;
        g.throughput(Throughput::Elements(steps));
        g.bench_with_input(BenchmarkId::from_parameter(n), &(stack, prog), |b, (stack, prog)| {
            b.iter(|| black_box(drive(Vm::with_stack(stack.clone(), prog.clone()), BIG_FUEL).steps));
        });
    }
    g.finish();
}

fn bench_selfapp(c: &mut Criterion) {
    depth_group(c, "recursion/selfapp", selfapp_countdown, &[10, 100, 1000, 10000]);
}
fn bench_primrec(c: &mut Criterion) {
    depth_group(c, "recursion/primrec_sumto", primrec_sumto, &[10, 100, 1000, 10000]);
}
fn bench_linrec(c: &mut Criterion) {
    depth_group(c, "recursion/linrec_countdown", linrec_countdown, &[10, 100, 1000, 10000]);
}

criterion_group!(benches, bench_selfapp, bench_primrec, bench_linrec);
criterion_main!(benches);
