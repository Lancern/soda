pub mod section;
pub mod symbol;

use std::error::Error;

use object::read::elf::{ElfFile, FileHeader as ElfFileHeader};
use object::read::File as InputFile;
use object::write::Object as OutputObject;
use object::ReadRef;

use crate::ctx::Context;
use crate::pass::{Pass, PassContext, PassHandle, PassManager};

pub trait ElfPass: 'static {
    const NAME: &'static str;

    type Output<'a>;
    type Error: Error + Send + Sync;

    fn run<'d, E, R>(
        &mut self,
        ctx: &PassContext<'d>,
        input: &ElfFile<'d, E, R>,
        output: &mut OutputObject<'d>,
    ) -> Result<Self::Output<'d>, Self::Error>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>;
}

/// Adapt a type that implements [`ElfPass`] to a type that implements [`Pass`].
#[derive(Debug, Default)]
pub struct ElfPassAdaptor<P>(P);

impl<P> ElfPassAdaptor<P> {
    pub fn adapt(pass: P) -> Self {
        Self(pass)
    }
}

impl<P: ElfPass> Pass for ElfPassAdaptor<P> {
    const NAME: &'static str = P::NAME;

    type Output<'a> = P::Output<'a>;
    type Error = P::Error;

    fn run<'d>(
        &mut self,
        ctx: &PassContext<'d>,
        soda: &mut Context<'d>,
    ) -> Result<Self::Output<'d>, Self::Error> {
        match &soda.input {
            InputFile::Elf32(elf32) => self.0.run(ctx, elf32, &mut soda.output),
            InputFile::Elf64(elf64) => self.0.run(ctx, elf64, &mut soda.output),
            _ => unreachable!(),
        }
    }
}

pub type ElfPassHandle<P> = PassHandle<ElfPassAdaptor<P>>;

pub trait PassManagerExt {
    fn add_elf_pass<P: ElfPass>(&mut self, pass: P) -> ElfPassHandle<P>;

    fn add_elf_pass_default<P>(&mut self) -> ElfPassHandle<P>
    where
        P: ElfPass + Default,
    {
        self.add_elf_pass(P::default())
    }
}

impl PassManagerExt for PassManager {
    fn add_elf_pass<P: ElfPass>(&mut self, pass: P) -> ElfPassHandle<P> {
        self.add_pass(ElfPassAdaptor::adapt(pass))
    }
}
