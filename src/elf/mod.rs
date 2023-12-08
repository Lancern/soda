mod pass;

use crate::elf::pass::reloc::ConvertRelocationPass;
use crate::elf::pass::section::CopyLodableSectionsPass;
use crate::elf::pass::symbol::GenerateSymbolPass;
use crate::elf::pass::PassManagerExt;
use crate::pass::PassManager;

/// Register passes required to convert an ELF shared library.
pub fn init_passes(pass_mgr: &mut PassManager) {
    // Copy input sections to output sections.
    let cls_pass = pass_mgr.add_elf_pass_default::<CopyLodableSectionsPass>();

    // Copy the dynamic symbols in the input shared library into the normal symbols in the output relocatable object.
    let sym_gen_pass = pass_mgr.add_elf_pass(GenerateSymbolPass { cls_pass });

    // Convert the dynamic relocations in the input shared library to corresponding static relocations in the output
    // relocatable file.
    pass_mgr.add_elf_pass(ConvertRelocationPass {
        cls_pass,
        sym_gen_pass,
    });
}
