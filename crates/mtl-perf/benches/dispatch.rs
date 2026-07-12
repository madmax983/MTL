use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use mtl_core::interp::Vm;
use mtl_perf::{drive, straightline, times_count, BIG_FUEL};

fn bench_flat(c: &mut Criterion) {
    let mut g = c.benchmark_group("dispatch/flat");
    g.sample_size(20);
    for &units in &[4usize, 16, 64, 256, 1024] {
        let prog = straightline(units);
        let steps = (units * 4) as u64;
        g.throughput(Throughput::Elements(steps));
        g.bench_with_input(BenchmarkId::from_parameter(steps), &prog, |b, prog| {
            b.iter(|| black_box(drive(Vm::new(prog.clone()), BIG_FUEL).steps));
        });
    }
    g.finish();
}

fn bench_steady(c: &mut Criterion) {
    let mut g = c.benchmark_group("dispatch/loop_steady");
    for &n in &[1_000i64, 10_000, 100_000] {
        let (stack, prog) = times_count(n);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &(stack, prog), |b, (stack, prog)| {
            b.iter(|| black_box(drive(Vm::with_stack(stack.clone(), prog.clone()), BIG_FUEL).steps));
        });
    }
    g.finish();
}

criterion_group!(benches, bench_flat, bench_steady);
criterion_main!(benches);
