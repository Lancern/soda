use std::convert::Infallible;

use object::read::elf::{ElfFile, FileHeader as ElfFileHeader};
use object::write::Object as OutputObject;
use object::ReadRef;

use crate::elf::pass::loader::LoaderPass;
use crate::elf::pass::{ElfPass, ElfPassHandle};
use crate::pass::PassContext;

#[derive(Debug)]
pub struct RelocatePass {
    loader: ElfPassHandle<LoaderPass>,
}

impl RelocatePass {
    pub fn new(loader: ElfPassHandle<LoaderPass>) -> Self {
        Self { loader }
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
    ) -> Result<Self::Output<'d>, Self::Error>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>,
    {
        todo!()
    }
}
