use thiserror::Error;

use crate::display::DisplayBytes;
use std::{borrow::Cow, ffi::CStr, fmt::Debug, io, mem::MaybeUninit, ptr};

pub fn current_uid() -> u32 {
    unsafe { libc::getuid() }
}

/// # Safety
/// A buffer that can viewed as a (possibly uninitialized) byte slice with the given capacity
pub unsafe trait BackingBuffer {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize);
}

/// # Safety
/// A buffer that can viewed as a (possibly uninitialized) byte slice with the given capacity
/// and can double its capacity on demand
pub unsafe trait GrowableBackingBuffer: BackingBuffer {
    /// Doubles the capacity of the buffer, and makes sure it's at least 128 bytes
    /// May discard all data currently in the buffer
    fn grow(&mut self);
}

unsafe impl GrowableBackingBuffer for Vec<u8> {
    fn grow(&mut self) {
        self.clear();
        self.reserve(self.capacity().max(64) * 2)
    }
}
unsafe impl BackingBuffer for Vec<u8> {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize) {
        unsafe { (self.as_mut_ptr(), self.capacity()) }
    }
}

unsafe impl BackingBuffer for &'_ mut [MaybeUninit<u8>] {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize) {
        unsafe { (self.as_mut_ptr().cast(), self.len()) }
    }
}

unsafe impl BackingBuffer for &'_ mut [u8] {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize) {
        unsafe { (self.as_mut_ptr(), self.len()) }
    }
}
unsafe impl<const LEN: usize> BackingBuffer for [u8; LEN] {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize) {
        unsafe { (self.as_mut_ptr(), self.len()) }
    }
}

unsafe impl<const LEN: usize> BackingBuffer for [MaybeUninit<u8>; LEN] {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize) {
        unsafe { (self.as_mut_ptr().cast(), self.len()) }
    }
}

unsafe impl GrowableBackingBuffer for Vec<i8> {
    fn grow(&mut self) {
        self.clear();
        self.reserve(self.capacity().max(64) * 2)
    }
}
unsafe impl BackingBuffer for Vec<i8> {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize) {
        unsafe { (self.as_mut_ptr().cast(), self.capacity()) }
    }
}

unsafe impl GrowableBackingBuffer for &'_ mut Vec<u8> {
    fn grow(&mut self) {
        self.clear();
        self.reserve(self.capacity().max(64) * 2)
    }
}
unsafe impl BackingBuffer for &'_ mut Vec<u8> {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize) {
        unsafe { (self.as_mut_ptr(), self.capacity()) }
    }
}

unsafe impl GrowableBackingBuffer for &'_ mut Vec<i8> {
    fn grow(&mut self) {
        self.clear();
        self.reserve(self.capacity().max(64) * 2)
    }
}
unsafe impl BackingBuffer for &'_ mut Vec<i8> {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize) {
        unsafe { (self.as_mut_ptr().cast(), self.capacity()) }
    }
}
unsafe impl BackingBuffer for &'_ mut [MaybeUninit<i8>] {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize) {
        unsafe { (self.as_mut_ptr().cast(), self.len()) }
    }
}

unsafe impl BackingBuffer for &'_ mut [i8] {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize) {
        unsafe { (self.as_mut_ptr().cast(), self.len()) }
    }
}
unsafe impl<const LEN: usize> BackingBuffer for [i8; LEN] {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize) {
        unsafe { (self.as_mut_ptr().cast(), self.len()) }
    }
}

unsafe impl<const LEN: usize> BackingBuffer for [MaybeUninit<i8>; LEN] {
    fn as_ptr_cap(&mut self) -> (*mut u8, usize) {
        unsafe { (self.as_mut_ptr().cast(), self.len()) }
    }
}

pub struct PwuId<Buf> {
    passwd: libc::passwd,
    buf: Buf,
}

impl<Buf> Debug for PwuId<Buf> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PwuId")
            .field("name", &self.name())
            .field("password", &self.password())
            .field("uid", &self.uid())
            .field("gid", &self.gid())
            .field("gecos", &self.gecos())
            .field("dir", &self.dir())
            .field("shell", &self.shell())
            .finish()
    }
}

impl<B> PwuId<B> {
    const NULL_VAL: libc::passwd = libc::passwd {
        pw_name: ptr::null_mut(),
        pw_passwd: ptr::null_mut(),
        pw_uid: 0,
        pw_gid: 0,
        pw_gecos: ptr::null_mut(),
        pw_dir: ptr::null_mut(),
        pw_shell: ptr::null_mut(),
    };
    pub fn name(&self) -> DisplayBytes {
        DisplayBytes::new(unsafe { CStr::from_ptr(self.passwd.pw_name) }.to_bytes())
    }
    pub fn password(&self) -> DisplayBytes {
        DisplayBytes::new(unsafe { CStr::from_ptr(self.passwd.pw_passwd) }.to_bytes())
    }
    pub fn uid(&self) -> u32 {
        self.passwd.pw_uid
    }
    pub fn gid(&self) -> u32 {
        self.passwd.pw_gid
    }
    pub fn gecos(&self) -> DisplayBytes {
        DisplayBytes::new(unsafe { CStr::from_ptr(self.passwd.pw_gecos) }.to_bytes())
    }
    pub fn dir(&self) -> DisplayBytes {
        DisplayBytes::new(unsafe { CStr::from_ptr(self.passwd.pw_dir) }.to_bytes())
    }
    pub fn shell(&self) -> DisplayBytes {
        DisplayBytes::new(unsafe { CStr::from_ptr(self.passwd.pw_shell) }.to_bytes())
    }
}

impl PwuId<Vec<u8>> {
    pub fn get_alloc(uid: u32) -> Result<Self, PwuIdErr> {
        let buf = Vec::with_capacity(1024);
        Self::get(buf, uid).map_err(|(err, _)| err)
    }
}

impl<Buf: GrowableBackingBuffer> PwuId<Buf> {
    /// Attempt to call libc::getpwuid_r, growing the backing buffer if necessary
    #[tracing::instrument(skip(buf), fields(buf_cap = buf.as_ptr_cap().1))]
    pub fn get(mut buf: Buf, uid: u32) -> Result<Self, (PwuIdErr, Buf)> {
        use PwuIdErr::*;
        for attempt in 0..32 {
            let err;
            (err, buf) = match Self::try_get(buf, uid) {
                Ok(passwd) => return Ok(passwd),
                Err((err, recovered_buf)) => (err, recovered_buf),
            };
            let BufferTooSmall = err else {
                return Err((err, buf));
            };
            buf.grow();
        }
        Err((BufferTooSmall, buf))
    }
}
#[derive(Debug, Error)]
pub enum PwuIdErr {
    #[error("The uid {0} was not found")]
    NotFound(u32),
    #[error("A signal was caught during the execution of getpwuid_r")]
    SignalCaught,
    #[error("An IO error occured")]
    IOErr,
    #[error("The maximum number of files was open already in the proccess")]
    InsufficientProcessFds,
    #[error("The maximum number of files was open already in the system")]
    InsufficientSystemFds,
    #[error("The provided buffer was too small")]
    BufferTooSmall,
}

impl<Buf: BackingBuffer> PwuId<Buf> {
    /// Attempt to call libc::getpwuid_r, without growing the backing buffer.
    #[tracing::instrument(skip(buf), fields(buf_cap = buf.as_ptr_cap().1))]
    pub fn try_get(mut buf: Buf, uid: u32) -> Result<Self, (PwuIdErr, Buf)> {
        let mut passwd = Self::NULL_VAL;
        let mut resultp = ptr::null_mut();

        let (ptr, cap) = buf.as_ptr_cap();
        let err = unsafe { libc::getpwuid_r(uid, &mut passwd, ptr.cast(), cap, &mut resultp) };
        use PwuIdErr::*;
        if resultp.is_null() {
            let err = match err {
                libc::EINTR => SignalCaught,
                libc::EIO => IOErr,
                libc::EMFILE => InsufficientProcessFds,
                libc::ENFILE => InsufficientSystemFds,
                libc::ERANGE => BufferTooSmall,
                _ => NotFound(uid),
            };
            return Err((err, buf));
        }

        Ok(Self { passwd, buf })
    }
    pub fn into_buf(self) -> Buf {
        self.buf
    }
}

#[derive(Clone, Copy)]
pub struct SystemName {
    uname: libc::utsname,
}

fn up_to_null(slice: &[i8]) -> &[u8] {
    // SAFETY: &[i8] and &[u8] have identical in-memory representation, valid bit patterns etc.
    let slice: &[u8] = unsafe { std::mem::transmute(slice) };
    let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    &slice[..len]
}
impl SystemName {
    pub fn get() -> Self {
        let arr = || [0; 65];
        let init = || libc::utsname {
            sysname: arr(),
            nodename: arr(),
            release: arr(),
            version: arr(),
            machine: arr(),
            domainname: arr(),
        };
        let mut uname = init();
        let buf = ptr::from_mut(&mut uname);
        if unsafe { libc::uname(buf) } == -1 {
            // uname call failed, set all buffers to null
            uname = init();
        };
        SystemName { uname }
    }
    pub fn system(&self) -> DisplayBytes {
        DisplayBytes::new(up_to_null(&self.uname.sysname))
    }
    pub fn node(&self) -> DisplayBytes {
        DisplayBytes::new(up_to_null(&self.uname.nodename))
    }
    pub fn release(&self) -> DisplayBytes {
        DisplayBytes::new(up_to_null(&self.uname.release))
    }
    pub fn version(&self) -> DisplayBytes {
        DisplayBytes::new(up_to_null(&self.uname.version))
    }
    pub fn machine(&self) -> DisplayBytes {
        DisplayBytes::new(up_to_null(&self.uname.machine))
    }
    pub fn domain(&self) -> DisplayBytes {
        DisplayBytes::new(up_to_null(&self.uname.domainname))
    }
}

impl Debug for SystemName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("SystemName");
        let mut add = |name, data: DisplayBytes| _ = s.field(name, &data);
        add("sysname", self.system());
        add("nodename", self.node());
        add("release", self.release());
        add("version", self.version());
        add("machine", self.machine());
        add("domainname", self.domain());
        s.finish()
    }
}
