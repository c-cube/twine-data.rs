use std::io::{Read, Write};

use anyhow::{anyhow, Context, Result};
use twine_data::{shallow_value::ShallowValue, Immediate};

struct State {
    w: Box<dyn Write>,
    string_ellipsis_threshold: u32,
}

impl State {
    fn new(w: Box<dyn Write>) -> State {
        State {
            w,
            string_ellipsis_threshold: 24,
        }
    }
}

fn dump_primitive(st: &mut State, v: ShallowValue) -> Result<()> {
    match v {
        ShallowValue::Imm(immediate) => match immediate {
            Immediate::Null => write!(st.w, "null")?,
            Immediate::Bool(b) => write!(st.w, "{b}")?,
            Immediate::Int64(i) => write!(st.w, "{i}")?,
            Immediate::Float(f) => write!(st.w, "{f}")?,
            Immediate::String(s) => write!(st.w, "{s:?}")?,
            Immediate::Bytes(bs) => {
                let len_render = bs.len().max(st.string_ellipsis_threshold as usize);
                let skipped = if bs.len() > len_render {
                    bs.len() - len_render
                } else {
                    0
                };
                let as_hex = hex::encode(&bs[..len_render]);
                write!(st.w, "{as_hex}")?;
                if skipped > 0 {
                    write!(st.w, " [… skipped {skipped}B]")?;
                }
            }
            Immediate::Cstor0(cstor_idx) => todo!(),
            Immediate::Pointer(_) => todo!(),
        },
        ShallowValue::Tag(_, _) => todo!(),
        ShallowValue::Array(array_cursor) => todo!(),
        ShallowValue::Map(map_cursor) => todo!(),
        ShallowValue::Cstor(cstor_idx, array_cursor) => todo!(),
    };
    Ok(())
}

pub fn run(cli: crate::opts::Dump) -> Result<()> {
    dbg!(&cli);

    let mut data = if cli.input.file.to_str() == Some("-") {
        let mut res = vec![];
        std::io::stdin().read_to_end(&mut res)?;
        res
    } else {
        std::fs::read(&cli.input.file).with_context(|| anyhow!("Reading {:?}", &cli.input.file))?
    };

    if cli.input.i_hex {
        data = hex::decode(&data)?;
    }

    let v = twine_data::value::read_value_from_entrypoint(&twine_data::Decoder::new(&data)?)?;
    dbg!(data.len());

    Ok(())
}
