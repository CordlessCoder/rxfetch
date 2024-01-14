use criterion::{criterion_group, criterion_main, Criterion};
use rxfetch::components::gpu::GPUIter;

fn gpu_iter(c: &mut Criterion) {
    c.bench_function("Init GPU iter", |b| {
        b.iter_with_large_drop(|| GPUIter::new());
    });
    c.bench_function("Find GPU", |b| {
        b.iter_batched_ref(
            || GPUIter::new(),
            |g| g.count(),
            criterion::BatchSize::SmallInput,
        )
    });
    c.bench_function("Drop GPU iter", |b| {
        b.iter_batched(
            || GPUIter::new(),
            |g| std::mem::drop(g),
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(gpu, gpu_iter);
criterion_main!(gpu);
