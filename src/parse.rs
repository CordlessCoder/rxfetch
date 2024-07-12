use winnow::{
    ascii, combinator as combo,
    prelude::*,
    stream::{self, Stream, StreamIsPartial},
    token, BStr,
};

pub(crate) struct FixedLengthHex(pub usize);
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

pub(crate) fn unhex(b: u8) -> u8 {
    const LUT: [u8; 256] = {
        let mut arr = [0; 256];
        let mut i = 0;
        while i < 256 {
            let b = i as u8;
            arr[b as usize] = match b {
                b'0'..=b'9' => b.wrapping_sub(b'0'),
                b'a'..=b'f' => b.wrapping_sub(b'a').wrapping_add(10),
                _ => 0,
            };
            i += 1;
        }
        arr
    };
    LUT[b as usize]
}
