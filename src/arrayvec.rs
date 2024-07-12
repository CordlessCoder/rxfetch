use core::mem::MaybeUninit;
use std::{
    fmt::{Debug, Display},
    io::{Read, Write},
    mem,
    ops::{Deref, DerefMut},
};

/// A *strictly* array-allocated, fixed-capacity, dynamic length data structure. Really handy to
/// avoid heap allocations.
pub struct ArrayVec<T, const CAP: usize> {
    arr: [MaybeUninit<T>; CAP],
    // SAFETY: Values at ..len are valid
    // len <= arr.len at all times
    len: usize,
}

impl<T, const CAP: usize> Default for ArrayVec<T, CAP> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const CAP: usize> Drop for ArrayVec<T, CAP> {
    fn drop(&mut self) {
        self.occupied_mut()
            .iter_mut()
            // SAFETY: Every element in occupied is guaranteed to be a valid, initialized value
            .for_each(|init| unsafe { init.assume_init_drop() })
    }
}

impl<T, const CAP: usize> ArrayVec<T, CAP> {
    pub const fn new() -> Self {
        Self {
            // SAFETY: [MaybeUninit<_>; N] does not need to be initialized to anything, as we only
            // assume that elements at ..len are valid, and len is zero so no elements are assumed
            // to be valid
            arr: unsafe { std::mem::MaybeUninit::uninit().assume_init() },
            len: 0,
        }
    }
    pub fn clear(&mut self) {
        // Create a new ArrayVec, letting the old one drop its values.
        *self = Self::new();
    }
    pub fn try_push(&mut self, value: T) -> Result<(), T> {
        match self.vacant() {
            [] => Err(value),
            [first, _rest @ ..] => {
                first.write(value);
                self.len += 1;
                Ok(())
            }
        }
    }
    pub fn push(&mut self, value: T) {
        _ = self.try_push(value)
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        // SAFETY: All values at ..len are valid
        let ret = unsafe { self.as_raw().get_unchecked(self.len - 1).assume_init_read() };
        self.len -= 1;
        Some(ret)
    }
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    #[inline]
    pub const fn is_full(&self) -> bool {
        self.spare_capacity() == 0
    }
    #[inline]
    pub const fn spare_capacity(&self) -> usize {
        CAP - self.len
    }
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: Every element returned by this is guaranteed to be a valid value of type T
        unsafe { mem::transmute(self.occupied()) }
    }
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: Every element returned by this is guaranteed to be a valid value of type T
        unsafe { mem::transmute(self.occupied_mut()) }
    }
    #[inline]
    pub fn as_raw(&self) -> &[MaybeUninit<T>; CAP] {
        &self.arr
    }
    pub fn as_mut_raw(&mut self) -> &mut [MaybeUninit<T>; CAP] {
        &mut self.arr
    }
    pub fn into_raw(mut self) -> [MaybeUninit<T>; CAP] {
        // We will move the values out of this ArrayVec
        self.len = 0;
        std::mem::replace(&mut self.arr, unsafe {
            // SAFETY: [MaybeUninit<_>; N] does not need to be initialized to anything, as we only
            // assume that elements at ..len are valid, and len is zero so no elements are assumed
            // to be valid
            std::mem::MaybeUninit::uninit().assume_init()
        })
    }
    /// SAFETY: These elements are possibly uninitialized and invalid, reading them is likely a bug
    pub fn vacant(&mut self) -> &mut [MaybeUninit<T>] {
        unsafe { self.arr.get_unchecked_mut(self.len..) }
    }
    /// SAFETY: Every element returned by this is guaranteed to be a valid value of type T
    pub fn occupied_mut(&mut self) -> &mut [MaybeUninit<T>] {
        unsafe { self.arr.get_unchecked_mut(..self.len) }
    }
    /// SAFETY: Every element returned by this is guaranteed to be a valid value of type T
    pub fn occupied(&self) -> &[MaybeUninit<T>] {
        unsafe { self.arr.get_unchecked(..self.len) }
    }
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let mut iter = iter.into_iter();
        while self.spare_capacity() > 0 {
            let Some(x) = iter.next() else { break };
            self.push(x);
        }
    }
}
impl<T: Copy, const CAP: usize> ArrayVec<T, CAP> {
    pub fn copy_from_slice(&mut self, slice: &[T]) {
        let vacant = self.vacant();
        // SAFETY: This many elements are free to be copied from the slice into the vacant space
        let len = vacant.len().min(slice.len());
        unsafe {
            vacant
                .as_mut_ptr()
                // SAFETY: Nonoverlapping copying is safe as we have a mutable reference to the
                // array, which cannot alias with the immutable reference to the slice
                .copy_from_nonoverlapping(slice.as_ptr().cast(), mem::size_of::<T>() * len);
            self.len += len;
        }
    }
}

impl<T, const CAP: usize> Deref for ArrayVec<T, CAP> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, const CAP: usize> DerefMut for ArrayVec<T, CAP> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T: Clone, const CAP: usize> Clone for ArrayVec<T, CAP> {
    fn clone(&self) -> Self {
        let mut out = Self::new();
        out.extend(self.iter().cloned());
        out
    }
}

impl<T: Debug, const CAP: usize> Debug for ArrayVec<T, CAP> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <[T] as Debug>::fmt(self.as_slice(), f)
    }
}

impl<T, const CAP: usize> IntoIterator for ArrayVec<T, CAP> {
    type Item = T;
    type IntoIter = ArrayVecIter<T, CAP>;

    fn into_iter(self) -> Self::IntoIter {
        let len = self.len();
        let arr = self.into_raw();
        ArrayVecIter { arr, idx: 0, len }
    }
}

/// An Iterator over an ArrayVec that owns the elements of the ArrayVec
pub struct ArrayVecIter<T, const CAP: usize> {
    arr: [MaybeUninit<T>; CAP],
    // The values at ..idx have already been consumed
    idx: usize,
    // The values at idx..len are yet to be consumed
    len: usize,
}

impl<T, const CAP: usize> Drop for ArrayVecIter<T, CAP> {
    fn drop(&mut self) {
        // Consume all remaining items, dropping them
        self.for_each(|_| ());
    }
}

impl<T, const CAP: usize> Iterator for ArrayVecIter<T, CAP> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.len {
            return None;
        }
        unsafe {
            // SAFETY: We will only read this value once, therefore we know we are free to take
            // ownership of it. As we advance the index after accessing it, it will not be accessed
            // again.
            let out = self.arr.get_unchecked(self.idx).assume_init_read();
            self.idx += 1;
            Some(out)
        }
    }
}

impl<T, const CAP: usize> FromIterator<T> for ArrayVec<T, CAP> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        let mut arr = Self::new();
        arr.extend(iter);
        arr
    }
}

impl<const CAP: usize> Write for ArrayVec<u8, CAP> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = self.len();
        self.copy_from_slice(buf);
        Ok(self.len() - len)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[repr(transparent)]
struct Loud<T: Debug>(T);
impl<T: Debug> Drop for Loud<T> {
    fn drop(&mut self) {
        println!("{:?} dropped!", self.0)
    }
}
impl<T: Debug> Debug for Loud<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl<T: Debug + Display> Display for Loud<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <T as Display>::fmt(&self.0, f)
    }
}

fn main() {
    let mut arr: ArrayVec<Loud<u8>, 1024> = ArrayVec::new();
    arr.extend([1, 2, 3, 4, 5, 6, 7, 8].map(Loud));
    arr.into_iter().take(4).for_each(|x| println!("{x}"));
}
