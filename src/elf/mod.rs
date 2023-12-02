mod pass;

use crate::elf::pass::loader::LoaderPass;
use crate::elf::pass::relocate::RelocatePass;
use crate::elf::pass::PassManagerExt;
use crate::pass::PassManager;

/// Register passes required to convert an ELF shared library.
pub fn init_passes(pass_mgr: &mut PassManager) {
    // Load the input shared library and produce a memory mapping of the sections loaded in the input shared library.
    let loader_pass = pass_mgr.add_elf_pass_default::<LoaderPass>();

    // Convert dynamic relocations into static relocations.
    pass_mgr.add_elf_pass(RelocatePass::new(loader_pass));

    // TODO: add a pass to write the output object.
}
