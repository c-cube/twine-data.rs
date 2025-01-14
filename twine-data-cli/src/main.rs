use anyhow::Result;
use clap::Parser;

mod dump;
mod opts;

fn main() -> Result<()> {
    let cli = opts::Cli::try_parse()?;
    match cli {
        opts::Cli::Dump(dump) => dump::run(dump)?,
        opts::Cli::FromJson(_) => todo!(),
    }

    Ok(())
}
