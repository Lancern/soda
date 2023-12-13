mod elf;
mod pass;
mod utils;

use std::borrow::Cow;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, Context as _};
use log::{Level as LogLevel, SetLoggerError};
use object::read::{File as InputFile, ObjectKind};
use object::Object as _;
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
        //
        // Examples of name conversion:
        // - `/dir/libxyz.so` will be converted to `/dir/xyz.o`
        // - `/dir/xyz.so` will be converted to `/dir/xyz.o`

        let mut path = self.input.clone();
        let file_name = path.file_name().unwrap().to_str().unwrap();
        path.set_file_name(convert_soname_to_object_name(file_name));

        Cow::Owned(path)
    }
}

fn main() -> ExitCode {
    let args = Args::from_args();
    if let Err(err) = do_main(&args) {
        eprintln!("Error: {:#}", err);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn do_main(args: &Args) -> anyhow::Result<()> {
    init_logger(args.verbosity)?;

    log::info!("Reading input shared library ...");
    let input_buffer = std::fs::read(&args.input).context(format!(
        "cannot read input shared library \"{}\"",
        args.input.display()
    ))?;
    let input_file = InputFile::parse(input_buffer.as_slice()).context(format!(
        "cannot parse input shared library \"{}\"",
        args.input.display()
    ))?;

    if input_file.kind() != ObjectKind::Dynamic {
        return Err(anyhow::Error::msg("input file is not a shared library"));
    }

    // Open the output file, preparing to write later.
    let output_path = &*args.get_output_path();
    let mut output_file = OutputFile::create(output_path).context(format!(
        "failed to open output file \"{}\"",
        output_path.display()
    ))?;

    // Convert the input shared library into output relocatable file.
    log::info!("Start the conversion");
    let output_object = match input_file {
        InputFile::Elf32(elf_file) => crate::elf::convert(elf_file)?,
        InputFile::Elf64(elf_file) => crate::elf::convert(elf_file)?,
        _ => {
            let err = anyhow::Error::msg(format!(
                "{} format is not supported yet",
                crate::utils::stringify::binary_format_to_str(input_file.format())
            ));
            return Err(err);
        }
    };

    // Save the produced output object to the output file.
    log::info!("Writing output file ...");
    output_object
        .write_stream(output_file.writer())
        .map_err(|err| anyhow!(format!("{:?}", err)))
        .context(format!(
            "failed to write output file \"{}\"",
            output_path.display()
        ))?;

    output_file.prevent_delete_on_drop();
    log::info!("Done.");

    Ok(())
}

/// Convert a shared library name into its corresponding object name.
///
/// Examples of the conversion:
/// - `libxyz.so` will be converted to `xyz.o`
/// - `xyz.so` will be converted to `xyz.o`
/// - `xyz` will be converted to `xyz.o`
///
/// Specifically:
/// - If the given soname does not ends with .so (regardless of case), then a plain ".o" suffix will be added to the
///   given name and we're done.
/// - Otherwise, replace the ".so" suffix with ".o".
/// - If the given soname begins with "lib" (regardless of case), remove that prefix.
fn convert_soname_to_object_name(soname: &str) -> String {
    let name_core = 'b: {
        if soname.len() < 3 {
            break 'b soname;
        }

        let (file_name_wo_ext, ext_suffix) = soname.split_at(soname.len() - 3);
        if ext_suffix.to_lowercase() != ".so" {
            // The given soname does not ends with .so.
            return format!("{}.o", soname);
        }

        if file_name_wo_ext.len() >= 3 {
            let (lib_prefix, name_core) = file_name_wo_ext.split_at(3);
            if lib_prefix.to_lowercase() == "lib" {
                break 'b name_core;
            }
        }

        file_name_wo_ext
    };

    format!("{}.o", name_core)
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
