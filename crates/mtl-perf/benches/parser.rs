use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use mtl_perf::gen_source;

fn bench_parse(c: &mut Criterion) {
    let mut g = c.benchmark_group("parser/throughput");
    for &units in &[100usize, 1000, 10000] {
        let src = gen_source(units);
        let bytes = src.len() as u64;
        g.throughput(Throughput::Bytes(bytes));
        g.bench_with_input(BenchmarkId::from_parameter(bytes), &src, |b, src| {
            b.iter(|| black_box(mtl_syntax::parse(black_box(src)).unwrap().len()));
        });
    }
    g.finish();
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
