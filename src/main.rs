mod ctx;
mod elf;
mod pass;

use std::borrow::Cow;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context as _};
use log::{Level as LogLevel, SetLoggerError};
use object::BinaryFormat;
use structopt::StructOpt;

use crate::ctx::Context;
use crate::pass::PassManager;

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
    #[structopt(short, parse(from_occurrences))]
    verbosity: u8,
}

impl Args {
    fn get_output_path(&self) -> Cow<Path> {
        if let Some(path) = &self.output {
            return Cow::Borrowed(path);
        }

        // If the user does not provide an output path, we form one by replacing the file name part of the input path
        // with a proper static library name.

        let mut path = self.input.clone();
        path.set_extension("a");

        Cow::Owned(path)
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::from_args();
    init_logger(args.verbosity)?;

    log::info!("Reading input shared library ...");
    let input_buffer = std::fs::read(&args.input).context(format!(
        "failed to read input shared library \"{}\"",
        args.input.display()
    ))?;

    let mut output_buffer = Vec::new();
    let mut ctx = Context::new(&input_buffer, &mut output_buffer)?;

    // Initialize passes.
    let mut passes = PassManager::new();
    match ctx.format() {
        BinaryFormat::Elf => {
            crate::elf::init_passes(&mut passes);
        }
        _ => unreachable!(),
    }

    // Open the output file, preparing to write later.
    let output_path = &*args.get_output_path();
    let mut output_file = OutputFile::create(output_path)
        .context(format!("failed to open output file \"{}\"", output_path.display()))?;

    // Run the passes.
    log::info!("Running all registered passes ...");
    passes.run(&mut ctx)?;

    // Save the produced output object to the output file.
    log::info!("Writing output file ...");
    ctx.output
        .write_stream(output_file.writer())
        .map_err(|err| anyhow!(format!("{:?}", err)))
        .context(format!("failed to write output file \"{}\"", output_path.display()))?;

    output_file.prevent_delete_on_drop();
    log::info!("Done.");

    Ok(())
}

fn init_logger(verbosity: u8) -> Result<(), SetLoggerError> {
    let level = match verbosity {
        0 => LogLevel::Warn,
        1 => LogLevel::Info,
        2 => LogLevel::Debug,
        _ => LogLevel::Trace,
    };
    simple_logger::init_with_level(level)?;
    Ok(())
}

#[derive(Debug)]
struct OutputFile {
    path: PathBuf,
    file: Option<BufWriter<File>>,
    delete_on_drop: bool,
}

impl OutputFile {
    fn create(path: &Path) -> Result<Self, std::io::Error> {
        let file = File::create(path)?;
        Ok(Self {
            path: PathBuf::from(path),
            file: Some(BufWriter::new(file)),
            delete_on_drop: true,
        })
    }

    fn writer(&mut self) -> &mut BufWriter<File> {
        self.file.as_mut().unwrap()
    }

    fn prevent_delete_on_drop(&mut self) {
        self.delete_on_drop = false;
    }
}

impl Drop for OutputFile {
    fn drop(&mut self) {
        if !self.delete_on_drop {
            return;
        }

        self.file.take(); // Close the output file.
        std::fs::remove_file(&self.path).ok();
    }
}
