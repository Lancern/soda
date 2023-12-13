use std::ops::Range;

use object::elf::{PT_LOAD, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, SHT_PROGBITS};
use object::read::elf::{
    ElfFile, ElfSection, ElfSegment, FileHeader as ElfFileHeader, ProgramHeader as _,
};
use object::read::Error as ReadError;
use object::write::{SectionId, SymbolId};
use object::{
    Object, ObjectSection, ObjectSegment, ReadRef, SectionFlags, SectionIndex, SectionKind,
};

use crate::pass::{Pass, PassContext};

/// A pass that copies loadable sections in the input shared library into the output relocatable object.
///
/// All such input sections will be copied into the same section in the output relocatable object so that internal
/// references won't break in further linking.
#[derive(Debug, Default)]
pub struct CopyLodableSectionsPass;

impl<'d, E, R> Pass<ElfFile<'d, E, R>> for CopyLodableSectionsPass
where
    E: ElfFileHeader,
    R: ReadRef<'d>,
{
    const NAME: &'static str = "copy sections";

    type Output = CopyLodableSectionsOutput;
    type Error = ReadError;

    fn run(&mut self, ctx: &PassContext<ElfFile<'d, E, R>>) -> Result<Self::Output, Self::Error>
    where
        E: ElfFileHeader,
        R: ReadRef<'d>,
    {
        let mut output = ctx.output.borrow_mut();

        // TODO: make the output section's name customizable.
        let output_sec_id = output.add_section(
            Vec::new(),
            "soda".as_bytes().to_vec(),
            SectionKind::Elf(SHT_PROGBITS),
        );
        let output_sec_sym = output.section_symbol(output_sec_id);
        let output_sec = output.section_mut(output_sec_id);

        let mut ret = CopyLodableSectionsOutput::new(output_sec_id, output_sec_sym);

        // First we collect all loadable sections.
        let input_sections = collect_loadable_sections(&ctx.input);
        if input_sections.is_empty() {
            return Ok(ret);
        }

        output_sec.flags = get_output_section_flags(&input_sections);

        // Copy the data of the collected input sections to the output section.
        // First calculate the size and alignment of the output section, together with the offset of each input section
        // in the output section.
        let mut output_sec_size = 0u64;
        for input_sec in &input_sections {
            let input_sec_name = String::from_utf8_lossy(input_sec.name_bytes()?);

            let input_sec_addr = input_sec.address();
            let input_sec_size = input_sec.size();
            let input_sec_align = input_sec.align();

            if input_sec_addr < output_sec_size {
                log::warn!("Overlapping section \"{}\"", input_sec_name);
            }
            if input_sec_addr % input_sec_align != 0 {
                log::warn!("Unaligned input section \"{}\"", input_sec_name);
            }

            let input_sec_end = input_sec_addr.checked_add(input_sec_size).unwrap();
            output_sec_size = input_sec_end;
            ret.input_section_ranges.push(SectionMap {
                index: input_sec.index(),
                input_addr_range: input_sec_addr..input_sec_end,
                output_addr_range: input_sec_addr..input_sec_end,
            });
        }

        assert!(output_sec_size <= std::usize::MAX as u64);

        // Calculate the alignment of the output section.
        let output_sec_align = input_sections.iter().map(|sec| sec.align()).max().unwrap();

        // Then do the data copy.
        let mut output_buffer = vec![0u8; output_sec_size as usize];
        for input_sec in &input_sections {
            let sec_data = input_sec.uncompressed_data()?;
            assert_eq!(sec_data.len(), input_sec.size() as usize);

            let input_sec_addr = input_sec.address();
            let input_sec_size = input_sec.size();
            let output_range = input_sec_addr as usize..(input_sec_addr + input_sec_size) as usize;

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

    /// The ID of the output section symbol.
    pub output_section_symbol: SymbolId,

    /// Gives the information about copied sections.
    pub input_section_ranges: Vec<SectionMap>,
}

impl CopyLodableSectionsOutput {
    /// Determine whether the specified input section is copied into the output section.
    pub fn is_section_copied(&self, idx: SectionIndex) -> bool {
        self.get_section_map(idx).is_some()
    }

    /// Given an input address, get the offset of the corresponding location in the output section.
    pub fn map_input_addr(&self, input_addr: u64) -> Option<u64> {
        self.get_section_map_by_addr(input_addr).map(|map| {
            let section_offset = input_addr - map.input_addr_range.start;
            map.output_addr_range.start + section_offset
        })
    }

    fn new(output_section_id: SectionId, output_section_symbol: SymbolId) -> Self {
        Self {
            output_section_id,
            output_section_symbol,
            input_section_ranges: Vec::new(),
        }
    }

    fn get_section_map(&self, section_idx: SectionIndex) -> Option<&SectionMap> {
        self.input_section_ranges
            .iter()
            .find(|map| map.index == section_idx)
    }

    fn get_section_map_by_addr(&self, addr: u64) -> Option<&SectionMap> {
        self.input_section_ranges
            .iter()
            .find(|map| map.input_addr_range.contains(&addr))
    }
}

#[derive(Clone, Debug)]
pub struct SectionMap {
    pub index: SectionIndex,
    pub input_addr_range: Range<u64>,
    pub output_addr_range: Range<u64>,
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
