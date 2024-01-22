use std::time::Instant;

use rxfetch::{components::name::SystemName, pci::PciDevIterBackend};

fn main() {
    env_logger::init();

    // let name = SystemName::get();
    // dbg!(&name);

    use rxfetch::components::gpu::PrettyDevice;
    let start = Instant::now();
    let backend = rxfetch::pci::PciAutoIter::try_init().unwrap();
    let devices = backend
        .filter_map(|res| {
            res.map_err(|err| log::warn!("PCI Error emitted by backend: {err:?}"))
                .ok()
        })
    .filter_map(|mut dev|dev.is_gpu().is_ok_and(|gpu|gpu).then_some(dev)).take(2)
        .filter_map(|mut dev| {
            pci_ids::Device::from_vid_pid(
                dev.vendor()
                    .map_err(|err| log::warn!("Failed to fetch PCI vendor for {dev:?}: {err:?}"))
                    .ok()?,
                dev.device()
                    .map_err(|err| log::warn!("Failed to fetch PCI device for {dev:?}: {err:?}"))
                    .ok()?,
            ).or_else(||
            {
                log::trace!(
                    "Device not found in PCI ID: {dev:?}, vendor: {vendor:?}, device: {device:?}, class: {class:?}",
                    vendor = dev.vendor(),
                    device = dev.device(),
                    class = dev.class(),
                );
                None
            }
            )
        });
    devices.for_each(|dev| println!("{}", PrettyDevice(dev)));
    let total = start.elapsed();
    println!("Total runtime(including printing): {total:?}");
}
