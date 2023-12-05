mod pass;

use crate::elf::pass::section::CopyLodableSectionsPass;
use crate::elf::pass::PassManagerExt;
use crate::pass::PassManager;

use self::pass::symbol::GenerateSymbolPass;

/// Register passes required to convert an ELF shared library.
pub fn init_passes(pass_mgr: &mut PassManager) {
    // Copy input sections to output sections.
    let cls_pass = pass_mgr.add_elf_pass_default::<CopyLodableSectionsPass>();

    // TODO: add a pass to copy the dynamic symbols in the input shared library into the normal symbols in the output
    // relocatable object.
    let sym_gen_pass = pass_mgr.add_elf_pass(GenerateSymbolPass::new(cls_pass));

    // TODO: add a pass to convert the relocations in the input shared library.
}
