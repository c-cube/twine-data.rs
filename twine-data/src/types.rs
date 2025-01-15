//! Base types.
//!
//! These types are used throughout the library.

use std::fmt::{Debug, Display};

/// An offset in a blob.
pub type Offset = u64;

/// A tag, similar to a CBOR tag.
pub type Tag = u32;

/// A constructor index, used to encode `enum`s (sum types, optional, etc.) by their index.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct VariantIdx(pub u32);

/// Immediate value, without nesting.
///
/// An immediate value is one that can be _skipped_ over efficiently.
/// Only immediate values can be stored in arrays and maps; more complex values have to be
/// stored indirectly via a reference.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Immediate<'a> {
    /// CBOR/JSON null.
    Null,
    Bool(bool),
    Int64(i64),
    Float(f64),
    /// Text, in UTF8
    String(&'a str),
    /// Binary blob.
    Bytes(&'a [u8]),
    /// A variant with 0 arguments.
    Variant0(VariantIdx),
    /// An explicit reference to a full value (which comes at an earlier offset).
    Ref(Offset),
    /// An automatically followed reference to a full value (which comes at an earlier offset).
    Pointer(Offset),
}

impl<'a> Default for Immediate<'a> {
    fn default() -> Self {
        Immediate::Null
    }
}

macro_rules! impl_from {
    ($variant:path, $typ:ty, $typ_real:ty) => {
        impl<'a> From<$typ_real> for Immediate<'a> {
            fn from(v: $typ_real) -> Immediate<'a> {
                $variant(v as $typ)
            }
        }
    };
}

impl<'a> From<()> for Immediate<'a> {
    fn from(_v: ()) -> Self {
        Immediate::Null
    }
}

impl_from!(Immediate::Bool, bool, bool);
impl_from!(Immediate::Int64, i64, i64);
impl_from!(Immediate::Int64, i64, i32);
impl_from!(Immediate::Float, f64, f64);
impl_from!(Immediate::Float, f64, f32);
impl_from!(Immediate::String, &'a str, &'a str);
impl_from!(Immediate::Bytes, &'a [u8], &'a [u8]);
impl_from!(Immediate::Pointer, Offset, Offset);

#[derive(Debug, Clone, Copy)]
pub struct Error {
    /// Error message.
    pub msg: &'static str,
    /// Offset at which error occurred in the twine blob.
    pub off: Offset,
}

pub type Result<T> = core::result::Result<T, Error>;

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Twine error: {} at offset=0x{:x} ({})",
            self.msg, self.off, self.off
        )
    }
}

impl std::error::Error for Error {}
