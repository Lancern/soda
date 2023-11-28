use std::error::Error;
use std::path::PathBuf;

use log::{Level as LogLevel, SetLoggerError};
use structopt::StructOpt;

#[derive(Clone, Debug, StructOpt)]
#[structopt(
    name = "soda",
    author = "Sirui Mu <msrlancern@gmail.com>",
    about = "Convert shared libraries into static libraries"
)]
struct Args {
    /// Path to the input shared library.
    #[structopt(parse(from_os_str))]
    input: PathBuf,

    /// Path to the output relocatable object file.
    #[structopt(short, long)]
    #[structopt(parse(from_os_str))]
    output: Option<PathBuf>,

    /// Output verbosity.
    #[structopt(long, parse(from_occurrences))]
    verbosity: u8,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::from_args();
    init_logger(args.verbosity)?;

    // TODO: implement main.

    Ok(())
}

fn init_logger(verbosity: u8) -> Result<(), SetLoggerError> {
    let level = match verbosity {
        0 => LogLevel::Info,
        1 => LogLevel::Debug,
        _ => LogLevel::Trace,
    };
    simple_logger::init_with_level(level)?;
    Ok(())
}
