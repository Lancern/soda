use std::borrow::Cow;

use object::elf::PT_LOAD;
use object::read::elf::{
    ElfFile, ElfSection, ElfSegment, FileHeader as ElfFileHeader, ProgramHeader as _,
};
use object::read::Error as ReadError;
use object::write::Object as OutputObject;
use object::{Object, ObjectSection, ObjectSegment, ReadRef, SectionIndex};

use crate::elf::pass::ElfPass;
use crate::pass::PassContext;

/// A pass that loads the input shared library into memory.
///
/// This pass will produce a memory mapping of the sections in the input shared library.
#[derive(Debug, Default)]
pub struct LoaderPass;

impl ElfPass for LoaderPass {
    const NAME: &'static str = "loader";

    type Output<'d> = LoadedDso<'d>;
    type Error = ReadError;

    fn run<'d, E, R>(
        &mut self,
        _ctx: &PassContext<'d>,
        input: &ElfFile<'d, E, R>,
        _output: &mut OutputObject<'d>,
    ) -> Result<Self::Output<'d>, Self::Error>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>,
    {
        let mut ret = LoadedDso {
            loaded_sections: Vec::new(),
        };

        let endian = input.endian();
        for (seg_header, seg) in input.raw_segments().iter().zip(input.segments()) {
            let seg_type = seg_header.p_type(endian);
            if seg_type != PT_LOAD {
                continue;
            }

            // Enumerate all sections in the current segment.
            for sec in input.sections() {
                if !is_section_in_segment(&sec, &seg) {
                    continue;
                }

                let sec_data = sec.uncompressed_data()?;
                ret.loaded_sections.push(LoadedSection {
                    index: sec.index(),
                    virt_addr: sec.address(),
                    mem_size: sec.size(),
                    alignment: sec.align(),
                    data: sec_data,
                });
            }
        }

        ret.loaded_sections.sort_by_key(|sec| sec.virt_addr);

        Ok(ret)
    }
}

#[derive(Debug)]
pub struct LoadedDso<'d> {
    pub loaded_sections: Vec<LoadedSection<'d>>,
}

#[derive(Debug)]
pub struct LoadedSection<'d> {
    pub index: SectionIndex,
    pub virt_addr: u64,
    pub mem_size: u64,
    pub alignment: u64,
    pub data: Cow<'d, [u8]>,
}

fn is_section_in_segment<'d, 'f, E, R>(
    section: &ElfSection<'d, 'f, E, R>,
    segment: &ElfSegment<'d, 'f, E, R>,
) -> bool
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    let sec_addr = section.address();
    let seg_addr = segment.address();

    let sec_size = section.size();
    let seg_size = segment.size();

    let sec_end_addr = sec_addr + sec_size;
    let seg_end_addr = seg_addr + seg_size;

    sec_addr >= seg_addr && sec_end_addr <= seg_end_addr
}
