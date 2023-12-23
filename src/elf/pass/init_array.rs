use object::elf::{R_X86_64_RELATIVE, SHT_FINI_ARRAY, SHT_INIT_ARRAY};
use object::read::elf::{ElfFile, FileHeader as ElfFileHeader};
use object::write::{Relocation as OutputRelocation, SymbolId};
use object::{
    Architecture, Object as _, ObjectSection as _, ReadRef, Relocation, RelocationKind, SectionKind,
};
use thiserror::Error;

use crate::elf::pass::section::CopyLodableSectionsPass;
use crate::pass::{Pass, PassContext, PassHandle};

/// Generate a .init_array section in the output relocatable file.
#[derive(Debug)]
pub struct GenerateInitArrayPass {
    inner: GenerateFuncPtrArray,
}

impl GenerateInitArrayPass {
    pub fn new(cls_pass: PassHandle<CopyLodableSectionsPass>) -> Self {
        Self {
            inner: GenerateFuncPtrArray::new(cls_pass),
        }
    }
}

impl<'d, E, R> Pass<ElfFile<'d, E, R>> for GenerateInitArrayPass
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    const NAME: &'static str = "generate init array";

    type Output = ();
    type Error = GenerateInitFiniArrayError;

    fn run(&mut self, ctx: &PassContext<ElfFile<'d, E, R>>) -> Result<Self::Output, Self::Error> {
        self.inner.generate(ctx, SHT_INIT_ARRAY)
    }
}

/// Generate a .fini_array section in the output relocatable file.
#[derive(Debug)]
pub struct GenerateFiniArrayPass {
    inner: GenerateFuncPtrArray,
}

impl GenerateFiniArrayPass {
    pub fn new(cls_pass: PassHandle<CopyLodableSectionsPass>) -> Self {
        Self {
            inner: GenerateFuncPtrArray::new(cls_pass),
        }
    }
}

impl<'d, E, R> Pass<ElfFile<'d, E, R>> for GenerateFiniArrayPass
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    const NAME: &'static str = "generate fini array";

    type Output = ();
    type Error = GenerateInitFiniArrayError;

    fn run(&mut self, ctx: &PassContext<ElfFile<'d, E, R>>) -> Result<Self::Output, Self::Error> {
        self.inner.generate(ctx, SHT_FINI_ARRAY)
    }
}

#[derive(Debug, Error)]
pub enum GenerateInitFiniArrayError {
    #[error("unsupported architecture: {0:?}")]
    UnsupportedArch(Architecture),

    #[error("unsupported reloc: {0:?}")]
    UnsupportedReloc(RelocationKind),
}

#[derive(Debug)]
struct GenerateFuncPtrArray {
    cls_pass: PassHandle<CopyLodableSectionsPass>,
}

impl GenerateFuncPtrArray {
    fn new(cls_pass: PassHandle<CopyLodableSectionsPass>) -> Self {
        Self { cls_pass }
    }

    fn generate<'d, E, R>(
        &self,
        ctx: &PassContext<ElfFile<'d, E, R>>,
        sec_type: u32,
    ) -> Result<(), GenerateInitFiniArrayError>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>,
    {
        assert!(sec_type == SHT_INIT_ARRAY || sec_type == SHT_FINI_ARRAY);
        let output_sec_name = match sec_type {
            SHT_INIT_ARRAY => ".init_array",
            SHT_FINI_ARRAY => ".fini_array",
            _ => unreachable!(),
        };

        let cls_output = ctx.get_pass_output(self.cls_pass);

        let input_sections = ctx
            .input
            .sections()
            .filter(|sec| sec.kind() == SectionKind::Elf(sec_type));

        let arch = ctx.input.architecture();
        let mut output_sec_size = 0;
        let mut output_relocs = Vec::new();

        for input_sec in input_sections {
            if !cls_output.is_section_copied(input_sec.index()) {
                continue;
            }

            let input_sec_size = input_sec.size();
            if input_sec_size == 0 {
                continue;
            }

            output_sec_size += input_sec_size;

            let input_sec_addr = input_sec.address();
            let input_sec_addr_range = input_sec_addr..input_sec_addr + input_sec_size;

            // Find all input relocations associated with the input section and convert them to corresponding output
            // relocations associated with the output section.

            for (input_reloc_addr, input_reloc) in ctx.input.dynamic_relocations().unwrap() {
                if !input_sec_addr_range.contains(&input_reloc_addr) {
                    continue;
                }

                let output_reloc = convert_init_fini_array_reloc(
                    arch,
                    input_reloc_addr,
                    &input_reloc,
                    cls_output.output_section_symbol,
                )?;
                output_relocs.push(output_reloc);
            }
        }

        if output_sec_size == 0 {
            return Ok(());
        }

        let mut output = ctx.output.borrow_mut();
        let output_sec_id = output.add_section(
            Vec::new(),
            output_sec_name.as_bytes().to_vec(),
            SectionKind::Elf(sec_type),
        );

        const INIT_FINI_ARRAY_ALIGN: u64 = 8;
        output.set_section_data(
            output_sec_id,
            vec![0u8; output_sec_size as usize],
            INIT_FINI_ARRAY_ALIGN,
        );

        for r in output_relocs {
            output.add_relocation(output_sec_id, r).unwrap();
        }

        Ok(())
    }
}

fn convert_init_fini_array_reloc(
    arch: Architecture,
    input_reloc_addr: u64,
    input_reloc: &Relocation,
    output_main_sec_sym: SymbolId,
) -> Result<OutputRelocation, GenerateInitFiniArrayError> {
    match arch {
        Architecture::X86_64 => {
            convert_init_fini_array_reloc_x86_64(input_reloc_addr, input_reloc, output_main_sec_sym)
        }
        arch => Err(GenerateInitFiniArrayError::UnsupportedArch(arch)),
    }
}

fn convert_init_fini_array_reloc_x86_64(
    input_reloc_addr: u64,
    input_reloc: &Relocation,
    output_main_sec_sym: SymbolId,
) -> Result<OutputRelocation, GenerateInitFiniArrayError> {
    let output_reloc = match input_reloc.kind() {
        RelocationKind::Elf(R_X86_64_RELATIVE) => OutputRelocation {
            offset: input_reloc_addr,
            size: 64,
            kind: RelocationKind::Absolute,
            encoding: input_reloc.encoding(),
            symbol: output_main_sec_sym,
            addend: input_reloc.addend(),
        },
        kind => {
            return Err(GenerateInitFiniArrayError::UnsupportedReloc(kind));
        }
    };
    Ok(output_reloc)
}
