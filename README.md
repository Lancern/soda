# soda

Convert shared libraries into static libraries.

> [!NOTE]
> This project is still under early development phase.

## Usage

```bash
soda /path/to/your/libfoo.so
```

You can specify `-o` to change the output file name. If omitted, the default
output file name will be `foo.o` if the input shared library is named
`libfoo.so`.

## Build

You need the latest stable Rust toolchain to build `soda`. Refer to [rustup] if
you don't have a Rust toolchain yet.

[rustup]: https://rustup.rs/

Clone this repository and build:

```bash
git clone https://github.com/Lancern/soda.git
cd soda
cargo build
```

If you want to build a release version:

```bash
cargo build --release
```

## Contribution

We welcome any form of contributions to this project:

- Create a new [issue] for _bug reports_ and _feature request_.
- Create a new [PR] for _bug fixes_ and _feature implementations_.
- Create a new [discussion] if you have anything to share and discuss, or if you
  meet any problems in the usage of this tool.

[issue]: https://github.com/Lancern/soda/issues
[PR]: https://github.com/Lancern/soda/pulls
[discussion]: https://github.com/Lancern/soda/discussions

## License

This project is open-sourced under [MIT License](./LICENSE).
