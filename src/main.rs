#![allow(unused)]
use std::time::Instant;

use rxfetch::pci::PciDevIterBackend;

fn main() {
    env_logger::init();

    use rxfetch::components::{gpu::PrettyDevice, *};
    // let start = Instant::now();
    let mut backend = rxfetch::pci::SysBusBackend::try_init().unwrap();
    while let Some(dev) = backend.next() {
        let Ok(mut dev) = dev else {
            continue;
        };
        if dev.is_gpu().unwrap() {
            let dev = pci_ids::Device::from_vid_pid(dev.vendor().unwrap(), dev.device().unwrap())
                .unwrap();
            println!("{}", PrettyDevice(dev))
        };
    }
    // let took = start.elapsed();
    // println!("New backend took {took:?}");
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
