use std::collections::HashMap;

use object::elf::{PT_LOAD, SHT_PROGBITS};
use object::read::elf::{
    ElfFile, ElfSection, ElfSegment, FileHeader as ElfFileHeader, ProgramHeader as _,
};
use object::read::Error as ReadError;
use object::write::{Object as OutputObject, SectionId};
use object::{Object, ObjectSection, ObjectSegment, ReadRef, SectionIndex, SectionKind};

use crate::elf::pass::ElfPass;
use crate::pass::PassContext;

/// A pass that copies loadable sections in the input shared library into the output relocatable object.
///
/// All such input sections will be copied into the same section in the output relocatable object so that internal
/// references won't break in further linking.
#[derive(Debug, Default)]
pub struct CopyLodableSectionsPass;

impl ElfPass for CopyLodableSectionsPass {
    const NAME: &'static str = "copy sections";

    type Output<'d> = CopyLodableSectionsOutput;
    type Error = ReadError;

    fn run<'d, E, R>(
        &mut self,
        _ctx: &PassContext<'d>,
        input: &ElfFile<'d, E, R>,
        output: &mut OutputObject<'d>,
    ) -> Result<Self::Output<'d>, Self::Error>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>,
    {
        let output_sec_id = output.add_section(Vec::new(), todo!(), SectionKind::Elf(SHT_PROGBITS));
        let mut ret = CopyLodableSectionsOutput::new(output_sec_id);

        // First we copy all loadable sections to output.
        let endian = input.endian();
        for (seg_header, seg) in input.raw_segments().iter().zip(input.segments()) {
            let seg_type = seg_header.p_type(endian);
            if seg_type != PT_LOAD {
                // Skip sections in non-loadable segments.
                continue;
            }

            // Enumerate all sections in the current segment which is loadable.
            for input_sec in input.sections() {
                if !is_section_in_segment(&input_sec, &seg) {
                    continue;
                }

                todo!()
            }
        }

        todo!()
    }
}

#[derive(Debug)]
pub struct CopyLodableSectionsOutput {
    /// The section ID of the output section.
    pub output_section_id: SectionId,

    /// Describe the relative offset of each input section within the output section.
    pub input_section_offsets: HashMap<SectionIndex, u64>,
}

impl CopyLodableSectionsOutput {
    fn new(output_section_id: SectionId) -> Self {
        Self {
            output_section_id,
            input_section_offsets: HashMap::new(),
        }
    }
}

fn is_section_in_segment<'d, 'f, E, R>(
    sec: &ElfSection<'d, 'f, E, R>,
    seg: &ElfSegment<'d, 'f, E, R>,
) -> bool
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    let sec_addr = sec.address();
    let seg_addr = seg.address();

    let sec_size = sec.size();
    let seg_size = seg.size();

    let sec_end_addr = sec_addr + sec_size;
    let seg_end_addr = seg_addr + seg_size;

    sec_addr >= seg_addr && sec_end_addr <= seg_end_addr
}
