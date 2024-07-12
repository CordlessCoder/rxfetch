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

use crate::{
    parse::{unhex, FixedLengthHex},
    ArrayVec,
};

use super::{
    NoProvider, PciBackendError, PciDevIterBackend, PciDevice, PciInfoProvider, Source, WrapPath,
};

// bus/device.function
#[derive(Debug)]
pub struct ProcBusBackend {
    bus_iter: std::fs::ReadDir,
    bus: Option<(std::fs::ReadDir, u8)>,
}

#[derive(Debug)]
pub struct ProcBusProvider {
    buf: ArrayVec<u8, 72>,
}

impl ProcBusProvider {
    pub fn from_devfile<P: AsRef<Path>>(file: P) -> Result<Self, PciBackendError> {
        let path = file.as_ref();
        let mut file = std::fs::File::open(path).map_err(PciBackendError::IOError)?;
        let mut buf = ArrayVec::new();
        std::io::copy(&mut file, &mut buf).map_err(PciBackendError::IOError)?;
        if buf.len() < 16 {
            return Err(PciBackendError::InvalidDevice);
        }
        Ok(ProcBusProvider { buf })
    }
}

fn parse_dev_file<
    'i,
    E: winnow::error::ParserError<Source<'i>> + winnow::error::AddContext<Source<'i>, &'static str>,
>(
    dev_filename: Source<'i>,
) -> Result<PciDevice<NoProvider>, winnow::error::ParseError<Source<'i>, E>>
where {
    winnow::seq!(
    PciDevice {
        device: FixedLengthHex(2),
        _: '.',
        function: FixedLengthHex(1),
        ..PciDevice::new(0,0,0,0)
    }
    )
    .parse(dev_filename)
}

fn get_header_type(dev: &PciDevice<ProcBusProvider>) -> u8 {
    dev.provider.buf[14]
}

impl PciInfoProvider for ProcBusProvider {
    fn get_class(dev: &mut PciDevice<Self>) -> Result<ArrayVec<u8, 32>, PciBackendError> {
        let class = dev.provider.buf[11];
        let subclass = dev.provider.buf[10];
        Ok(ArrayVec::from_iter([class, subclass]))
    }
    fn get_vendor(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError> {
        Ok(u16::from_le_bytes(
            dev.provider.buf[0..2].try_into().unwrap(),
        ))
    }
    fn get_device(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError> {
        Ok(u16::from_le_bytes(
            dev.provider.buf[2..4].try_into().unwrap(),
        ))
    }
    fn get_susbystem_vid(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError> {
        let svid = match get_header_type(dev) {
            0x0 => dev
                .provider
                .buf
                .get(47..49)
                .ok_or(PciBackendError::InvalidDevice)?
                .try_into()
                .unwrap(),
            0x2 => dev
                .provider
                .buf
                .get(66..68)
                .ok_or(PciBackendError::InvalidDevice)?
                .try_into()
                .unwrap(),
            _ => return Err(PciBackendError::NotAvailable),
        };
        Ok(u16::from_le_bytes(svid))
    }
    fn get_susbystem_did(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError> {
        let sid = match get_header_type(dev) {
            0x0 => dev
                .provider
                .buf
                .get(49..51)
                .ok_or(PciBackendError::InvalidDevice)?
                .try_into()
                .unwrap(),
            0x2 => dev
                .provider
                .buf
                .get(64..66)
                .ok_or(PciBackendError::InvalidDevice)?
                .try_into()
                .unwrap(),
            _ => return Err(PciBackendError::NotAvailable),
        };
        Ok(u16::from_le_bytes(sid))
    }
    fn get_revision(dev: &mut PciDevice<Self>) -> Result<u8, PciBackendError> {
        Ok(dev.provider.buf[8])
    }
}

impl PciDevIterBackend for ProcBusBackend {
    type BackendInfoProvider = ProcBusProvider;

    fn try_init() -> Result<Self, PciBackendError> {
        let bus_iter = fs::read_dir("/proc/bus/pci/").map_err(|_| PciBackendError::NotAvailable)?;
        Ok(Self {
            bus_iter,
            bus: None,
        })
    }
}
impl Iterator for ProcBusBackend {
    type Item = Result<PciDevice<ProcBusProvider>, PciBackendError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Bus currently being iterated over
        loop {
            let Self { bus_iter, bus } = self;
            let Some((devices, bus)) = bus else {
                // Find the next bus to iterate over
                let bus_dir = match bus_iter.next()? {
                    Ok(bus) => bus,
                    Err(err) => return Some(Err(PciBackendError::IOError(err))),
                };
                let bus_path = bus_dir.path();
                // SAFETY: We recieved this path from a ReadDir iterator, it must have a file name
                let bus_dir = bus_path.file_name().unwrap().as_encoded_bytes();

                // Ignore devices file
                if bus_dir == b"devices" {
                    continue;
                };
                let dev_iter = match std::fs::read_dir(&bus_path) {
                    Ok(d) => d,
                    Err(err) => return Some(Err(PciBackendError::IOError(err))),
                };
                // Parse bus
                let Ok(b): Result<u8, winnow::error::ParseError<&[u8], ()>> =
                    FixedLengthHex(2).parse(bus_dir)
                else {
                    return Some(Err(PciBackendError::InvalidDevice));
                };
                // Store bus
                *bus = Some((dev_iter, b));
                continue;
            };
            let dev = match devices.next() {
                Some(Ok(dev)) => dev,
                Some(Err(err)) => return Some(Err(PciBackendError::IOError(err))),
                None => {
                    // We have exhausted the devices in this bus, remove the bus iterator
                    self.bus = None;
                    continue;
                }
            };
            let dev_path = dev.path();
            let filename = dev_path.file_name().unwrap().as_encoded_bytes();
            // Attempt to parse the device filename
            let Ok(mut dev) = parse_dev_file::<()>(filename) else {
                return Some(Err(PciBackendError::InvalidDevice));
            };
            // Attach the device to the current bus
            dev.bus = *bus;
            let data = match ProcBusProvider::from_devfile(&dev_path) {
                Ok(p) => p,
                Err(err) => return Some(Err(err)),
            };
            return Some(Ok(dev.with_provider(data)));
        }
        todo!()
    }
}
