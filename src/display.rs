use std::{
    borrow::Cow,
    fmt::{Debug, Display, Write},
    ops::{Deref, DerefMut},
};

pub struct DisplayBytes<'b, const REPLACEMENT: char = '�'>(pub Cow<'b, [u8]>);

impl<'b> DisplayBytes<'b, '�'> {
    pub fn new<C: Into<Cow<'b, [u8]>>>(bytes: C) -> Self {
        DisplayBytes(bytes.into())
    }
}

impl<const R: char> Deref for DisplayBytes<'_, R> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DisplayBytes<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.to_mut()
    }
}

impl<const REPLACEMENT: char> Display for DisplayBytes<'_, REPLACEMENT> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut bytes = &self[..];
        while !bytes.is_empty() {
            match std::str::from_utf8(bytes) {
                Ok(s) => {
                    bytes = &[];
                    f.write_str(s)
                }
                Err(err) => {
                    let len = err.valid_up_to();
                    if len == 0 {
                        f.write_char(REPLACEMENT)?;
                        bytes = &bytes[1..];
                        continue;
                    }
                    // Add one as we want to grab the last valid byte
                    let (valid, rest) = bytes.split_at(len + 1);
                    let valid = unsafe { std::str::from_utf8_unchecked(valid) };
                    bytes = rest;
                    f.write_str(valid)
                }
            }?;
        }
        Ok(())
    }
}

impl<const REPLACEMENT: char> Debug for DisplayBytes<'_, REPLACEMENT> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut bytes = &self[..];
        f.write_char('"')?;
        Display::fmt(&self, f)?;
        f.write_char('"')?;
        Ok(())
    }
}
