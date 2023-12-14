use object::elf::{R_X86_64_GLOB_DAT, R_X86_64_JUMP_SLOT, R_X86_64_RELATIVE};
use object::read::elf::{ElfFile, FileHeader as ElfFileHeader};
use object::read::Error as ReadError;
use object::write::Relocation as OutputRelocation;
use object::{Architecture, Object as _, ReadRef, RelocationKind, RelocationTarget};
use thiserror::Error;

use crate::elf::pass::section::CopyLodableSectionsPass;
use crate::elf::pass::symbol::GenerateSymbolPass;
use crate::pass::{Pass, PassContext, PassHandle};

/// A pass that converts the dynamic relocations in the input shared library into corresponding static relocations in
/// the output relocatable file.
#[derive(Debug)]
pub struct ConvertRelocationPass {
    pub cls_pass: PassHandle<CopyLodableSectionsPass>,
    pub sym_gen_pass: PassHandle<GenerateSymbolPass>,
}

impl ConvertRelocationPass {
    fn convert_x86_64_relocations<'d, E, R>(
        &self,
        ctx: &PassContext<ElfFile<'d, E, R>>,
    ) -> Result<(), ConvertRelocationError>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>,
    {
        let input_reloc_iter = match ctx.input.dynamic_relocations() {
            Some(iter) => iter,
            None => {
                return Ok(());
            }
        };

        let cls_output = ctx.get_pass_output(self.cls_pass);
        let sym_map = ctx.get_pass_output(self.sym_gen_pass);

        let mut output = ctx.output.borrow_mut();

        for (input_reloc_addr, input_reloc) in input_reloc_iter {
            if input_reloc_addr >= cls_output.output_section_size {
                todo!();
            }

            let output_reloc_offset = input_reloc_addr;

            let output_reloc = match input_reloc.kind() {
                RelocationKind::Elf(R_X86_64_RELATIVE) => OutputRelocation {
                    offset: output_reloc_offset,
                    size: input_reloc.size(),
                    kind: RelocationKind::Absolute,
                    encoding: input_reloc.encoding(),
                    symbol: cls_output.output_section_symbol,
                    addend: input_reloc.addend(),
                },

                RelocationKind::Elf(R_X86_64_GLOB_DAT)
                | RelocationKind::Elf(R_X86_64_JUMP_SLOT) => {
                    let target_sym_idx = match input_reloc.target() {
                        RelocationTarget::Symbol(sym_idx) => sym_idx,
                        _ => todo!(),
                    };
                    let output_sym_id = sym_map.get_output_symbol(target_sym_idx).unwrap();
                    OutputRelocation {
                        offset: output_reloc_offset,
                        size: input_reloc.size(),
                        kind: RelocationKind::Absolute,
                        encoding: input_reloc.encoding(),
                        symbol: output_sym_id,
                        addend: input_reloc.addend(),
                    }
                }

                kind => {
                    return Err(ConvertRelocationError::UnsupportedReloc(kind));
                }
            };

            output
                .add_relocation(cls_output.output_section_id, output_reloc)
                .unwrap();
        }

        Ok(())
    }
}

impl<'d, E, R> Pass<ElfFile<'d, E, R>> for ConvertRelocationPass
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    const NAME: &'static str = "convert relocations";

    type Output = ();
    type Error = ConvertRelocationError;

    fn run(&mut self, ctx: &PassContext<ElfFile<'d, E, R>>) -> Result<Self::Output, Self::Error>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>,
    {
        match ctx.input.architecture() {
            Architecture::X86_64 => {
                self.convert_x86_64_relocations(ctx)?;
            }
            arch => {
                return Err(ConvertRelocationError::UnsupportedArch(arch));
            }
        }

        Ok(())
    }
}

/// Errors that may occur when converting input relocations.
#[derive(Debug, Error)]
pub enum ConvertRelocationError {
    #[error("read ELF failed: {0:?}")]
    ReadElfError(#[from] ReadError),

    #[error("unsupported architecture: {0:?}")]
    UnsupportedArch(Architecture),

    #[error("unsupported reloc: {0:?}")]
    UnsupportedReloc(RelocationKind),
}
