use std::{
    fmt::{Debug, LowerHex},
    fs,
    io::{ErrorKind, Read},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};
use tracing::warn;
use winnow::{
    ascii, combinator as combo,
    prelude::*,
    stream::{self, Stream, StreamIsPartial},
    token, BStr,
};
#[cfg(unix)]
mod linux_sysfs;
#[cfg(unix)]
pub use linux_sysfs::*;
#[cfg(unix)]
mod linux_procfs;
#[cfg(unix)]
pub use linux_procfs::*;
mod id_parser;

use crate::{parse::FixedLengthHex, ArrayVec};

#[derive(Clone)]
pub struct PciDevice<BackendProvider> {
    domain: u16,
    bus: u8,
    device: u8,
    function: u8,
    provider: BackendProvider,
}

pub type Source<'i> = &'i [u8];

struct NoProvider;

impl PciDevice<NoProvider> {
    fn new(domain: u16, bus: u8, device: u8, function: u8) -> Self {
        PciDevice {
            domain,
            bus,
            device,
            function,
            provider: NoProvider,
        }
    }
    fn with_provider<P: PciInfoProvider>(self, provider: P) -> PciDevice<P> {
        let Self {
            domain,
            bus,
            device,
            function,
            ..
        } = self;
        PciDevice {
            domain,
            bus,
            device,
            function,
            provider,
        }
    }
}

pub enum AutoProvider {
    #[cfg(unix)]
    SysFS(SysBusProvider),
    #[cfg(unix)]
    ProcFS(ProcBusProvider),
    None,
}

/// Get an owned reference
fn to_owned_dev(pcidev: &mut PciDevice<AutoProvider>) -> PciDevice<AutoProvider> {
    let mut dev = PciDevice::new(0, 0, 0, 0).with_provider(AutoProvider::None);
    std::mem::swap(pcidev, &mut dev);
    dev
}

macro_rules! delegate {
    ($(($name:ident -> $return:ty)),*$(,)?) => {
        $(
    fn $name(pcidev: &mut PciDevice<AutoProvider>) -> $return {
        // Grab the device as an owned value
        let dev = to_owned_dev(pcidev);
        let PciDevice {
            domain,
            bus,
            device,
            function,
            provider,
        } = dev;
        let (ret, dev) = match provider {
            #[cfg(unix)]
            AutoProvider::SysFS(provider) => {
                let mut dev = PciDevice {
                    domain,
                    bus,
                    device,
                    function,
                    provider,
                };
                let ret = SysBusProvider::$name(&mut dev);
                let PciDevice {
                    domain,
                    bus,
                    device,
                    function,
                    provider,
                } = dev;

                let dev = PciDevice {
                    domain,
                    bus,
                    device,
                    function,
                    provider: AutoProvider::SysFS(provider),
                };
                (ret, dev)
            }
            #[cfg(unix)]
            AutoProvider::ProcFS(provider) => {
                let mut dev = PciDevice {
                    domain,
                    bus,
                    device,
                    function,
                    provider,
                };
                let ret = ProcBusProvider::$name(&mut dev);
                let PciDevice {
                    domain,
                    bus,
                    device,
                    function,
                    provider,
                } = dev;

                let dev = PciDevice {
                    domain,
                    bus,
                    device,
                    function,
                    provider: AutoProvider::ProcFS(provider),
                };
                (ret, dev)
            }
            other => (
                Err(PciBackendError::NotAvailable),
                PciDevice {
                    domain,
                    bus,
                    device,
                    function,
                    provider: other,
                },
            ),
        };
        // Restore the device
        *pcidev = dev;
        ret
    }
        )*
    };
}

impl PciInfoProvider for AutoProvider {
    delegate![
        (get_class -> Result<ArrayVec<u8, 32>, PciBackendError>),
        (get_vendor -> Result<u16, PciBackendError>),
        (get_device -> Result<u16, PciBackendError>),
        (get_revision -> Result<u8, PciBackendError>),
        (get_susbystem_vid -> Result<u16, PciBackendError>),
        (get_susbystem_did -> Result<u16, PciBackendError>),
    ];
}

impl<P: PciInfoProvider> PciDevice<P> {
    pub fn class(&mut self) -> Result<ArrayVec<u8, 32>, PciBackendError> {
        P::get_class(self)
    }
    pub fn vendor(&mut self) -> Result<u16, PciBackendError> {
        P::get_vendor(self)
    }
    pub fn device(&mut self) -> Result<u16, PciBackendError> {
        P::get_device(self)
    }
    pub fn susbystem_vid(&mut self) -> Result<u16, PciBackendError> {
        P::get_susbystem_vid(self)
    }
    pub fn susbystem_did(&mut self) -> Result<u16, PciBackendError> {
        P::get_susbystem_did(self)
    }
    pub fn revision(&mut self) -> Result<u8, PciBackendError> {
        P::get_revision(self)
    }
    pub fn is_gpu(&mut self) -> Result<bool, PciBackendError> {
        Ok(self.class()?.first().is_some_and(|&class| class == 3))
    }
}

struct HexDebug<T: LowerHex>(T);
impl<T: LowerHex> Debug for HexDebug<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.0, f)
    }
}

impl<B> Debug for PciDevice<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dev = f.debug_struct("PciDevice");
        dev.field("domain", &HexDebug(self.domain));
        dev.field("bus", &HexDebug(self.bus));
        dev.field("device", &HexDebug(self.device));
        dev.field("function", &HexDebug(self.function));
        dev.finish_non_exhaustive()?;
        Ok(())
    }
}

// TODO: Add support for PCI resources, to eventually get available vram
// decode flags according to https://elixir.bootlin.com/linux/latest/source/include/linux/ioport.h
// struct PciDeviceResource {
//     addr: usize,
//     len: usize,
//     prefetch: bool,
//     prefetch: bool,
// }

#[derive(Debug)]
pub enum PciBackendError {
    NotAvailable,
    IOError(std::io::Error),
    InvalidDevice,
}

pub trait PciInfoProvider: Sized {
    fn get_class(device: &mut PciDevice<Self>) -> Result<ArrayVec<u8, 32>, PciBackendError>;
    fn get_vendor(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError>;
    fn get_device(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError>;
    fn get_susbystem_vid(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError>;
    fn get_susbystem_did(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError>;
    fn get_revision(dev: &mut PciDevice<Self>) -> Result<u8, PciBackendError>;
    // fn get_resources(dev: &mut PciDevice<Self>) -> Result<ArrayVec<,32>, PciBackendError>;
}

pub trait PciDevIterBackend:
    Sized + Iterator<Item = Result<PciDevice<Self::BackendInfoProvider>, PciBackendError>>
{
    type BackendInfoProvider: PciInfoProvider;

    /// Attempt to initialize the backend
    fn try_init() -> Result<Self, PciBackendError>;

    /// Attempt to initialize the backend and panic on failure
    fn init() -> Self {
        Self::try_init().expect("Failed to initialize PciBackend")
    }
}

#[derive(Debug)]
struct WrapPath<'b> {
    path: &'b mut PathBuf,
    count: usize,
}
impl<'b> WrapPath<'b> {
    pub fn new<P: AsRef<Path>>(path: &'b mut PathBuf, push: P) -> Self {
        let push = push.as_ref();
        let count = push.ancestors().count() - 1;
        path.push(push);
        Self { path, count }
    }
}

impl<'b> Deref for WrapPath<'b> {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &*self.path
    }
}

impl<'b> DerefMut for WrapPath<'b> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.path
    }
}

impl Drop for WrapPath<'_> {
    fn drop(&mut self) {
        for _ in 0..self.count {
            self.path.pop();
        }
    }
}

// TODO: Add backends for MacOS and Windows

/// An iterator over attached PCI devices that picks its device fetching backend automatically
pub enum PciAutoIter {
    #[cfg(unix)]
    SysFS(SysBusBackend),
    #[cfg(unix)]
    ProcFS(ProcBusBackend),
}

impl PciDevIterBackend for PciAutoIter {
    type BackendInfoProvider = AutoProvider;

    fn try_init() -> Result<Self, PciBackendError> {
        let sysfs = |_| SysBusBackend::try_init().map(PciAutoIter::SysFS);
        let proc = |_| ProcBusBackend::try_init().map(PciAutoIter::ProcFS);
        sysfs(()).or_else(proc)
    }
}

impl Iterator for PciAutoIter {
    type Item = Result<PciDevice<AutoProvider>, PciBackendError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            PciAutoIter::SysFS(b) => b.next().map(|r| {
                r.map(|d| {
                    let PciDevice {
                        domain,
                        bus,
                        device,
                        function,
                        provider,
                    } = d;
                    PciDevice {
                        domain,
                        bus,
                        device,
                        function,
                        provider: AutoProvider::SysFS(provider),
                    }
                })
            }),
            PciAutoIter::ProcFS(b) => b.next().map(|r| {
                r.map(|d| {
                    let PciDevice {
                        domain,
                        bus,
                        device,
                        function,
                        provider,
                    } = d;
                    PciDevice {
                        domain,
                        bus,
                        device,
                        function,
                        provider: AutoProvider::ProcFS(provider),
                    }
                })
            }),
        }
    }
}
