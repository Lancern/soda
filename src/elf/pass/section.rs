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

        let mut ret = CopyLodableSectionsOutput {
            output_section_id: output_sec_id,
            output_section_symbol: output_sec_sym,
            output_section_size: 0,
            section_maps: Vec::new(),
        };

        // First we collect all loadable sections. The returned section list is sorted by their base addresses.
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
                log::warn!(
                    "Overlapping section \"{}\" (section index {})",
                    input_sec_name,
                    input_sec.index().0
                );
            }
            if input_sec_align != 0 && input_sec_addr % input_sec_align != 0 {
                log::warn!(
                    "Unaligned input section \"{}\" (section index {})",
                    input_sec_name,
                    input_sec.index().0
                );
            }

            let input_sec_end = input_sec_addr.checked_add(input_sec_size).unwrap();
            output_sec_size = input_sec_end;
            ret.section_maps.push(SectionMap {
                index: input_sec.index(),
                addr_range: input_sec_addr..input_sec_end,
            });
        }

        assert!(output_sec_size <= std::usize::MAX as u64);
        ret.output_section_size = output_sec_size;

        // Calculate the alignment of the output section.
        let output_sec_align = input_sections.iter().map(|sec| sec.align()).max().unwrap();

        // Then do the data copy.
        let mut output_buffer = vec![0u8; output_sec_size as usize];
        for input_sec in &input_sections {
            let sec_data = input_sec.uncompressed_data()?;
            assert!(sec_data.len() <= input_sec.size() as usize);

            if sec_data.is_empty() {
                continue;
            }

            let input_sec_addr = input_sec.address();
            let output_range = input_sec_addr as usize..input_sec_addr as usize + sec_data.len();

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
            // We don't deal with the UND section. (i.e. the section at index 0)
            if input_sec.index().0 == 0 {
                continue;
            }

            if input_sec.address() == 0 {
                // Input sections whose address is 0 are not included in the memory image.
                continue;
            }

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

    /// Size of the output section.
    pub output_section_size: u64,

    /// Gives the information about copied sections.
    pub section_maps: Vec<SectionMap>,
}

impl CopyLodableSectionsOutput {
    /// Determine whether the specified input section is copied into the output section.
    pub fn is_section_copied(&self, idx: SectionIndex) -> bool {
        self.get_section_map(idx).is_some()
    }

    fn get_section_map(&self, section_idx: SectionIndex) -> Option<&SectionMap> {
        self.section_maps
            .iter()
            .find(|map| map.index == section_idx)
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct SectionMap {
    pub index: SectionIndex,
    pub addr_range: Range<u64>,
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

#[cfg(test)]
mod test {
    use std::ops::Range;

    use object::read::elf::ElfFile64;
    use object::read::SectionIndex;
    use object::write::Object as OutputObject;
    use object::{Architecture, BinaryFormat, Endianness};

    use crate::pass::test::PassTest;
    use crate::pass::{Pass, PassHandle, PassManager};

    use super::{CopyLodableSectionsPass, SectionMap};

    struct CopyLoadableSectionPassTest;

    impl PassTest for CopyLoadableSectionPassTest {
        type Input = ElfFile64<'static>;
        type Pass = CopyLodableSectionsPass;

        fn setup(&mut self, pass_mgr: &mut PassManager<Self::Input>) -> PassHandle<Self::Pass> {
            pass_mgr.add_pass_default::<CopyLodableSectionsPass>()
        }

        fn check_pass_output(&mut self, output: &<Self::Pass as Pass<Self::Input>>::Output) {
            fn addr_range(addr: u64, size: u64) -> Range<u64> {
                addr..addr + size
            }

            macro_rules! make_section_maps {
                ( $( { $index:expr, $addr:expr, $size:expr $(,)? } ),* $(,)? ) => {
                    vec![
                        $(
                            SectionMap {
                                index: SectionIndex($index),
                                addr_range: addr_range($addr, $size),
                            }
                        ),*
                    ]
                };
            }

            assert_eq!(output.output_section_size, 0x95e28);
            assert_eq!(
                output.section_maps,
                make_section_maps! {
                    { 1, 0x2e0, 0x30 },
                    { 2, 0x310, 0x24 },
                    { 3, 0x338, 0x2910 },
                    { 4, 0x2c48, 0x8a48 },
                    { 5, 0xb690, 0x1cb3f },
                    { 6, 0x281d0, 0xb86 },
                    { 7, 0x28d58, 0x180 },
                    { 8, 0x28ed8, 0x7320 },
                    { 9, 0x301f8, 0x2280 },
                    { 10, 0x33000, 0x1b },
                    { 11, 0x33020, 0x1710 },
                    { 12, 0x34730, 0x28 },
                    { 13, 0x34760, 0x4a4a4 },
                    { 14, 0x7ec04, 0xd },
                    { 15, 0x7f000, 0x4d70 },
                    { 16, 0x83d70, 0x1b5c },
                    { 17, 0x858d0, 0x9804 },
                    { 18, 0x8f0d4, 0x2234 },
                    { 19, 0x92390, 0x10 },
                    { 20, 0x92390, 0x8 },
                    { 21, 0x92398, 0x8 },
                    { 22, 0x923a0, 0x2490 },
                    { 23, 0x94830, 0x210 },
                    { 24, 0x94a40, 0x598 },
                    { 25, 0x94fe8, 0xb98 },
                    { 26, 0x95b80, 0xa0 },
                    { 27, 0x95c20, 0x208 },
                }
            );
        }
    }

    #[test]
    fn test_cls_pass() {
        let input = crate::elf::test::get_test_input_file();
        let output = OutputObject::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);
        crate::pass::test::run_pass_test(CopyLoadableSectionPassTest, input, output);
    }
}
