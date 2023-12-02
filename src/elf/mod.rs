mod pass;

use std::error::Error;

use object::read::elf::{ElfFile, FileHeader as ElfFileHeader};
use object::write::elf::Writer as ElfWriter;
use object::write::Object as OutputObject;
use object::{File as InputFile, ReadRef};

use crate::ctx::Context;
use crate::elf::pass::copy_sections::CopySectionsPass;
use crate::elf::pass::loader::LoaderPass;
use crate::elf::pass::relocate::RelocatePass;
use crate::pass::{Pass, PassContext, PassHandle, PassManager};

/// Register passes required to convert an ELF shared library.
pub fn init_passes(pass_mgr: &mut PassManager) {
    // Analyze the data structure after loading the input DSO.
    let loader_pass = pass_mgr.add_pass_default::<ElfPassAdaptor<LoaderPass>>();

    // Copy important sections from the input shared library into the output relocatable file.
    let copy_sections_pass = pass_mgr.add_pass(ElfPassAdaptor(CopySectionsPass::new(loader_pass)));

    // Convert dynamic relocations into static relocations.
    pass_mgr.add_pass(ElfPassAdaptor(RelocatePass::new(
        loader_pass,
        copy_sections_pass,
    )));
}

pub trait ElfPass: 'static {
    const NAME: &'static str;

    type Output<'a>;
    type Error: Error + Send + Sync;

    fn run<'d, E, R>(
        &mut self,
        ctx: &PassContext<'_, 'd>,
        input: &ElfFile<'d, E, R>,
        output: &mut OutputObject<'d>,
        output_writer: &mut ElfWriter,
    ) -> Result<Self::Output<'d>, Self::Error>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>;
}

/// Adapt a type that implements [`ElfPass`] to a type that implements [`Pass`].
#[derive(Debug, Default)]
pub struct ElfPassAdaptor<P>(P);

impl<P: ElfPass> Pass for ElfPassAdaptor<P> {
    const NAME: &'static str = P::NAME;

    type Output<'a> = P::Output<'a>;
    type Error = P::Error;

    fn run<'d>(
        &mut self,
        ctx: &PassContext<'_, 'd>,
        soda: &mut Context<'d>,
    ) -> Result<Self::Output<'d>, Self::Error> {
        let mut writer = ElfWriter::new(soda.endian(), soda.is_64(), soda.output_buffer);
        match &soda.input {
            InputFile::Elf32(elf32) => self.0.run(ctx, elf32, &mut soda.output, &mut writer),
            InputFile::Elf64(elf64) => self.0.run(ctx, elf64, &mut soda.output, &mut writer),
            _ => unreachable!(),
        }
    }
}

pub type ElfPassHandle<P> = PassHandle<ElfPassAdaptor<P>>;
