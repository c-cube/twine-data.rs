//! Encoder.
//!
//! The encoder is used to write a complex DAG, in the twine format, into
//! a byte stream.

use std::io;

use crate::{
    types::{CstorIdx, Offset, Tag},
    Immediate,
};

/// Encode `n` as LEB128 into `buf`, returns how many bytes were used.
///
/// Requires `buf` to have at least 10 bytes of capacity.
#[allow(dead_code)]
pub(crate) fn enc_leb128(mut n: u64, buf: &mut [u8]) -> usize {
    assert!(buf.len() >= 10);

    let mut count = 0;
    loop {
        let c = (n & 0b0111_1111) as u8;

        if c as u64 == n {
            buf[count] = c;
            count += 1;
            return count;
        } else {
            buf[count] = c | 0b1000_0000;
            count += 1;
            n = n >> 7;
        }
    }
}

pub struct Encoder<W: io::Write> {
    w: W,
    offset: Offset,
}

pub type Result<T> = std::result::Result<T, io::Error>;

impl<W: io::Write> Encoder<W> {
    /// Create an encoder.
    pub fn new(w: W) -> Self {
        Encoder { w, offset: 0 }
    }

    /// Write the tag and small integer.
    #[inline(always)]
    fn first_byte(&mut self, high: u8, low: u8) -> Result<Offset> {
        self.w.write(&[(high << 4) | low])?;
        let off = self.offset;
        self.offset += 1;
        Ok(off)
    }

    /// Write the tag `high` and an integer `n`.
    fn first_byte_and_u64(&mut self, high: u8, n: u64) -> Result<Offset> {
        if n < 15 {
            return self.first_byte(high, n as u8);
        }

        let mut buf = [0u8; 11];
        buf[0] = (high << 4) | 15;
        let len = enc_leb128(n - 15, &mut buf[1..]);
        self.w.write(&buf[0..len + 1])?;

        let off = self.offset;
        self.offset += len as u64 + 1;
        Ok(off)
    }

    #[inline(always)]
    pub fn write_null(&mut self) -> Result<Offset> {
        return self.first_byte(0, 2);
    }

    #[inline(always)]
    pub fn write_bool(&mut self, b: bool) -> Result<Offset> {
        return self.first_byte(0, b as u8);
    }

    /// Write an integer.
    pub fn write_i64(&mut self, n: i64) -> Result<Offset> {
        if n < 0 {
            let n = ((-n) - 1) as u64;
            self.first_byte_and_u64(2, n)
        } else {
            self.first_byte_and_u64(1, n as u64)
        }
    }

    pub fn write_ref(&mut self, p: Offset) -> Result<Offset> {
        let off = self.offset;
        debug_assert!(off > p); // can only point to previous values.

        // compute relative offset to `p`.
        let n = off - p - 1;
        self.first_byte_and_u64(14, n)
    }

    pub fn write_pointer(&mut self, p: Offset) -> Result<Offset> {
        let off = self.offset;
        debug_assert!(off > p); // can only point to previous values.

        // compute relative offset to `p`.
        let n = off - p - 1;
        self.first_byte_and_u64(15, n)
    }

    pub fn write_f32(&mut self, f: f32) -> Result<Offset> {
        let bytes = f32::to_le_bytes(f);
        let off = self.first_byte(3, 0)?;
        self.w.write(&bytes)?;
        self.offset += bytes.len() as u64;
        Ok(off)
    }

    pub fn write_f64(&mut self, f: f64) -> Result<Offset> {
        let bytes = f64::to_le_bytes(f);
        let off = self.first_byte(3, 1)?;
        self.w.write(&bytes)?;
        self.offset += bytes.len() as u64;
        Ok(off)
    }

    /// Write a unicode string.
    pub fn write_string(&mut self, s: &str) -> Result<Offset> {
        let len = s.len() as u64;
        let off = self.first_byte_and_u64(4, len)?;
        self.w.write(s.as_bytes())?;
        self.offset += len;
        Ok(off)
    }

    /// Write a binary blob.
    pub fn write_bytes(&mut self, b: &[u8]) -> Result<Offset> {
        let len = b.len() as u64;
        let off = self.first_byte_and_u64(5, len)?;
        self.w.write(b)?;
        self.offset += len;
        Ok(off)
    }

    /// Write a nullary constructor.
    #[inline(always)]
    pub fn write_cstor0(&mut self, c: CstorIdx) -> Result<Offset> {
        self.first_byte_and_u64(10, c.0 as u64)
    }

    /// Write an immediate value.
    #[must_use]
    pub fn write_immediate(&mut self, imm: Immediate) -> Result<Offset> {
        match imm {
            Immediate::Null => self.write_null(),
            Immediate::Bool(b) => self.write_bool(b),
            Immediate::Int64(i) => self.write_i64(i),
            Immediate::Float(f) => self.write_f64(f),
            Immediate::String(s) => self.write_string(s),
            Immediate::Bytes(b) => self.write_bytes(b),
            Immediate::Cstor0(c) => self.write_cstor0(c),
            Immediate::Ref(p) => self.write_ref(p),
            Immediate::Pointer(p) => self.write_pointer(p),
        }
    }

    /// Write the immediate; but if it's a pointer, return the pointer
    /// without writing a thing.
    #[inline(always)]
    #[must_use]
    pub fn write_immediate_or_return_pointer(&mut self, imm: Immediate) -> Result<Offset> {
        match imm {
            Immediate::Pointer(p) => Ok(p),
            _ => self.write_immediate(imm),
        }
    }

    pub fn write_tag(&mut self, tag: Tag, v: Immediate) -> Result<Offset> {
        let off = self.first_byte_and_u64(8, tag as u64)?;
        let _ = self.write_immediate(v)?;
        Ok(off)
    }

    /// Write an array.
    ///
    /// The values in the array must be converted to immediates already,
    /// possibly by way of writing them first and making pointers to
    /// their written representation.
    pub fn write_array(&mut self, arr: &[Immediate]) -> Result<Offset> {
        let off = self.first_byte_and_u64(6, arr.len() as u64)?;
        for v in arr {
            let _ = self.write_immediate(*v)?;
        }
        Ok(off)
    }

    /// Write a map. Keys and values must already be encoded into immediates.
    pub fn write_map(&mut self, map: &[(Immediate, Immediate)]) -> Result<Offset> {
        let off = self.first_byte_and_u64(7, map.len() as u64)?;
        for (k, v) in map {
            let _ = self.write_immediate(*k)?;
            let _ = self.write_immediate(*v)?;
        }
        Ok(off)
    }

    /// Write a constructor `c` with arguments `args`.
    pub fn write_cstor(&mut self, c: CstorIdx, args: &[Immediate]) -> Result<Immediate<'static>> {
        match args.len() {
            0 => Ok(Immediate::Cstor0(c)),
            1 => {
                let off = self.first_byte_and_u64(11, c.0 as u64)?;
                let _ = self.write_immediate(args[0])?;
                Ok(Immediate::Pointer(off))
            }
            _ => {
                let off = self.first_byte_and_u64(12, c.0 as u64)?;

                // now write number of arguments as LEB128
                let mut buf_len = [0u8; 10];
                let len_of_len = enc_leb128(args.len() as u64, &mut buf_len[..]);
                self.w.write(&buf_len[0..len_of_len])?;
                self.offset += len_of_len as u64;

                for a in args {
                    let _ = self.write_immediate(*a)?;
                }
                Ok(Immediate::Pointer(off))
            }
        }
    }

    /// Write the postfix to point to `entrypoint`, and consume the encoder.
    pub fn finalize(mut self, entrypoint: Immediate) -> Result<()> {
        // first, write the entrypoint.
        let entrypoint = self.write_immediate_or_return_pointer(entrypoint)?;

        let mut top = self.offset;
        debug_assert!(top > entrypoint);
        let mut delta = top - entrypoint - 1;
        // if delta is too big (ie if entrypoint is a large value, too large to fit
        // in a `u8` pointer), we go through an intermediate `Immediate::Pointer` step.
        if delta > 250 {
            let ptr_to_entry = self.write_pointer(entrypoint)?;

            // recompute delta. Writing the pointer should take at most 11 bytes,
            // so this time the delta will fit in a single byte.
            top = self.offset;
            delta = top - ptr_to_entry - 1;
        }

        debug_assert!(delta <= 250);
        self.w.write(&[delta as u8])?;
        self.offset += 1;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_enc_leb128() {
        let mut buf = [0u8; 16];
        {
            let n = enc_leb128(42, &mut buf);
            assert_eq!(1, n);
            assert_eq!(Some(42), leb128::read::unsigned(&mut &buf[..]).ok());
        }
        {
            let n = enc_leb128(329282522, &mut buf);
            assert_eq!(5, n);
            assert_eq!(Some(329282522), leb128::read::unsigned(&mut &buf[..]).ok());
        }
        {
            let n = enc_leb128(u64::MAX, &mut buf);
            assert_eq!(10, n);
            dbg!(&buf);
            assert_eq!(Some(u64::MAX), leb128::read::unsigned(&mut &buf[..]).ok());
        }
    }

    proptest! {
        #[test]
        fn same_as_leb128_crate(n: u64){
            let mut ours = [0u8; 12];
            let ours_len = enc_leb128(n, &mut ours[..]);

            let mut ref_v = vec![];
            let _ref_len = leb128::write::unsigned( &mut ref_v,n).unwrap();
            assert_eq!(&ref_v, &ours[0..ours_len])
        }
    }

    #[test]
    fn test_ref() {
        use crate::value::Value as V;
        use crate::Decoder;

        let mut res: Vec<u8> = vec![];
        let mut enc = Encoder::new(&mut res);

        let off1 = enc.write_string("hello").unwrap();
        let off2 = enc.write_string("world").unwrap();
        let v = V::Array(vec![V::Ref(off1), V::Ref(off2)]);
        let off_v = crate::value::write_value(&mut enc, &v).unwrap();
        assert_eq!(
            &[69u8, 104, 101, 108, 108, 111, 69, 119, 111, 114, 108, 100, 98, 236, 231][..]
                as &[u8],
            res.as_slice()
        );

        let dec = Decoder::new(&res).unwrap();
        let v2 = crate::value::read_value(&dec, off_v).unwrap();
        assert_eq!(v, v2);
    }
}
