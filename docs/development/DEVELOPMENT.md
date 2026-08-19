# Development environment

## Supported toolchain contract

The repository declarations are the exact authority for tools whose versions
must match across developers and CI:

| Tool | Contract | Declaration |
| --- | --- | --- |
| Node.js | Exact `24.16.0` | `.node-version` |
| pnpm | Exact `11.19.0` | Root `packageManager` |
| Rust | Exact stable `1.97.1`, `minimal` profile, `rustfmt` and `clippy` | `rust-toolchain.toml` |

The following are minimum supported versions. A newer compatible version is
acceptable, although the doctor reports a warning when it differs from the
locally validated value:

| Tool | Minimum supported | Locally validated on macOS arm64 |
| --- | --- | --- |
| CMake | `3.25.0` | `4.4.2` |
| Ninja | `1.10.0` | `1.13.2` |
| Git | `2.40.0` | `2.50.1` |
| Xcode (macOS only) | `15.0` | `26.6` |
| Visual Studio/MSVC (Windows only) | Visual Studio 2022 with MSVC v143, compiler `19.38` or newer | Not yet validated by a Windows runner |

`CMakeLists.txt` and `CMakePresets.json` both declare CMake `3.25.0` as the
project minimum. The validated versions document one known-good machine; they
are not additional exact pins. Windows remains an acceptance gate until it is
verified on a real Windows x64 runner.

## macOS setup

1. Install the exact Node release from the official [Node.js download
   page](https://nodejs.org/en/download), then verify `node --version` prints
   `v24.16.0`.
2. Enable the Corepack bundled with that Node release with `corepack enable`.
   Corepack honors the repository's `packageManager` field; see the official
   [Node.js Corepack documentation](https://nodejs.org/api/corepack.html).
3. Install Rust through the official [rustup installer](https://rustup.rs/),
   then run `rustup toolchain install 1.97.1 --profile minimal` and
   `rustup component add rustfmt clippy --toolchain 1.97.1`.
4. Install CMake from the official [CMake download page](https://cmake.org/download/)
   and Ninja from the official [Ninja releases](https://github.com/ninja-build/ninja/releases).
5. Install Xcode from the official [Apple Xcode page](https://developer.apple.com/xcode/).
   The Xcode command-line tools must be available to `xcodebuild`.
6. Install or update Git from the official [Git downloads page](https://git-scm.com/downloads)
   if Xcode's supplied Git does not meet the supported minimum.

## Windows setup

1. Install the exact Node release from the official [Node.js download
   page](https://nodejs.org/en/download) and run `corepack enable` in a new
   terminal. Corepack uses `packageManager` to select pnpm `11.19.0`.
2. Install Rust with the official [rustup installer](https://rustup.rs/), then
   run `rustup toolchain install 1.97.1 --profile minimal` and
   `rustup component add rustfmt clippy --toolchain 1.97.1`.
3. Install the official [CMake Windows installer](https://cmake.org/download/),
   [Ninja release](https://github.com/ninja-build/ninja/releases), and
   [Git for Windows](https://git-scm.com/download/win).
4. Install [Visual Studio 2022](https://visualstudio.microsoft.com/vs/) with
   the **Desktop development with C++** workload, MSVC v143 build tools, and a
   Windows SDK. Run build commands from a Visual Studio developer prompt so
   `cl` is available on `PATH`.

## Diagnose, build, and test

From the repository root, use the standalone Node.js doctor before building:

```sh
node tools/scripts/doctor.mjs
node tools/scripts/doctor.mjs --json
```

This standalone form is the doctor bootstrap guarantee: it requires Node.js
only and cannot cause pnpm/Corepack to run before the doctor starts. The root
`pnpm run doctor` script is a convenience command only after pnpm is already
available; do not use it to diagnose a fresh machine because the pnpm shim
executes before the script can enforce its read-only rules.

The doctor only reads repository declarations and invokes version probes. Its
pnpm probe sets `COREPACK_ENABLE_NETWORK=0`, so a Corepack shim cannot fetch a
missing package-manager release. Its Rust probes set `RUSTUP_AUTO_INSTALL=0`,
resolve `rustup which --toolchain <pinned-version> <binary>`, then invoke the
returned installed binary directly rather than a repository-selecting Rustup
proxy. It never installs or upgrades tools, writes configuration, changes
`PATH`, invokes a package manager to modify state, or requires Python. It
emits `PASS`, `WARN`, `FAIL`, and `SKIP` records; a missing required tool or
incompatible required version exits non-zero. Xcode is checked only on macOS
and MSVC only on Windows.

Install JavaScript workspace dependencies using the pinned package manager:

```sh
pnpm install --frozen-lockfile
```

Run the current repository checks:

```sh
pnpm test:doctor
cargo +1.97.1 fmt --all -- --check
cargo +1.97.1 check --workspace
cmake --preset native-debug
cmake --build --preset native-debug
ctest --preset native-debug
```

Tauri CLI is intentionally not a global prerequisite. When the desktop shell
is introduced, its CLI will be a versioned repository dependency and invoked
through the package manager.
