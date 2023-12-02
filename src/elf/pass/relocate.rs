use std::convert::Infallible;

use object::read::elf::{ElfFile, FileHeader as ElfFileHeader};
use object::write::elf::Writer as ElfWriter;
use object::write::Object as OutputObject;
use object::ReadRef;

use crate::elf::pass::copy_sections::CopySectionsPass;
use crate::elf::pass::loader::LoaderPass;
use crate::elf::{ElfPass, ElfPassHandle};
use crate::pass::PassContext;

#[derive(Debug)]
pub struct RelocatePass {
    loader: ElfPassHandle<LoaderPass>,
    copied_sections: ElfPassHandle<CopySectionsPass>,
}

impl RelocatePass {
    pub fn new(
        loader: ElfPassHandle<LoaderPass>,
        copied_sections: ElfPassHandle<CopySectionsPass>,
    ) -> Self {
        Self {
            loader,
            copied_sections,
        }
    }
}

impl ElfPass for RelocatePass {
    const NAME: &'static str = "relocate";

    type Output<'a> = ();
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
