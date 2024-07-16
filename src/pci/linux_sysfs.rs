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

#[derive(Debug)]
pub struct SysBusBackend {
    dir_iter: std::fs::ReadDir,
}

pub struct SysBusProvider {
    path: PathBuf,
}

fn parse_device<
    'i,
    E: winnow::error::ParserError<Source<'i>> + winnow::error::AddContext<Source<'i>, &'static str>,
>(
    stream: Source<'i>,
) -> Result<PciDevice<NoProvider>, winnow::error::ParseError<Source<'i>, E>>
where {
    let hex = FixedLengthHex;
    winnow::seq!(
    PciDevice {
        domain: hex(4),
        _: ':',
        bus: hex(2),
        _: ':',
        device: hex(2),
        _: '.',
        function: hex(1),
        .. PciDevice::new(0, 0, 0, 0)
    }
    )
    .parse(stream)
}

impl PciInfoProvider for SysBusProvider {
    fn get_class(dev: &mut PciDevice<Self>) -> Result<ArrayVec<u8, 32>, PciBackendError> {
        // Temporarily add "class" to the end of the path, removing it once WrapPath goes out of
        // scope
        let class = WrapPath::new(&mut dev.provider.path, "class");

        let mut file = fs::File::open(&*class).map_err(PciBackendError::IOError)?;
        let mut buf: ArrayVec<u8, 64> = ArrayVec::new();
        std::io::copy(&mut file, &mut buf).map_err(PciBackendError::IOError)?;
        Ok(buf
            .chunks_exact(2)
            // Skip leading 0x
            .skip(1)
            .map(|bytes| (unhex(bytes[0]) << 4) + unhex(bytes[1]))
            .collect())
    }
    fn get_vendor(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError> {
        let vendor = WrapPath::new(&mut dev.provider.path, "vendor");
        let vendor = vendor.as_path();

        let mut file = fs::File::open(vendor).map_err(PciBackendError::IOError)?;

        let mut buf: ArrayVec<u8, 32> = ArrayVec::new();
        std::io::copy(&mut file, &mut buf).map_err(PciBackendError::IOError)?;

        // Skip leading 0x
        let bytes = buf.get(2..6).ok_or(PciBackendError::InvalidDevice)?;
        Ok(bytes
            .chunks_exact(2)
            .map(|bytes| (unhex(bytes[0]) << 4) + unhex(bytes[1]))
            .fold(0, |acc, hex| (acc << 8) | hex as u16))
    }

    fn get_device(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError> {
        let device = WrapPath::new(&mut dev.provider.path, "device");
        let device = device.as_path();

        let mut file = fs::File::open(device).map_err(PciBackendError::IOError)?;

        let mut buf = [0; 32];
        file.read(&mut buf).map_err(PciBackendError::IOError)?;

        // Skip leading 0x
        let bytes = buf.get(2..6).ok_or(PciBackendError::InvalidDevice)?;
        Ok(bytes
            .chunks_exact(2)
            .map(|bytes| (unhex(bytes[0]) << 4) | unhex(bytes[1]))
            .fold(0, |acc, hex| (acc << 8) | hex as u16))
    }
    fn get_susbystem_vid(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError> {
        let path = WrapPath::new(&mut dev.provider.path, "subsystem_vendor");
        let path = path.as_path();

        let mut file = fs::File::open(path).map_err(PciBackendError::IOError)?;

        let mut buf: ArrayVec<u8, 32> = ArrayVec::new();
        std::io::copy(&mut file, &mut buf).map_err(PciBackendError::IOError)?;

        // Skip leading 0x
        let bytes = buf.get(2..6).ok_or(PciBackendError::InvalidDevice)?;
        Ok(bytes
            .chunks_exact(2)
            .map(|bytes| (unhex(bytes[0]) << 4) + unhex(bytes[1]))
            .fold(0, |acc, hex| (acc << 8) | hex as u16))
    }
    fn get_susbystem_did(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError> {
        let path = WrapPath::new(&mut dev.provider.path, "subsystem_device");
        let path = path.as_path();

        let mut file = fs::File::open(path).map_err(PciBackendError::IOError)?;

        let mut buf: ArrayVec<u8, 32> = ArrayVec::new();
        std::io::copy(&mut file, &mut buf).map_err(PciBackendError::IOError)?;

        let bytes = buf.get(2..6).ok_or(PciBackendError::InvalidDevice)?;
        Ok(bytes
            .chunks_exact(2)
            // Skip leading 0x
            .skip(1)
            .map(|bytes| (unhex(bytes[0]) << 4) + unhex(bytes[1]))
            .fold(0, |acc, hex| (acc << 8) | hex as u16))
    }

    fn get_revision(dev: &mut PciDevice<Self>) -> Result<u8, PciBackendError> {
        let path = WrapPath::new(&mut dev.provider.path, "revision");
        let path = path.as_path();

        let mut file = fs::File::open(path).map_err(PciBackendError::IOError)?;

        let mut buf: ArrayVec<u8, 32> = ArrayVec::new();
        std::io::copy(&mut file, &mut buf).map_err(PciBackendError::IOError)?;

        let bytes = buf.get(2..4).ok_or(PciBackendError::InvalidDevice)?;
        Ok((unhex(bytes[0]) << 4) | unhex(bytes[1]))
    }
}

impl PciDevIterBackend for SysBusBackend {
    type BackendInfoProvider = SysBusProvider;

    fn try_init() -> Result<Self, PciBackendError> {
        let dir_iter =
            fs::read_dir("/sys/bus/pci/devices").map_err(|_| PciBackendError::NotAvailable)?;
        Ok(Self { dir_iter })
    }
}
impl Iterator for SysBusBackend {
    type Item = Result<PciDevice<SysBusProvider>, PciBackendError>;

    fn next(&mut self) -> Option<Self::Item> {
        for dir in self.dir_iter.by_ref() {
            let dir = match dir {
                Ok(dir) => dir,
                Err(err) => return Some(Err(PciBackendError::IOError(err))),
            };
            let name = dir.file_name();
            let name = name.as_encoded_bytes();

            let dev = match parse_device::<()>(name) {
                Ok(dev) => dev,
                Err(err) => {
                    warn!(
                        "Failed to parse PCI device: `{name}` Error: {err:?}",
                        name = String::from_utf8_lossy(name)
                    );
                    continue;
                }
            };

            return Some(Ok(dev.with_provider(SysBusProvider { path: dir.path() })));
        }
        None
    }
}
