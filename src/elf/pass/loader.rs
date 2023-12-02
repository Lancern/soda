use std::convert::Infallible;

use object::read::elf::{ElfFile, FileHeader as ElfFileHeader};
use object::write::elf::Writer as OutputWriter;
use object::write::Object as OutputObject;
use object::ReadRef;

use crate::elf::ElfPass;
use crate::pass::PassContext;

#[derive(Debug, Default)]
pub struct LoaderPass;

impl ElfPass for LoaderPass {
    const NAME: &'static str = "loader";

    type Output<'a> = LoadedDso;
    type Error = Infallible;

    fn run<'d, E, R>(
        &mut self,
        ctx: &PassContext<'d>,
        input: &ElfFile<'d, E, R>,
        output: &mut OutputObject<'d>,
        output_writer: &mut OutputWriter,
    ) -> Result<Self::Output<'d>, Self::Error>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>,
    {
        todo!()
    }
}

#[derive(Debug)]
pub struct LoadedDso {
    // TODO: implement LoadedDso.
}
