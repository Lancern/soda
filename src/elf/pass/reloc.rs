use object::elf::{R_X86_64_GLOB_DAT, R_X86_64_JUMP_SLOT, R_X86_64_RELATIVE};
use object::read::elf::{ElfFile, FileHeader as ElfFileHeader};
use object::read::Error as ReadError;
use object::write::{Object as OutputObject, Relocation as OutputRelocation};
use object::{Architecture, Object as _, ReadRef, RelocationKind, RelocationTarget};
use thiserror::Error;

use crate::elf::pass::section::CopyLodableSectionsPass;
use crate::elf::pass::symbol::GenerateSymbolPass;
use crate::elf::pass::{ElfPass, ElfPassHandle};
use crate::pass::PassContext;

/// A pass that converts the dynamic relocations in the input shared library into corresponding static relocations in
/// the output relocatable file.
#[derive(Debug)]
pub struct ConvertRelocationPass {
    pub cls_pass: ElfPassHandle<CopyLodableSectionsPass>,
    pub sym_gen_pass: ElfPassHandle<GenerateSymbolPass>,
}

impl ConvertRelocationPass {
    fn convert_x86_64_relocations<'d, 'f, E, R>(
        &self,
        ctx: &PassContext<'d>,
        input: &ElfFile<'d, E, R>,
        output: &mut OutputObject<'d>,
    ) -> Result<(), ConvertRelocationError>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>,
    {
        let input_reloc_iter = match input.dynamic_relocations() {
            Some(iter) => iter,
            None => {
                return Ok(());
            }
        };

        let cls_output = ctx.get_pass_output(self.cls_pass);
        let sym_map = ctx.get_pass_output(self.sym_gen_pass);

        for (input_reloc_addr, input_reloc) in input_reloc_iter {
            let output_reloc_offset = match cls_output.map_input_addr(input_reloc_addr) {
                Some(offset) => offset,
                None => {
                    continue;
                }
            };

            let output_reloc = match input_reloc.kind() {
                RelocationKind::Elf(R_X86_64_RELATIVE) => {
                    // For an R_X86_64_RELATIVE reloc, its addend is the virtual address of the relocation target in the
                    // input shared library. Thus we need to map its addend to the corresponding location in the output
                    // relocatable section.
                    let output_addend = match cls_output.map_input_addr(input_reloc.addend() as u64)
                    {
                        Some(addend) => addend,
                        None => {
                            log::warn!("Relocation target is out of lodable input sections");
                            continue;
                        }
                    };
                    OutputRelocation {
                        offset: output_reloc_offset,
                        size: input_reloc.size(),
                        kind: RelocationKind::Absolute,
                        encoding: input_reloc.encoding(),
                        symbol: cls_output.output_section_symbol,
                        addend: output_addend as i64,
                    }
                }

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

impl ElfPass for ConvertRelocationPass {
    const NAME: &'static str = "convert relocations";

    type Output<'a> = ();
    type Error = ConvertRelocationError;

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
        match input.architecture() {
            Architecture::X86_64 => {
                self.convert_x86_64_relocations(ctx, input, output)?;
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
