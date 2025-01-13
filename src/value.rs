//! Full, nested values, with individual allocations.
//!
//! Such values are useful for various tools that need a global view of the
//! data, or for manipulating twine data as if it were JSON.

use std::io;

use crate::{shallow_value::ShallowValue, Encoder};

use super::{
    types::{CstorIdx, Offset, Tag},
    Decoder, Immediate, Result,
};

/// A value, potentially containing other values. All the sub-values live in the same allocator.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Value {
    Null,
    Bool(bool),
    Int64(i64),
    Float(f64),
    /// Text, in UTF8
    String(String),
    /// Binary blob.
    Bytes(Vec<u8>),
    /// A constructor with 0 arguments.
    Cstor0(CstorIdx),
    /// A reference to a full value (which comes at an earlier offset).
    Pointer(Offset),
    Tag(Tag, Box<Value>),
    Array(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Cstor(CstorIdx, Vec<Value>),
}

impl Default for Value {
    fn default() -> Self {
        Value::Null
    }
}

impl<'a> From<Immediate<'a>> for Value {
    fn from(v: Immediate<'a>) -> Self {
        match v {
            Immediate::Null => Value::Null,
            Immediate::Bool(b) => Value::Bool(b),
            Immediate::Int64(i) => Value::Int64(i),
            Immediate::Float(f) => Value::Float(f),
            Immediate::String(s) => Value::String(s.to_string()),
            Immediate::Bytes(bs) => Value::Bytes(bs.to_vec()),
            Immediate::Cstor0(cstor_idx) => Value::Cstor0(cstor_idx),
            Immediate::Pointer(p) => Value::Pointer(p),
        }
    }
}

/// Read a value from a decoder, starting at given offset.
pub fn read_value(d: &Decoder, off: Offset) -> Result<Value> {
    let v: Value = match d.get_shallow_value(off)? {
        ShallowValue::Imm(v) => Value::from(v),
        ShallowValue::Tag(tag, off) => Value::Tag(tag, Box::new(read_value(d, off)?)),
        ShallowValue::Array(arr) => {
            let mut arr_v = Vec::with_capacity(arr.len());
            for off in arr {
                let off = off?;
                arr_v.push(read_value(d, off)?)
            }
            Value::Array(arr_v)
        }
        ShallowValue::Map(map) => {
            let mut map_v = Vec::with_capacity(map.len());
            for kv in map {
                let (k, v) = kv?;
                map_v.push((read_value(d, k)?, read_value(d, v)?))
            }
            Value::Map(map_v)
        }
        ShallowValue::Cstor(cstor_idx, args) => {
            let mut args_v = Vec::with_capacity(args.len());
            for a in args {
                let a = a?;
                args_v.push(read_value(d, a)?)
            }
            Value::Cstor(cstor_idx, args_v)
        }
    };
    Ok(v)
}

/// Find the entrypoint and read a value from it.
pub fn read_value_from_entrypoint(d: &Decoder) -> Result<Value> {
    let off = d.entrypoint()?;
    read_value(d, off)
}

fn write_value_or_imm<'a, W: io::Write>(
    enc: &'_ mut Encoder<W>,
    v: &'a Value,
) -> io::Result<Immediate<'a>> {
    let imm = match v {
        Value::Null => Immediate::Null,
        Value::Bool(b) => Immediate::Bool(*b),
        Value::Int64(i) => Immediate::Int64(*i),
        Value::Float(f) => Immediate::Float(*f),
        Value::String(s) => Immediate::String(&s),
        Value::Bytes(vec) => Immediate::Bytes(&vec),
        Value::Cstor0(cstor_idx) => Immediate::Cstor0(*cstor_idx),
        Value::Pointer(p) => Immediate::Pointer(*p),
        Value::Tag(tag, v) => {
            let v = write_value_or_imm(enc, v)?;
            enc.write_tag(*tag, v)?.into()
        }
        Value::Array(arr) => {
            // locally gather immediates
            let mut res = Vec::with_capacity(arr.len());
            for x in arr {
                res.push(write_value_or_imm(enc, x)?);
            }
            enc.write_array(&res)?.into()
        }
        Value::Map(map) => {
            // locally gather immediates
            let mut res = Vec::with_capacity(map.len());
            for (k, v) in map {
                let k = write_value_or_imm(enc, k)?;
                let v = write_value_or_imm(enc, v)?;
                res.push((k, v));
            }
            enc.write_map(&res)?.into()
        }
        Value::Cstor(cstor_idx, args) => {
            let mut args_res = Vec::with_capacity(args.len());
            for x in args {
                args_res.push(write_value_or_imm(enc, x)?);
            }
            enc.write_cstor(*cstor_idx, &args_res)?.into()
        }
    };
    Ok(imm)
}

/// Write a value, return an offset to it.
pub fn write_value<'a, W: io::Write>(enc: &mut Encoder<W>, v: &Value) -> io::Result<Offset> {
    let imm = write_value_or_imm(enc, v)?;
    enc.write_immediate_or_return_pointer(imm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn arb_values() -> impl Strategy<Value = Value> {
        // https://proptest-rs.github.io/proptest/proptest/tutorial/recursive.html heck yeah
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(|b| Value::Bool(b)),
            any::<i64>().prop_map(|b| Value::Int64(b)),
            any::<f64>().prop_map(|b| Value::Float(b)),
            ".*".prop_map(|s| Value::String(s)),
            // prop::collection::vec(arb_json(), 0..10).prop_map(Json::Array),
            // prop::collection::hash_map(".*", arb_json(), 0..10).prop_map(Json::Map),
        ];
        leaf.prop_recursive(8, 256, 10, |inner| prop_oneof![
            prop::collection::vec(inner.clone(), 0..10).prop_map(|v| Value::Array(v)),
            prop::collection::vec((inner.clone(), inner.clone()), 0..10).prop_map(|map| Value::Map(map))
        ]).boxed()
    }

    proptest! {
        #[test]
        fn encode_then_decode(v in arb_values()) {
            let mut res = vec![];
            let mut enc= crate::Encoder::new(&mut res);
            let offset = write_value(&mut enc, &v).unwrap();

            let v2 = read_value(&Decoder::new(&res[..]).unwrap(), offset).unwrap();
            assert_eq!(v, v2);
        }
    }
}
