use crate::display::DisplayBytes;
use std::{fmt::Debug, io, ptr};

#[derive(Clone)]
pub struct SystemName {
    uname: libc::utsname,
}

impl SystemName {
    pub fn get() -> Self {
        let arr = || std::array::from_fn(|_| 0);
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
    fn up_to_null(slice: &[i8]) -> &[u8] {
        // SAFETY: &[i8] and &[u8] have identical in-memory represebtation, valid bit patterns etc.
        let slice: &[u8] = unsafe { std::mem::transmute(slice) };
        let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
        &slice[..len]
    }
    pub fn system(&self) -> &[u8] {
        Self::up_to_null(&self.uname.sysname)
    }
    pub fn node(&self) -> &[u8] {
        Self::up_to_null(&self.uname.nodename)
    }
    pub fn release(&self) -> &[u8] {
        Self::up_to_null(&self.uname.release)
    }
    pub fn version(&self) -> &[u8] {
        Self::up_to_null(&self.uname.version)
    }
    pub fn machine(&self) -> &[u8] {
        Self::up_to_null(&self.uname.machine)
    }
    pub fn domain(&self) -> &[u8] {
        Self::up_to_null(&self.uname.domainname)
    }
}

impl Debug for SystemName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("SystemName");
        let mut add = |name, data| _ = s.field(name, &DisplayBytes::new(data));
        add("sysname", self.system());
        add("nodename", self.node());
        add("release", self.release());
        add("version", self.version());
        add("machine", self.machine());
        add("domainname", self.domain());
        s.finish()
    }
}
