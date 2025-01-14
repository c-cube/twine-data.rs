//! Deserialization.
//!
//! Reading data from `twine` is done by offset.

use crate::shallow_value::{ArrayCursor, MapCursor};

pub use super::shallow_value::ShallowValue;
use super::types::*;

/// A decoder for a twine blob.
#[derive(Clone)]
pub struct Decoder<'a> {
    bs: &'a [u8],
}

impl<'a> std::fmt::Debug for Decoder<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Decoder {{bs: {} bytes}}", self.bs.len())
    }
}

impl<'a> Decoder<'a> {
    /// Create a new decoder reading from these bytes.
    pub fn new(bs: &'a [u8]) -> Result<Self> {
        if bs.len() > u32::MAX as usize {
            return Err(Error {
                msg: "byte buffer is too long",
                off: 0,
            });
        }
        Ok(Self { bs })
    }

    /// Read (high, low) nibbles at the given offset.
    #[inline]
    pub(crate) fn first_byte(&self, off: Offset) -> (u8, u8) {
        let c = self.bs[off as usize];
        let high = c >> 4;
        let low = c & 0xf;
        (high, low)
    }

    /// read an integer in LEB128
    fn leb128(&self, mut off: Offset) -> Result<(u64, u8)> {
        let mut res: u64 = 0;
        let mut shift = 0;
        let mut n_bytes = 0;

        loop {
            n_bytes += 1;
            let c = self.bs[off as usize];
            off += 1;
            let cur = c & 0x7f;
            res = res | ((cur as u64) << shift);

            if cur == c {
                // last byte
                return Ok((res, n_bytes));
            } else {
                shift += 7;
                if shift >= 64 {
                    return Err(Error {
                        msg: "out of bound for LEB128",
                        off,
                    });
                }
            }
        }
    }

    pub(crate) fn u64_with_low(&self, off: Offset, low: u8) -> Result<(u64, Offset)> {
        if low < 15 {
            return Ok((low as u64, 0));
        }
        let (rest, consumed) = self.leb128(off + 1)?;
        Ok((rest + 15, consumed as Offset))
    }

    /// Dereference the offset.
    ///
    /// If the value at this offset is a pointer, follow the pointer;
    /// repeat until it's not. This is done implicitly by most
    /// other functions in this module, but it can be useful to do it
    /// by hand in case there is caching done on decoding (eg. to memoize the
    /// value decoded at a particular offset, it's better to dereference
    /// the offset first).
    pub fn deref(&self, mut off: Offset) -> Result<Offset> {
        loop {
            let (high, low) = self.first_byte(off);
            if high == 15 {
                let (p, _) = self.u64_with_low(off, low)?;
                // checked sub
                off = off.checked_sub(p as Offset + 1).ok_or_else(|| Error {
                    msg: "pointer underflow",
                    off,
                })?;
            } else {
                return Ok(off);
            }
        }
    }

    fn i64_pos(&'_ self, off: Offset, low: u8) -> Result<i64> {
        let (x, _) = self.u64_with_low(off, low)?;
        if x > i64::MAX as u64 {
            return Err(Error {
                msg: "i64 overflow",
                off,
            });
        }
        Ok(x as i64)
    }

    fn i64_neg(&'_ self, off: Offset, low: u8) -> Result<i64> {
        let (x, _) = self.u64_with_low(off, low)?;
        if x > i64::MAX as u64 {
            return Err(Error {
                msg: "i64 overflow",
                off,
            });
        }
        Ok(-(x as i64) - 1)
    }

    fn str(&'_ self, mut off: Offset, low: u8) -> Result<&'a str> {
        let (len, n_bytes) = self.u64_with_low(off, low)?;
        off = off + 1 + n_bytes;
        std::str::from_utf8(&self.bs[off as usize..(off as usize + len as usize)]).map_err(|_| {
            Error {
                msg: "overflow in string",
                off,
            }
        })
    }

    fn bytes(&'_ self, mut off: Offset, low: u8) -> Result<&'a [u8]> {
        let (len, n_bytes) = self.u64_with_low(off, low)?;
        off = off + 1 + n_bytes;
        Ok(&self.bs[off as usize..(off as usize + len as usize)])
    }

    fn float(&'_ self, off: Offset, low: u8) -> Result<f64> {
        let off1 = (off + 1) as usize;
        if low == 0 {
            let arr: [u8; 4] = self.bs[off1..off1 + 4].try_into().unwrap();
            let u: u32 = u32::from_le_bytes(arr);
            let f = f32::from_bits(u);
            Ok(f as f64)
        } else if low == 1 {
            let arr: [u8; 8] = self.bs[off1..off1 + 8].try_into().unwrap();
            let u: u64 = u64::from_le_bytes(arr);
            let f = f64::from_bits(u);
            Ok(f)
        } else {
            Err(Error {
                msg: "expected float",
                off,
            })
        }
    }

    fn tag(&'_ self, mut off: Offset, low: u8) -> Result<(Tag, Offset)> {
        let (tag, n_bytes) = self.u64_with_low(off, low)?;
        if tag > u32::MAX as u64 {
            return Err(Error {
                msg: "tag overflow",
                off,
            });
        }
        off = off + 1 + n_bytes;
        Ok((tag as Tag, off))
    }

    fn array_cursor(&'_ self, mut off: Offset, low: u8) -> Result<ArrayCursor<'a>> {
        let (len, n_bytes) = self.u64_with_low(off, low)?;
        if len > u32::MAX as u64 {
            return Err(Error {
                msg: "Size overflow for array",
                off,
            });
        }
        off = off + 1 + n_bytes;
        let dec = self.clone();
        Ok(ArrayCursor {
            dec,
            off,
            n_items: len as u32,
        })
    }

    fn map_cursor(&'_ self, mut off: Offset, low: u8) -> Result<MapCursor<'a>> {
        let (len, n_bytes) = self.u64_with_low(off, low)?;
        if len > u32::MAX as u64 {
            return Err(Error {
                msg: "Size overflow for dict",
                off,
            });
        }
        off = off + 1 + n_bytes;
        let dec = self.clone();
        Ok(MapCursor {
            dec,
            off,
            n_items: len as u32,
        })
    }

    fn cstor(&'_ self, mut off: Offset, high: u8, low: u8) -> Result<(CstorIdx, ArrayCursor<'a>)> {
        macro_rules! mk_cstor {
            ($idx: expr) => {{
                if $idx > u32::MAX as u64 {
                    return Err(Error {
                        msg: "cstor overflow",
                        off,
                    });
                }
                CstorIdx($idx as u32)
            }};
        }

        let dec = self.clone();
        if high == 10 {
            let (idx, _) = self.u64_with_low(off, low)?;
            Ok((
                mk_cstor!(idx),
                ArrayCursor {
                    dec,
                    off,
                    n_items: 0,
                },
            ))
        } else if high == 11 {
            let (idx, n_bytes_idx) = self.u64_with_low(off, low)?;
            let arr = ArrayCursor {
                dec,
                off: off + 1 + n_bytes_idx,
                n_items: 1,
            };
            Ok((mk_cstor!(idx), arr))
        } else if high == 12 {
            let (idx, n_bytes_idx) = self.u64_with_low(off, low)?;
            off = off + 1 + n_bytes_idx;
            let (n_items, n_bytes_n_items) = self.leb128(off)?;
            if n_items > u32::MAX as u64 {
                return Err(Error {
                    msg: "overflow in cstor arguments",
                    off,
                });
            }
            let n_items = n_items as u32;

            off = off + n_bytes_n_items as Offset;
            let arr = ArrayCursor { off, n_items, dec };
            Ok((mk_cstor!(idx), arr))
        } else {
            Err(Error {
                msg: "expected cstor",
                off,
            })
        }
    }

    /// Skip an immediate value, return offset of next value.
    pub(crate) fn skip(&self, off: Offset) -> Result<Offset> {
        let (high, low) = self.first_byte(off);
        let off: Offset = match high {
            0 => off + 1,
            1 | 2 => {
                let (_, n_bytes) = self.u64_with_low(off, low)?;
                off + 1 + n_bytes
            }
            3 => {
                if low == 0 {
                    off + 5
                } else {
                    off + 9
                }
            }
            4 | 5 => {
                let (len, n_bytes) = self.u64_with_low(off, low)?;
                if len > u32::MAX as u64 {
                    return Err(Error {
                        msg: "length overflow",
                        off,
                    });
                }
                off + 1 + n_bytes + len as Offset
            }
            6 | 7 | 8 => {
                return Err(Error {
                    msg: "cannot skip over array/dict/tag",
                    off,
                })
            }

            9 | 13 => {
                return Err(Error {
                    msg: "tag is reserved",
                    off,
                })
            }
            10 => {
                let (_, n_bytes) = self.u64_with_low(off, low)?;
                off + 1 + n_bytes
            }
            11 | 12 => {
                return Err(Error {
                    msg: "cannot skip over constructor",
                    off,
                })
            }
            14 | 15 => {
                let (_, n_bytes) = self.u64_with_low(off, low)?;
                off + 1 + n_bytes
            }
            _ => {
                unreachable!()
            }
        };
        Ok(off)
    }

    /// Read one value at the given offset.
    ///
    /// This does not recurse into subvalues, so it is fairly fast. The main way of
    /// deserializing from twine is through this function.
    /// If the value at `off` is a pointer, it is implicitly followed.
    /// As a consequence, this never returns a `Immediate::Pointer` value.
    pub fn get_shallow_value(&'_ self, mut off: Offset) -> Result<ShallowValue<'a>> {
        use ShallowValue::*;

        off = self.deref(off)?;
        let (high, low) = self.first_byte(off);
        let v: ShallowValue = match high {
            0 => {
                if low == 2 {
                    Imm(Immediate::Null)
                } else if low == 0 {
                    Imm(Immediate::Bool(false))
                } else if low == 1 {
                    Imm(Immediate::Bool(true))
                } else {
                    return Err(Error {
                        msg: "invalid value with high=0",
                        off,
                    });
                }
            }
            1 => Imm(Immediate::Int64(self.i64_pos(off, low)?)),
            2 => Imm(Immediate::Int64(self.i64_neg(off, low)?)),
            3 => Imm(Immediate::Float(self.float(off, low)?)),
            4 => Imm(Immediate::String(self.str(off, low)?)),
            5 => Imm(Immediate::Bytes(self.bytes(off, low)?)),
            6 => {
                let arr = self.array_cursor(off, low)?;
                Array(arr)
            }
            7 => {
                let map = self.map_cursor(off, low)?;
                Map(map)
            }
            8 => {
                let (tag, off) = self.tag(off, low)?;
                Tag(tag, off)
            }
            10 | 11 | 12 => {
                let (cstor_idx, args) = self.cstor(off, high, low)?;
                Cstor(cstor_idx, args)
            }
            14 => {
                let (p, _) = self.u64_with_low(off, low)?;
                // checked sub
                let p = off.checked_sub(p as Offset + 1).ok_or_else(|| Error {
                    msg: "ref underflow",
                    off,
                })?;
                Imm(Immediate::Ref(p))
            }
            15 => unreachable!(), // we did deref!
            _ => {
                return Err(Error {
                    msg: "invalid value",
                    off,
                })
            }
        };
        Ok(v)
    }

    /// Get an integer.
    pub fn get_i64(&self, off: Offset) -> Result<i64> {
        match self.get_shallow_value(off)? {
            ShallowValue::Imm(Immediate::Int64(i)) => Ok(i),
            _ => Err(Error {
                msg: "expected integer",
                off,
            }),
        }
    }

    pub fn get_bool(&self, off: Offset) -> Result<bool> {
        match self.get_shallow_value(off)? {
            ShallowValue::Imm(Immediate::Bool(b)) => Ok(b),
            _ => Err(Error {
                msg: "expected bool",
                off,
            }),
        }
    }

    pub fn get_null(&self, off: Offset) -> Result<()> {
        match self.get_shallow_value(off)? {
            ShallowValue::Imm(Immediate::Null) => Ok(()),
            _ => Err(Error {
                msg: "expected null",
                off,
            }),
        }
    }

    pub fn get_float(&self, off: Offset) -> Result<f64> {
        match self.get_shallow_value(off)? {
            ShallowValue::Imm(Immediate::Float(f)) => Ok(f),
            _ => Err(Error {
                msg: "expected float",
                off,
            }),
        }
    }

    pub fn get_str(&self, off: Offset) -> Result<&'a str> {
        match self.get_shallow_value(off)? {
            ShallowValue::Imm(Immediate::String(s)) => Ok(s),
            _ => Err(Error {
                msg: "expected string",
                off,
            }),
        }
    }

    pub fn get_bytes(&self, off: Offset) -> Result<&'a [u8]> {
        match self.get_shallow_value(off)? {
            ShallowValue::Imm(Immediate::Bytes(s)) => Ok(s),
            _ => Err(Error {
                msg: "expected bytes",
                off,
            }),
        }
    }

    /// Read an array of offsets into `res`
    pub fn get_array(&self, off: Offset, res: &mut Vec<Offset>) -> Result<()> {
        res.clear();
        match self.get_shallow_value(off)? {
            ShallowValue::Array(arr) => {
                for off in arr {
                    res.push(off?)
                }
                Ok(())
            }
            _ => Err(Error {
                msg: "expected array",
                off,
            }),
        }
    }

    /// Read a dictionary of offsets into `res`.
    ///
    /// `res` is cleared before reading.
    pub fn get_dict(&self, off: Offset, res: &mut Vec<(Offset, Offset)>) -> Result<()> {
        res.clear();
        match self.get_shallow_value(off)? {
            ShallowValue::Map(d) => {
                for pair in d {
                    let (k, v) = pair?;
                    res.push((k, v))
                }
                Ok(())
            }
            _ => Err(Error {
                msg: "expected dict",
                off,
            }),
        }
    }

    /// Read a tagged value.
    ///
    /// The value itself is not read, only an offset to it is returned.
    pub fn get_tag(&self, off: Offset) -> Result<(Tag, Offset)> {
        match self.get_shallow_value(off)? {
            ShallowValue::Tag(tag, off) => Ok((tag, off)),
            _ => Err(Error {
                msg: "expected tag",
                off,
            }),
        }
    }

    /// Read a constructor value. The constructor index is returned,
    /// and (the offsets of the) arguments are pushed into `args`.
    ///
    /// `args` is cleared first.
    pub fn get_cstor(&self, off: Offset, args: &mut Vec<Offset>) -> Result<CstorIdx> {
        args.clear();

        match self.get_shallow_value(off)? {
            ShallowValue::Cstor(cstor_idx, c_args) => {
                for off in c_args {
                    args.push(off?)
                }
                Ok(cstor_idx)
            }
            _ => Err(Error {
                msg: "expected cstor",
                off,
            }),
        }
    }

    /// Find the entrypoint.
    ///
    /// A twine blob is terminated with a postfix (in essence, a pointer to the actual
    /// toplevel value). This reads the postfix and returns the offset of the toplevel value.
    pub fn entrypoint(&self) -> Result<Offset> {
        let last = self.bs.len() as Offset - 1;
        let off = last - self.bs[last as usize] as Offset - 1;
        self.deref(off)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_dec_leb128() {
        {
            let dec = Decoder::new(&[17]).unwrap();
            assert_eq!(Some((17, 1)), dec.leb128(0).ok());
        }

        {
            let dec = Decoder::new(&[0x88, 0x85, 0x09]).unwrap();
            assert_eq!(Some(((9 << (7 + 7)) + (5 << 7) + 8, 3)), dec.leb128(0).ok());
        }
    }

    proptest! {
        #[test]
        fn decode_from_leb128_crate(n: u64){
            let mut ref_v = vec![];
            let ref_len = leb128::write::unsigned( &mut ref_v,n).unwrap();

            let dec = Decoder::new(&ref_v).unwrap();
            let (n2, len) = dec.leb128(0).unwrap();
            assert_eq!(n2, n);
            assert_eq!(ref_len, len as usize);
        }
    }
}
