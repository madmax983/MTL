use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mtl_bench_validate::load_solution;
use mtl_core::interp::{run, Vm};
use mtl_perf::corpus::{corpus_root, tier2_v03_cases, tv0_cases, CorpusCase};

const FUEL: u64 = 100_000;

fn bench_set(c: &mut Criterion, group: &str, cases: Vec<CorpusCase>) {
    let mut g = c.benchmark_group(group);
    for case in &cases {
        let path = corpus_root().join(case.task).join(case.version).join("solution.mtl");
        let prog = load_solution(&path).unwrap();
        g.bench_function(case.task, |b| {
            b.iter(|| {
                for input in &case.inputs {
                    black_box(run(Vm::with_stack(input.clone(), prog.clone()), FUEL));
                }
            });
        });
    }
    g.finish();
}

fn bench_tv0(c: &mut Criterion) {
    bench_set(c, "corpus/T_v0", tv0_cases());
}
fn bench_tier2_v03(c: &mut Criterion) {
    bench_set(c, "corpus/tier2_v03", tier2_v03_cases());
}

criterion_group!(benches, bench_tv0, bench_tier2_v03);
criterion_main!(benches);
