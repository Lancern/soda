use object::{Architecture, BinaryFormat};

/// Get the string representation of a `BinaryFormat` value.
pub fn binary_format_to_str(f: BinaryFormat) -> &'static str {
    match f {
        BinaryFormat::Coff => "coff",
        BinaryFormat::Elf => "elf",
        BinaryFormat::MachO => "macho",
        BinaryFormat::Pe => "pe",
        BinaryFormat::Wasm => "wasm",
        BinaryFormat::Xcoff => "xcoff",
        _ => unreachable!(),
    }
}

/// Get the string representation of an `Architecture` value.
pub fn arch_to_str(arch: Architecture) -> &'static str {
    match arch {
        Architecture::Unknown => "unknown",
        Architecture::Aarch64 => "aarch64",
        Architecture::Aarch64_Ilp32 => "aarch64_ilp32",
        Architecture::Arm => "arm",
        Architecture::Avr => "avr",
        Architecture::Bpf => "bpf",
        Architecture::Csky => "csky",
        Architecture::I386 => "i386",
        Architecture::X86_64 => "x86_64",
        Architecture::X86_64_X32 => "x32",
        Architecture::Hexagon => "hexagon",
        Architecture::LoongArch64 => "loongarch64",
        Architecture::Mips => "mips",
        Architecture::Mips64 => "mips64",
        Architecture::Msp430 => "msp430",
        Architecture::PowerPc => "powerpc",
        Architecture::PowerPc64 => "powerpc64",
        Architecture::Riscv32 => "riscv32",
        Architecture::Riscv64 => "riscv64",
        Architecture::S390x => "s390x",
        Architecture::Sbf => "sbf",
        Architecture::Sparc64 => "sparc64",
        Architecture::Wasm32 => "wasm32",
        Architecture::Wasm64 => "wasm64",
        Architecture::Xtensa => "xtensa",
        _ => unreachable!(),
    }
}
