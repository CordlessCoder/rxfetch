use criterion::{criterion_group, criterion_main, Criterion};
use rxfetch::components::gpu::PCIDevIter;

fn pci_iter(c: &mut Criterion) {
    c.bench_function("Init PCI iter", |b| {
        b.iter_with_large_drop(|| PCIDevIter::new());
    });
    c.bench_function("Iterate over all PCI devices", |b| {
        b.iter_batched_ref(
            || PCIDevIter::new(),
            |g| g.count(),
            criterion::BatchSize::SmallInput,
        )
    });
    c.bench_function("Drop PCI iter", |b| {
        b.iter_batched(
            || PCIDevIter::new(),
            |g| std::mem::drop(g),
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(pci, pci_iter);
criterion_main!(pci);
