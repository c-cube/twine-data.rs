//! Shallow values.

use crate::{
    types::{CstorIdx, Offset, Tag},
    Decoder, Immediate, Result,
};

/// A value, potentially containing other values.
///
/// Lifetime 'a is the lifetime of the decode (and string slices in it)
#[derive(Debug, Clone)]
pub enum ShallowValue<'a> {
    Imm(Immediate<'a>),
    Tag(Tag, Offset),
    Array(ArrayCursor<'a>),
    Dict(DictCursor<'a>),
    Cstor(CstorIdx, ArrayCursor<'a>),
}

#[derive(Debug, Clone)]
pub struct ArrayCursor<'a> {
    pub(crate) dec: Decoder<'a>,
    pub(crate) off: Offset,
    pub(crate) n_items: u32,
}

impl<'a> ArrayCursor<'a> {
    pub fn len(&self) -> usize {
        self.n_items as usize
    }
}

impl<'a> Iterator for ArrayCursor<'a> {
    type Item = Result<Offset>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.n_items == 0 {
            return None;
        }

        let off = self.off;

        match self.dec.skip(off) {
            Ok(off2) => {
                self.off = off2;
                self.n_items -= 1;
                Some(Ok(off))
            }
            Err(e) => {
                self.n_items = 0;
                Some(Err(e))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct DictCursor<'a> {
    pub(crate) dec: Decoder<'a>,
    pub(crate) off: Offset,
    pub(crate) n_items: u32,
}

impl<'a> DictCursor<'a> {
    pub fn len(&self) -> usize {
        self.n_items as usize
    }
}

impl<'a> Iterator for DictCursor<'a> {
    type Item = Result<(Offset, Offset)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.n_items == 0 {
            return None;
        }

        let k_off = self.off;

        match self.dec.skip(k_off) {
            Ok(v_off) => {
                self.off = v_off;

                match self.dec.skip(v_off) {
                    Ok(off2) => {
                        self.n_items -= 1;
                        self.off = off2;
                        Some(Ok((k_off, v_off)))
                    }
                    Err(e) => {
                        self.n_items = 0;
                        Some(Err(e))
                    }
                }
            }
            Err(e) => {
                self.n_items = 0;
                Some(Err(e))
            }
        }
    }
}
