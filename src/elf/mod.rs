mod pass;

use crate::elf::pass::copy_sections::CopySectionsPass;
use crate::elf::pass::loader::LoaderPass;
use crate::elf::pass::relocate::RelocatePass;
use crate::elf::pass::ElfPassAdaptor;
use crate::pass::PassManager;

/// Register passes required to convert an ELF shared library.
pub fn init_passes(pass_mgr: &mut PassManager) {
    // Analyze the data structure after loading the input DSO.
    let loader_pass = pass_mgr.add_pass_default::<ElfPassAdaptor<LoaderPass>>();

    // Copy important sections from the input shared library into the output relocatable file.
    let copy_sections_pass =
        pass_mgr.add_pass(ElfPassAdaptor::adapt(CopySectionsPass::new(loader_pass)));

    // Convert dynamic relocations into static relocations.
    pass_mgr.add_pass(ElfPassAdaptor::adapt(RelocatePass::new(
        loader_pass,
        copy_sections_pass,
    )));
}
