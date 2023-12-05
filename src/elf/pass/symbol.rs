use std::collections::HashMap;

use object::elf::{STV_DEFAULT, STV_PROTECTED};
use object::read::elf::{ElfFile, ElfSymbol, FileHeader as ElfFileHeader};
use object::read::Error as ReadError;
use object::write::{
    Object as OutputObject, Symbol as OutputSymbol, SymbolId, SymbolSection as OutputSymbolSection,
};
use object::{Object, ObjectSymbol, ReadRef, SymbolFlags, SymbolIndex, SymbolSection};

use crate::elf::pass::section::{CopyLodableSectionsOutput, CopyLodableSectionsPass};
use crate::elf::pass::{ElfPass, ElfPassHandle};
use crate::pass::PassContext;

/// A pass that generates the symbol table of the output relocatable file.
///
/// This pass generates the symbol table based on the dynamic symbols of the input shared library:
/// - For an undefined dynamic symbol in the input shared library, a corresponding undefined symbol will be generated in
///   the output relocatable file;
/// - For a defined dynamic symbol with default visibility (i.e. external linkage), a corresponding defined symbol will
///   be generated in the output relocatable file.
///
/// This pass will produce a symbol map that maps input dynamic symbols to output symbols.
#[derive(Debug)]
pub struct GenerateSymbolPass {
    copy_sections: ElfPassHandle<CopyLodableSectionsPass>,
}

impl GenerateSymbolPass {
    /// Create a new `GenerateSymbolPass`.
    pub fn new(copy_sections: ElfPassHandle<CopyLodableSectionsPass>) -> Self {
        Self { copy_sections }
    }
}

impl ElfPass for GenerateSymbolPass {
    const NAME: &'static str = "generate symbols";

    type Output<'a> = SymbolMap;
    type Error = ReadError;

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
        let copied_sections = ctx.get_pass_output(self.copy_sections);

        let mut sym_map = HashMap::new();
        for input_sym in input.dynamic_symbols() {
            if !input_sym.is_undefined() {
                // The symbol is a defined symbol. All undefined symbols will be added to the output relocatable file
                // unconditionally.

                if !input_sym.is_global() {
                    // Local defined symbols will not be included in the output relocatable file.
                    continue;
                }

                let sym_vis = match input_sym.flags() {
                    // The `st_other` field in the dynamic symbol entry gives the symbol's visibility.
                    SymbolFlags::Elf { st_other, .. } => st_other,
                    _ => unreachable!(),
                };
                if sym_vis != STV_DEFAULT && sym_vis != STV_PROTECTED {
                    // The symbol is a defined symbol that is not visible from outside of the input shared library.
                    // We don't include such symbol in the output relocatable file.
                    continue;
                }

                // Ensure that the section that contains the symbol has been copied into the output relocatable file.
                if let Some(sym_section_idx) = input_sym.section_index() {
                    if !copied_sections.is_input_section_copied(sym_section_idx) {
                        // Weired. The section that contains this defined symbol is not copied into the output
                        // relocatable file. For now, we just skip this symbol.
                        // TODO: maybe emit a warning here?
                        continue;
                    }
                }
            }

            let output_sym = create_output_symbol(&input_sym, copied_sections)?;
            let output_sym_id = output.add_symbol(output_sym);
            sym_map.insert(input_sym.index(), output_sym_id);
        }

        Ok(SymbolMap(sym_map))
    }
}

#[derive(Debug)]
pub struct SymbolMap(HashMap<SymbolIndex, SymbolId>);

fn create_output_symbol<'d, 'f, E, R>(
    input_sym: &ElfSymbol<'d, 'f, E, R>,
    copied_sections: &CopyLodableSectionsOutput,
) -> Result<OutputSymbol, ReadError>
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    let name = input_sym.name_bytes()?.to_vec();

    let section = match input_sym.section() {
        SymbolSection::None => OutputSymbolSection::None,
        SymbolSection::Undefined => OutputSymbolSection::Undefined,
        SymbolSection::Absolute => OutputSymbolSection::Absolute,
        SymbolSection::Common => OutputSymbolSection::None,
        SymbolSection::Section(sec_idx) => {
            assert!(copied_sections.is_input_section_copied(sec_idx));
            OutputSymbolSection::Section(copied_sections.output_section_id)
        }
        _ => unreachable!(),
    };

    let (st_info, st_other) = match input_sym.flags() {
        SymbolFlags::Elf { st_info, st_other } => (st_info, st_other),
        _ => unreachable!(),
    };

    Ok(OutputSymbol {
        name,
        value: input_sym.address(),
        size: input_sym.size(),
        kind: input_sym.kind(),
        scope: input_sym.scope(),
        weak: input_sym.is_weak(),
        section,
        flags: SymbolFlags::Elf { st_info, st_other },
    })
}
