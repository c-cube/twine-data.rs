use clap::Parser;
use std::path::PathBuf;

/// Input
#[derive(Parser, Debug, Clone)]
pub struct Input {
    pub file: PathBuf,
    #[arg(long)]
    pub i_hex: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct Dump {
    #[command(flatten)]
    pub input: Input,
    #[arg(long)]
    pub hexdump: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct FromJson {}

#[derive(Parser, Debug, Clone)]
pub enum Cli {
    Dump(Dump),
    FromJson(FromJson),
}
