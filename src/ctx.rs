use object::elf::{ELFDATA2LSB, ELFDATA2MSB, EM_386, EM_X86_64};
use object::read::elf::{ElfFile, ElfFile32, ElfFile64, FileHeader as ElfFileHeader};
use object::read::{Error as ObjectReadError, File as InputFile, ReadRef};
use object::write::Object as OutputObject;
use object::{Architecture, BinaryFormat, Endianness};
use thiserror::Error;

/// Provide a context for the whole converting process.
#[derive(Debug)]
#[non_exhaustive]
pub struct Context<'d> {
    /// The input file.
    pub input: InputFile<'d>,

    /// The output object.
    pub output: OutputObject<'d>,
}

impl<'d> Context<'d> {
    /// Create a new context.
    ///
    /// This function will:
    /// - Parse the data in the input buffer as a shared library;
    /// - Check the class, architecture, OS, etc. and ensure that we can properly handle the shared library;
    /// - Create an output object builder on top of the given output buffer;
    /// - Create a new `Context` object and wraps the parsed input file and output object.
    pub fn new(input_buffer: &'d [u8]) -> Result<Self, CreateContextError> {
        let input = InputFile::parse(input_buffer)?;
        let output = match &input {
            InputFile::Elf32(elf32) => Self::from_elf_input_file(elf32)?,
            InputFile::Elf64(elf64) => Self::from_elf_input_file(elf64)?,
            input => return Err(CreateContextError::UnsupportedBinaryFormat(input.format())),
        };
        Ok(Self { input, output })
    }

    /// Get the binary format.
    pub fn format(&self) -> BinaryFormat {
        self.output.format()
    }

    fn from_elf_input_file<E>(
        input: &ElfFile<'d, E>,
    ) -> Result<OutputObject<'d>, CreateContextError>
    where
        E: ElfFileHeader<Endian = Endianness>,
    {
        // Checks that the architecture and OS is supported by current version.
        let elf_ident = input.raw_header().e_ident();

        let endian = match elf_ident.data {
            ELFDATA2LSB => Endianness::Little,
            ELFDATA2MSB => Endianness::Big,
            _ => {
                return Err(CreateContextError::corrupted_data(
                    "unknown ELFDATA* value in ELF ident",
                ));
            }
        };

        let elf_machine = input.raw_header().e_machine(endian);
        let arch = match elf_machine {
            EM_386 => Architecture::I386,
            EM_X86_64 => Architecture::X86_64,
            _ => {
                return Err(CreateContextError::UnsupportedArch);
            }
        };

        Ok(OutputObject::new(BinaryFormat::Elf, arch, endian))
    }
}

/// Errors that occur when creating a new context.
#[derive(Debug, Error)]
pub enum CreateContextError {
    /// The data format in the input buffer is unknown.
    #[error("unknown input data format: {0:?}")]
    UnknownFormat(#[from] ObjectReadError),

    /// The data format in the input buffer is recognized, but some data fields are invalid.
    #[error("corrected input file data: {0}")]
    CorruptedData(String),

    /// The binary object format in the input buffer is not yet supported.
    #[error("unsupported input data format: {}", get_binary_format_str(*.0))]
    UnsupportedBinaryFormat(BinaryFormat),

    /// The architecture is not yet supported.
    #[error("unsupported architecture")]
    UnsupportedArch,
}

impl CreateContextError {
    fn corrupted_data<T>(text: T) -> Self
    where
        T: Into<String>,
    {
        Self::CorruptedData(text.into())
    }
}

fn get_binary_format_str(f: BinaryFormat) -> &'static str {
    match f {
        BinaryFormat::Coff => "coff",
        BinaryFormat::Elf => "elf",
        BinaryFormat::MachO => "macho",
        BinaryFormat::Pe => "pe",
        BinaryFormat::Wasm => "wasm",
        BinaryFormat::Xcoff => "xcoff",
        _ => "unknown",
    }
}

trait InputFileFromExt<T> {
    fn from(value: T) -> Self;
}

impl<'d, R> InputFileFromExt<ElfFile32<'d, Endianness, R>> for InputFile<'d, R>
where
    R: ReadRef<'d>,
{
    fn from(value: ElfFile32<'d, Endianness, R>) -> Self {
        Self::Elf32(value)
    }
}

impl<'d, R> InputFileFromExt<ElfFile64<'d, Endianness, R>> for InputFile<'d, R>
where
    R: ReadRef<'d>,
{
    fn from(value: ElfFile64<'d, Endianness, R>) -> Self {
        Self::Elf64(value)
    }
}
