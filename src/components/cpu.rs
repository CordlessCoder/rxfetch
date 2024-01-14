use std::ops::Range;

type ID = u16;
/// Oh so very Linux specific CPU representation
pub struct Cpu {
    cores: u16,
    /// (freq, None)
    /// (lower_bound, Some(higher_bound))
    frequencies: (u32, Option<u32>),
    /// The range of cpu device IDs associated with this specific CPU
    range: Range<ID>,
}

impl Cpu {
    pub fn iter() -> CpuIter {
        CpuIter::new()
    }
}

pub struct CpuIter {
    id: ID,
}

impl CpuIter {
    pub fn new() -> Self {
        Self { id: 0 }
    }
}
