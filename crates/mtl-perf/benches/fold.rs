use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use mtl_core::interp::Vm;
use mtl_perf::{drive, fold_quotes, fold_sum, BIG_FUEL};

fn bench_fold_sum(c: &mut Criterion) {
    let mut g = c.benchmark_group("fold/sum_ints");
    g.sample_size(10);
    for &n in &[10usize, 100, 1000, 10000] {
        let (stack, prog) = fold_sum(n);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &(stack, prog), |b, (stack, prog)| {
            b.iter(|| black_box(drive(Vm::with_stack(stack.clone(), prog.clone()), BIG_FUEL).steps));
        });
    }
    g.finish();
}

fn bench_fold_quotes(c: &mut Criterion) {
    let mut g = c.benchmark_group("fold/quote_list");
    g.sample_size(10);
    for &n in &[10usize, 100, 1000, 10000] {
        let (stack, prog) = fold_quotes(n);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &(stack, prog), |b, (stack, prog)| {
            b.iter(|| black_box(drive(Vm::with_stack(stack.clone(), prog.clone()), BIG_FUEL).steps));
        });
    }
    g.finish();
}

criterion_group!(benches, bench_fold_sum, bench_fold_quotes);
criterion_main!(benches);
