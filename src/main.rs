#![allow(unused)]
use std::time::Instant;

use rxfetch::pci::PciDevIterBackend;

fn main() {
    env_logger::init();

    use rxfetch::components::{gpu::PrettyDevice, *};
    let mut backend = rxfetch::pci::SysBusBackend::try_init().unwrap();
    let start = Instant::now();
    let devices: Vec<_> = backend
        .filter_map(|res| {
            res.map_err(|err| log::warn!("PCI Error emitted by backend: {err:?}"))
                .ok()
        })
        .filter_map(|mut dev| {
            pci_ids::Device::from_vid_pid(
                dev.vendor()
                    .map_err(|err| log::warn!("Failed to fetch PCI vendor for {dev:?}"))
                    .ok()?,
                dev.device()
                    .map_err(|err| log::warn!("Failed to fetch PCI device for {dev:?}"))
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
        }).collect();
    let took = start.elapsed();
    devices
        .iter()
        .for_each(|dev| println!("{:?}", PrettyDevice(dev)));
    println!("Fetching all device data took {took:?}");
    // while let Some(dev) = backend.next() {
    //     let Ok(mut dev) = dev else {
    //         continue;
    //     };
    //     // if dev.is_gpu().is_ok_and(|b| b) {
    //     let Some(dev) = pci_ids::Device::from_vid_pid(dev.vendor().unwrap(), dev.device().unwrap())
    //     else {
    //         log::trace!(
    //             "Device not found in PCI ID: {dev:?}, vendor: {vendor:?}, device: {device:?}, class: {class:?}",
    //             vendor = dev.vendor(),
    //             device = dev.device(),
    //             class = dev.class(),
    //         );
    //         continue;
    //     };
    //     println!("{:?}", PrettyDevice(dev))
    //     // };
    // }
}
// use std::{
//     fs::File,
//     io::{stdout, BufWriter, Read, Write},
//     os::fd::RawFd,
//     path::Path,
//     time::Instant,
// };
//
// use rxfetch::components::{gpu::PrettyDevice, *};
// use sysinfo::{CpuRefreshKind, SystemExt};

// fn main() {
// let start = Instant::now();
// let gpus = gpu::GPUIter::new();
// for gpu in gpus {
//     println!("{}", PrettyDevice(gpu));
// }
// let took = start.elapsed();
// println!("Took {took:?}")
// }

// struct FileHandle(RawFd);
// impl FileHandle {
//     fn open<P: AsRef<Path>>(file: P) -> Self {}
// }
//
// fn time<T, F: FnOnce() -> T>(f: F, desc: &str) -> T {
//     let start = Instant::now();
//     let ret = f();
//     let took = start.elapsed();
//     println!("{desc} {took:?}");
//     ret
// }
