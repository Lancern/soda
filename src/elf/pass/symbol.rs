use std::collections::HashMap;

use object::read::elf::{ElfFile, ElfSymbol, FileHeader as ElfFileHeader};
use object::read::Error as ReadError;
use object::write::{Symbol as OutputSymbol, SymbolId, SymbolSection as OutputSymbolSection};
use object::{Object, ObjectSymbol, ReadRef, SymbolFlags, SymbolIndex, SymbolSection};

use crate::elf::pass::section::{CopyLodableSectionsOutput, CopyLodableSectionsPass};
use crate::pass::{Pass, PassContext, PassHandle};

/// A pass that generates the symbol table of the output relocatable file.
///
/// This pass generates the symbol table based on the dynamic symbols of the input shared library. Specifically, for
/// each dynamic symbol in the input shared library whose containing section is included in the output relocatable file,
/// a corresponding symbol will be generated in the output relocatable file's symbol table:
///
/// - Undefined input symbol will generate a corresponding undefined output symbol;
/// - Defined local symbol will generate a corresponding defined local symbol;
/// - Defined external symbol will generate a corresponding defined external symbol.
///
/// This pass will produce a symbol map that maps input dynamic symbols to output symbols.
#[derive(Debug)]
pub struct GenerateSymbolPass {
    pub cls_pass: PassHandle<CopyLodableSectionsPass>,
}

impl<'d, E, R> Pass<ElfFile<'d, E, R>> for GenerateSymbolPass
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    const NAME: &'static str = "generate symbols";

    type Output = SymbolMap;
    type Error = ReadError;

    fn run(&mut self, ctx: &PassContext<ElfFile<'d, E, R>>) -> Result<Self::Output, Self::Error>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>,
    {
        let mut output = ctx.output.borrow_mut();

        let cls_output = ctx.get_pass_output(self.cls_pass);

        let mut sym_map = HashMap::new();
        for input_sym in ctx.input.dynamic_symbols() {
            // Ensure that the section containing the symbol has been copied into the output relocatable file. If not,
            // such symbols will not cause the generation of an output symbol.
            if let Some(sym_section_idx) = input_sym.section_index() {
                if !cls_output.is_section_copied(sym_section_idx) {
                    continue;
                }
            }

            let output_sym = create_output_symbol(&input_sym, cls_output)?;
            let output_sym_id = output.add_symbol(output_sym);
            sym_map.insert(input_sym.index(), output_sym_id);
        }

        Ok(SymbolMap(sym_map))
    }
}

#[derive(Debug)]
pub struct SymbolMap(HashMap<SymbolIndex, SymbolId>);

impl SymbolMap {
    /// Get the output symbol corresponding to the specified input symbol.
    pub fn get_output_symbol(&self, input_sym: SymbolIndex) -> Option<SymbolId> {
        self.0.get(&input_sym).copied()
    }
}

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
        SymbolSection::Common => OutputSymbolSection::Common,
        SymbolSection::Section(sec_idx) => {
            assert!(copied_sections.is_section_copied(sec_idx));
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
