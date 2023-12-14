use object::read::elf::ElfFile64;

pub fn get_test_input_file() -> ElfFile64<'static> {
    let file_data = include_bytes!("libspdlog.so.1.12.0").as_slice();
    ElfFile64::parse(file_data).unwrap()
}
