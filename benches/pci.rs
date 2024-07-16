use criterion::{criterion_group, criterion_main, Criterion};
use rxfetch::pci::{PciAutoIter, PciDevIterBackend};

fn pci_iter(c: &mut Criterion) {
    c.bench_function("Init and drop PCI iter", |b| {
        b.iter_batched(
            || (),
            |_| PciAutoIter::init(),
            criterion::BatchSize::PerIteration,
        );
    });
    c.bench_function("Iterate over all PCI devices", |b| {
        b.iter_batched_ref(
            PciAutoIter::init,
            |g| g.count(),
            criterion::BatchSize::PerIteration,
        )
    });
    c.bench_function("Drop PCI iter", |b| {
        b.iter_batched(
            PciAutoIter::init,
            std::mem::drop,
            criterion::BatchSize::PerIteration,
        )
    });
}

criterion_group!(pci, pci_iter);
criterion_main!(pci);
