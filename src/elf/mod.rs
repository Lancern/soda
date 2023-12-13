mod pass;

use object::read::elf::{ElfFile, FileHeader as ElfFileHeader};
use object::write::Object as OutputObject;
use object::{BinaryFormat, Endian, Endianness, Object as _, ReadRef};

use crate::elf::pass::reloc::ConvertRelocationPass;
use crate::elf::pass::section::CopyLodableSectionsPass;
use crate::elf::pass::symbol::GenerateSymbolPass;
use crate::pass::PassManager;

pub fn convert<'d, E, R>(input: ElfFile<'d, E, R>) -> anyhow::Result<OutputObject<'static>>
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    let output = create_elf_output(&input);

    let mut pass_mgr = PassManager::new();
    init_passes(&mut pass_mgr);

    let output = pass_mgr.run(input, output)?;
    Ok(output)
}

fn create_elf_output<'d, E, R>(input: &ElfFile<'d, E, R>) -> OutputObject<'static>
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    let endian = Endianness::from_big_endian(input.endian().is_big_endian()).unwrap();
    let arch = input.architecture();
    OutputObject::new(BinaryFormat::Elf, arch, endian)
}

/// Register passes required to convert an ELF shared library.
fn init_passes<'d, E, R>(pass_mgr: &mut PassManager<ElfFile<'d, E, R>>)
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    // Copy input sections to output sections.
    let cls_pass = pass_mgr.add_pass_default::<CopyLodableSectionsPass>();

    // Copy the dynamic symbols in the input shared library into the normal symbols in the output relocatable object.
    let sym_gen_pass = pass_mgr.add_pass(GenerateSymbolPass { cls_pass });

    // Convert the dynamic relocations in the input shared library to corresponding static relocations in the output
    // relocatable file.
    pass_mgr.add_pass(ConvertRelocationPass {
        cls_pass,
        sym_gen_pass,
    });
}
