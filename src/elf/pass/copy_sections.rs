use std::convert::Infallible;

use object::read::elf::{ElfFile, FileHeader as ElfFileHeader};
use object::write::elf::Writer as ElfWriter;
use object::write::Object as OutputObject;
use object::ReadRef;

use crate::elf::pass::loader::LoaderPass;
use crate::elf::{ElfPass, ElfPassHandle};
use crate::pass::PassContext;

/// A pass that copies section data from input shared library to output relocatable file.
#[derive(Debug)]
pub struct CopySectionsPass {
    loader: ElfPassHandle<LoaderPass>,
}

impl CopySectionsPass {
    pub fn new(loader: ElfPassHandle<LoaderPass>) -> Self {
        Self { loader }
    }
}

impl ElfPass for CopySectionsPass {
    const NAME: &'static str = "copy sections";

    type Output<'a> = CopiedSections;
    type Error = Infallible;

    fn run<'d, E, R>(
        &mut self,
        ctx: &PassContext<'d>,
        input: &ElfFile<'d, E, R>,
        output: &mut OutputObject<'d>,
        output_writer: &mut ElfWriter,
    ) -> Result<Self::Output<'d>, Self::Error>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>,
    {
        todo!()
    }
}

#[derive(Debug)]
pub struct CopiedSections {
    // TODO: implement struct CopiedSections
}
