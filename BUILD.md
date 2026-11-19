# Building and Installing AMO VS

AMO VS is a cross-platform terminal code editor written in Rust. This guide
covers prerequisites, building from source, installing the resulting binary and
running the test suite.

## Prerequisites

| Requirement | Notes |
| --- | --- |
| Rust 1.74+ (edition 2021) | Install via [rustup](https://rustup.rs/) |
| A C compiler | Needed for `tree-sitter` grammars and `libgit2` (`git2`) |
| CMake | Required by some native build scripts |
| A UTF-8 terminal | Nerd Fonts are optional; shipped icons are single-column |

Platform-specific notes:

- **Linux:** install `build-essential`, `cmake` and `pkg-config`
  (e.g. `sudo apt install build-essential cmake pkg-config`).
- **macOS:** install the Xcode command line tools (`xcode-select --install`)
  and `cmake` (e.g. `brew install cmake`).
- **Windows:** use MSVC build tools (Visual Studio Build Tools with "C++ build
  tools") or MinGW-w64; install CMake from <https://cmake.org/>.

Verify your toolchain:

```bash
rustc --version
cargo --version
cc --version
cmake --version
```

## Building from source

Clone the repository and enter the project directory:

```bash
git clone https://github.com/OMIDPYTHON3/amo-vs.git
cd amo-vs
```

### Linux / macOS

Use the provided build script:

```bash
./build.sh release        # release build -> dist/amo-vs
./build.sh debug          # debug build   -> dist/amo-vs
./build.sh all            # cross-compile all configured targets
```

Or build directly with Cargo:

```bash
cargo build --release     # binary at target/release/amo-vs
cargo build               # binary at target/debug/amo-vs
```

### Windows (PowerShell)

```powershell
.\build.ps1 -Configuration Release   # -> dist\amo-vs.exe
.\build.ps1 -Configuration Debug     # -> dist\amo-vs.exe (debug)
```

Or with Cargo:

```powershell
cargo build --release                # binary at target\release\amo-vs.exe
```

### Cross-compiling

`./build.sh all` builds these targets when the corresponding rustup target is
installed:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `aarch64-linux-android`
- `x86_64-pc-windows-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`

Missing targets are skipped. Install them first with:

```bash
rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
```

Cross-compilation to Apple/Windows targets from Linux additionally needs the
appropriate toolchain/linker (e.g. `mingw-w64` for the GNU Windows target).

## Running the tests

```bash
cargo test
```

Integration tests live under `tests/`; unit tests are colocated with the
modules in `src/`.

## Installing

### From source (all platforms)

After building, copy the binary to a directory on your `PATH`:

```bash
sudo install -m 755 dist/amo-vs /usr/local/bin/amo-vs
```

Or, without root privileges, install into your user bin directory:

```bash
mkdir -p ~/.local/bin
install -m 755 dist/amo-vs ~/.local/bin/amo-vs
# ensure ~/.local/bin is on your PATH
```

On Windows, create a folder such as `%USERPROFILE%\Bin`, copy
`dist\amo-vs.exe` into it, and add that folder to your `PATH`.

### Installing via Cargo (from a checkout)

```bash
cargo install --path .
```

This compiles in release mode and installs `amo-vs` into `~/.cargo/bin`
(`%USERPROFILE%\.cargo\bin` on Windows).

## Running

```bash
amo-vs                       # empty workspace
amo-vs /path/to/project      # open a folder
amo-vs src/main.rs           # open a file
amo-vs --no-mouse            # disable mouse capture
```

Configuration is created on first run in your platform config directory:
`~/.config/amo-vs` (Linux), `~/Library/Application Support/amo-vs` (macOS) or
`%APPDATA%\amo-vs` (Windows). A commented `settings.toml` is generated there;
custom themes may be placed in `<config dir>/themes/<name>.toml`.

## Uninstalling

```bash
sudo rm -f /usr/local/bin/amo-vs       # or: rm -f ~/.local/bin/amo-vs
# if installed via cargo:
cargo uninstall amo-vs
```

On Windows, delete the copied `amo-vs.exe` and remove the folder from `PATH`.

## Troubleshooting

- **CMake / C compiler errors:** install the prerequisites listed above and
  retry `cargo build`.
- **Linker errors for `libgit2`:** ensure a C compiler and CMake are on your
  `PATH`, then run `cargo clean` and rebuild.
- **Missing cross targets:** run `rustup target add <target>` for each target
  you want to build (see `./build.sh all` output for the exact names).
