<img align="right" width="220" src="https://github.com/RelRashica/nosharp/raw/main/NoSharp_logo.png" alt="No# logo" style="background: transparent; border: none; box-shadow: none;" />

<h1 style="border-bottom: none; margin: 0;">No#</h1>

**No#** (`/ˈnoʊ.ʃɑːrp/`) is a small, C#-inspired programming language written in Rust.

It is a personal project built for learning, experimentation, and having fun with language development. No# is still in its early stages, and its features and implementation are actively evolving.

## Architecture

No# uses a compiler and runtime architecture built around WebAssembly.

- **Compiler (Rust):** Handles lexing, parsing, typechecking, and WebAssembly generation.
- **Runtime (C# / .NET):** The `dotnah` CLI loads and executes compiled WebAssembly modules using Wasmtime.
- **Host functions:** The runtime provides functions that No# programs can import and use.

## Project Layout

```text
nosharp/
├── src/                  # Rust compiler implementation
├── Cargo.toml            # Rust package configuration
└── Cargo.lock            # Dependency lockfile
```

The .NET runtime and CLI are maintained separately in the `dotnah` repository.

## How it works

When running a No# program:

1. `dotnah` reads the `.nos` source file.
2. The Rust compiler processes the source code.
3. The compiler generates a WebAssembly module.
4. Wasmtime loads and executes the generated module.

## Usage

Run a No# program using the `dotnah` CLI:

```powershell
dotnah run path/to/program.nos
```

## Technologies

- Rust
- C#
- .NET
- WebAssembly
- Wasmtime

## Project Status

No# is an experimental language under active development.

Features are limited, and breaking changes should be expected. It is not intended for production use.

## Related Projects

- [.NAH](https://github.com/Arian-ctrl/dotnah) - .NET runtime and CLI for No#.

## License

No# is distributed under the MIT License. See [LICENSE](LICENSE) for details.
