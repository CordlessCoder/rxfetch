use log::warn;
use std::{
    fmt::{Debug, LowerHex},
    fs,
    io::{ErrorKind, Read},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};
use winnow::{
    ascii, combinator as combo,
    prelude::*,
    stream::{self, Stream, StreamIsPartial},
    token, BStr,
};

use crate::ArrayVec;

#[derive(Clone)]
pub struct PciDevice<BackendProvider> {
    domain: u16,
    bus: u8,
    device: u8,
    function: u8,
    provider: BackendProvider,
}

pub type Source<'i> = &'i [u8];

struct FixedLengthHex(pub usize);
impl<
        I: Stream<Slice = I> + StreamIsPartial + stream::AsBStr,
        O: ascii::HexUint,
        E: winnow::error::AddContext<I> + winnow::error::ParserError<I>,
    > Parser<I, O, E> for FixedLengthHex
where
    <I as Stream>::Token: stream::AsChar,
{
    fn parse_next(&mut self, input: &mut I) -> PResult<O, E> {
        token::take(self.0)
            .complete_err()
            .context("Not enough bytes in hex field")
            .and_then(ascii::hex_uint::<I, O, E>.context("Invalid hex digits in hex field"))
            .parse_next(input)
    }
}

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
    fn parse_short<
        'i,
        E: winnow::error::ParserError<Source<'i>>
            + winnow::error::AddContext<Source<'i>, &'static str>,
    >(
        stream: Source<'i>,
    ) -> Result<PciDevice<NoProvider>, winnow::error::ParseError<Source<'i>, E>>
where {
        let hex = |len| FixedLengthHex(len);
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
    pub fn is_gpu(&mut self) -> Result<bool, PciBackendError> {
        Ok(self.class()?.get(0).is_some_and(|&class| class == 3))
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
        Ok(())
    }
}

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
}

pub trait PciDevIterBackend: Sized {
    type BackendInfoProvider: PciInfoProvider;

    /// Attempt to initialize the backend
    fn try_init() -> Result<Self, PciBackendError>;
    fn next(&mut self) -> Option<Result<PciDevice<Self::BackendInfoProvider>, PciBackendError>>;
}

#[derive(Debug)]
pub struct SysBusBackend {
    dir_iter: std::fs::ReadDir,
}

pub struct SysBusProvider {
    path: PathBuf,
}

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
        &mut self.path
    }
}

impl Drop for WrapPath<'_> {
    fn drop(&mut self) {
        for _ in 0..self.count {
            self.path.pop();
        }
    }
}

fn unhex(b: u8) -> u8 {
    const LUT: [u8; 256] = {
        let mut arr = [0; 256];
        let mut i = 0;
        while i < 256 {
            let b = i as u8;
            arr[b as usize] = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b + 10 - b'a',
                _ => 0,
            };
            i += 1;
        }
        arr
    };
    LUT[b as usize]
}

impl PciInfoProvider for SysBusProvider {
    fn get_class(dev: &mut PciDevice<Self>) -> Result<ArrayVec<u8, 32>, PciBackendError> {
        // Temporarily add "class" to the end of the path, removing it once WrapPath goes out of
        // scope
        let class = WrapPath::new(&mut dev.provider.path, "class");

        let class = class.as_path();
        let mut file = fs::File::open(class).map_err(|err| PciBackendError::IOError(err))?;
        let mut buf: ArrayVec<u8, 64> = ArrayVec::new();
        std::io::copy(&mut file, &mut buf).map_err(|err| PciBackendError::IOError(err))?;
        Ok(buf
            // Skip leading 0x
            .chunks_exact(2)
            .skip(1)
            .map(|bytes| (unhex(bytes[0]) << 4) + unhex(bytes[1]))
            .collect())
    }
    fn get_vendor(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError> {
        let vendor = WrapPath::new(&mut dev.provider.path, "vendor");
        let vendor = vendor.as_path();

        let mut file = fs::File::open(vendor).map_err(|err| PciBackendError::IOError(err))?;

        let mut buf: ArrayVec<u8, 32> = ArrayVec::new();
        std::io::copy(&mut file, &mut buf).map_err(|err| PciBackendError::IOError(err))?;

        let bytes = buf.get(2..6).ok_or(PciBackendError::InvalidDevice)?;
        Ok(bytes
            .chunks_exact(2)
            .map(|bytes| (unhex(bytes[0]) << 4) + unhex(bytes[1]))
            .fold(0, |acc, hex| (acc << 8) | hex as u16))
    }

    fn get_device(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError> {
        let device = WrapPath::new(&mut dev.provider.path, "device");
        let device = device.as_path();

        let mut file = fs::File::open(device).map_err(|err| PciBackendError::IOError(err))?;

        let mut buf = [0; 32];
        // std::io::copy(&mut file, &mut buf)
        // .map_err(|err| PciBackendError::IOError(err))?;
        file.read(&mut buf)
            .map_err(|err| PciBackendError::IOError(err))?;

        let bytes = buf.get(2..6).ok_or(PciBackendError::InvalidDevice)?;
        Ok(bytes
            .chunks_exact(2)
            .map(|bytes| (unhex(bytes[0]) << 4) | unhex(bytes[1]))
            .fold(0, |acc, hex| (acc << 8) | hex as u16))
    }
    fn get_susbystem_vid(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError> {
        let path = WrapPath::new(&mut dev.provider.path, "subsystem_vendor");
        let path = path.as_path();

        let mut file = fs::File::open(path).map_err(|err| PciBackendError::IOError(err))?;

        let mut buf: ArrayVec<u8, 32> = ArrayVec::new();
        std::io::copy(&mut file, &mut buf).map_err(|err| PciBackendError::IOError(err))?;

        let bytes = buf.get(2..6).ok_or(PciBackendError::InvalidDevice)?;
        Ok(bytes
            .chunks_exact(2)
            // Skip leading 0x
            .skip(1)
            .map(|bytes| (unhex(bytes[0]) << 4) + unhex(bytes[1]))
            .fold(0, |acc, hex| (acc << 8) | hex as u16))
    }
    fn get_susbystem_did(dev: &mut PciDevice<Self>) -> Result<u16, PciBackendError> {
        let path = WrapPath::new(&mut dev.provider.path, "subsystem_device");
        let path = path.as_path();

        let mut file = fs::File::open(path).map_err(|err| PciBackendError::IOError(err))?;

        let mut buf: ArrayVec<u8, 32> = ArrayVec::new();
        std::io::copy(&mut file, &mut buf).map_err(|err| PciBackendError::IOError(err))?;

        let bytes = buf.get(2..6).ok_or(PciBackendError::InvalidDevice)?;
        Ok(bytes
            .chunks_exact(2)
            // Skip leading 0x
            .skip(1)
            .map(|bytes| (unhex(bytes[0]) << 4) + unhex(bytes[1]))
            .fold(0, |acc, hex| (acc << 8) | hex as u16))
    }
}

impl PciDevIterBackend for SysBusBackend {
    type BackendInfoProvider = SysBusProvider;

    fn try_init() -> Result<Self, PciBackendError> {
        let dir_iter =
            fs::read_dir("/sys/bus/pci/devices").map_err(|_| PciBackendError::NotAvailable)?;
        Ok(Self { dir_iter })
    }
    fn next(&mut self) -> Option<Result<PciDevice<Self::BackendInfoProvider>, PciBackendError>> {
        while let Some(dir) = self.dir_iter.next() {
            let dir = match dir {
                Ok(dir) => dir,
                Err(err) => return Some(Err(PciBackendError::IOError(err))),
            };
            let name = dir.file_name();
            let name = name.as_encoded_bytes();

            let dev = match PciDevice::parse_short::<()>(name.into()) {
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
// TODO: Add backends for /proc/bus/pci and /proc/pci, as well as a MacOS and Windows backend

pub struct PciDevIter {}
