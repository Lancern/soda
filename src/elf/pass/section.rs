use std::collections::HashMap;
use std::ops::Range;

use object::elf::{PT_LOAD, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, SHT_PROGBITS};
use object::read::elf::{
    ElfFile, ElfSection, ElfSegment, FileHeader as ElfFileHeader, ProgramHeader as _,
};
use object::read::Error as ReadError;
use object::write::{Object as OutputObject, SectionId};
use object::{
    Object, ObjectSection, ObjectSegment, ReadRef, SectionFlags, SectionIndex, SectionKind,
};

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
        // TODO: make the output section's name customizable.
        let output_sec_id = output.add_section(
            Vec::new(),
            "soda".as_bytes().to_vec(),
            SectionKind::Elf(SHT_PROGBITS),
        );
        let output_sec = output.section_mut(output_sec_id);

        let mut ret = CopyLodableSectionsOutput::new(output_sec_id);

        // First we collect all loadable sections.
        let input_sections = collect_loadable_sections(input);
        if input_sections.is_empty() {
            return Ok(ret);
        }

        output_sec.flags = get_output_section_flags(&input_sections);

        // Copy the data of the collected input sections to the output section.
        // First calculate the size and alignment of the output section, together with the offset of each input section
        // in the output section.
        let mut output_sec_size = 0u64;
        for input_sec in &input_sections {
            let input_sec_size = input_sec.size();

            output_sec_size = output_sec_size.next_multiple_of(input_sec.align());
            ret.input_section_ranges.insert(
                input_sec.index(),
                output_sec_size..output_sec_size + input_sec_size,
            );

            output_sec_size += input_sec_size;
        }

        assert!(output_sec_size <= std::usize::MAX as u64);

        let output_sec_align = input_sections.iter().map(|sec| sec.align()).max().unwrap();

        // Then do the data copy.
        let mut output_buffer = vec![0u8; output_sec_size as usize];
        for input_sec in &input_sections {
            let sec_data = input_sec.uncompressed_data()?;
            assert_eq!(sec_data.len(), input_sec.size() as usize);

            let output_range = ret.input_section_ranges.get(&input_sec.index()).unwrap();
            let output_range = output_range.start as usize..output_range.end as usize;

            let output_slice = &mut output_buffer[output_range];
            output_slice.copy_from_slice(&sec_data);
        }

        // Set the output section's data.
        output_sec.set_data(output_buffer, output_sec_align);

        Ok(ret)
    }
}

fn collect_loadable_sections<'d, 'f, E, R>(
    input: &'f ElfFile<'d, E, R>,
) -> Vec<ElfSection<'d, 'f, E, R>>
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    let endian = input.endian();
    let mut input_sections = Vec::new();

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

            input_sections.push(input_sec);
        }
    }

    input_sections.sort_by_key(|sec| sec.address());
    input_sections
}

fn get_output_section_flags<'d, 'f, E, R>(
    input_sections: &[ElfSection<'d, 'f, E, R>],
) -> SectionFlags
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    let mut writable = false;
    let mut executable = false;

    for input_sec in input_sections {
        let sec_flags = match input_sec.flags() {
            SectionFlags::Elf { sh_flags } => sh_flags,
            _ => unreachable!(),
        };
        writable |= sec_flags & SHF_WRITE as u64 != 0;
        executable |= sec_flags & SHF_EXECINSTR as u64 != 0;
    }

    let mut raw_flags = SHF_ALLOC;
    if writable {
        raw_flags |= SHF_WRITE;
    }
    if executable {
        raw_flags |= SHF_EXECINSTR;
    }

    SectionFlags::Elf {
        sh_flags: raw_flags as u64,
    }
}

#[derive(Debug)]
pub struct CopyLodableSectionsOutput {
    /// The section ID of the output section.
    pub output_section_id: SectionId,

    /// Describe the offset range of each input section within the output section.
    pub input_section_ranges: HashMap<SectionIndex, Range<u64>>,
}

impl CopyLodableSectionsOutput {
    /// Determine whether the specified input section is copied into the output section.
    pub fn is_input_section_copied(&self, idx: SectionIndex) -> bool {
        self.input_section_ranges.contains_key(&idx)
    }

    fn new(output_section_id: SectionId) -> Self {
        Self {
            output_section_id,
            input_section_ranges: HashMap::new(),
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
