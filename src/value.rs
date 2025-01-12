//! Full, nested values.
//!
//! Such values are useful for various tools that need a global view of the
//! data, or for manipulating twine data as if it were JSON.
//!
//! This relies on a bump allocator (via bumpalo), via the feature `bumpalo`.

use std::io;

use crate::{shallow_value::ShallowValue, Encoder};

use super::{
    types::{CstorIdx, Offset, Tag},
    Decoder, Immediate, Result,
};
use bumpalo::Bump;

/// A value, potentially containing other values. All the sub-values live in the same allocator.
#[derive(Debug, Clone, Copy)]
pub enum Value<'a, 'tmp> {
    Imm(Immediate<'a>),
    Tag(Tag, &'tmp Value<'a, 'tmp>),
    Array(&'tmp [Value<'a, 'tmp>]),
    Map(&'tmp [(Value<'a, 'tmp>, Value<'a, 'tmp>)]),
    Cstor(CstorIdx, &'tmp [Value<'a, 'tmp>]),
}

impl<'a, 'tmp> Default for Value<'a, 'tmp> {
    fn default() -> Self {
        Value::Imm(Default::default())
    }
}

/// Read a value into `bump`.
pub fn get_value<'a, 'tmp>(
    d: &'a Decoder<'a>,
    alloc: &'tmp Bump,
    off: Offset,
) -> Result<Value<'a, 'tmp>> {
    use Value::*;

    let v: Value = match d.get_shallow_value(off)? {
        ShallowValue::Imm(v) => Imm(v),
        ShallowValue::Tag(tag, off) => {
            let v = alloc.alloc(get_value(d, alloc, off)?);
            Tag(tag, v)
        }
        ShallowValue::Array(arr) => {
            let n_items = arr.len();
            let args: &'tmp mut [Value] = alloc.alloc_slice_fill_copy(n_items, Default::default());
            for (i, off) in arr.into_iter().enumerate() {
                let off = off?;
                args[i] = get_value(d, alloc, off)?;
            }
            Array(args)
        }
        ShallowValue::Map(dict) => {
            let n_items = dict.len();
            let pairs: &'tmp mut [(Value, Value)] =
                alloc.alloc_slice_fill_copy(n_items, Default::default());
            for (i, pair) in dict.into_iter().enumerate() {
                let (k, v) = pair?;
                pairs[i] = (get_value(d, alloc, k)?, get_value(d, alloc, v)?);
            }
            Map(pairs)
        }
        ShallowValue::Cstor(cstor_idx, args) => {
            let local: Vec<Offset> = args.into_iter().collect::<Result<Vec<_>>>()?;
            let args: &'tmp mut [Value] =
                alloc.alloc_slice_fill_copy(local.len(), Default::default());
            for (i, off) in local.into_iter().enumerate() {
                args[i] = get_value(d, alloc, off)?;
            }
            Cstor(cstor_idx, args)
        }
    };
    Ok(v)
}

/// Find the entrypoint and read a value from it.
pub fn get_value_from_entrypoint<'a, 'tmp>(
    d: &'a Decoder<'a>,
    bump: &'tmp Bump,
) -> Result<Value<'a, 'tmp>> {
    let off = d.entrypoint()?;
    get_value(d, bump, off)
}

fn write_value_or_imm<'a, 'tmp, W: io::Write>(
    enc: &mut Encoder<W>,
    v: Value<'a, 'tmp>,
) -> io::Result<Immediate<'a>> {
    match v {
        Value::Imm(imm) => Ok(imm),
        Value::Tag(tag, v) => {
            let v = write_value_or_imm(enc, *v)?;
            Ok(enc.write_tag(tag, v)?.into())
        }
        Value::Array(arr) => {
            // locally gather immediates
            let mut res = Vec::with_capacity(arr.len());
            for x in arr {
                res.push(write_value_or_imm(enc, *x)?);
            }
            Ok(enc.write_array(&res)?.into())
        }
        Value::Map(map) => {
            // locally gather immediates
            let mut res = Vec::with_capacity(map.len());
            for (k, v) in map {
                let k = write_value_or_imm(enc, *k)?;
                let v = write_value_or_imm(enc, *v)?;
                res.push((k, v));
            }
            Ok(enc.write_map(&res)?.into())
        }
        Value::Cstor(cstor_idx, args) => {
            let mut args_res = Vec::with_capacity(args.len());
            for x in args {
                args_res.push(write_value_or_imm(enc, *x)?);
            }
            Ok(enc.write_cstor(cstor_idx, &args_res)?.into())
        }
    }
}

/// Write a value, return an offset to it.
pub fn write_value<'a, 'tmp, W: io::Write>(
    enc: &mut Encoder<W>,
    v: Value<'a, 'tmp>,
) -> io::Result<Offset> {
    let imm = write_value_or_imm(enc, v)?;
    enc.write_immediate_or_return_pointer(imm)
}
