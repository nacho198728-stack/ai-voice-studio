# Development environment

This guide describes the implemented Phase 0.5 repository. Run commands from
the repository root unless a section says otherwise. The macOS arm64 gate has
been exercised locally. The Windows x64 workflow exists, but real Windows
execution is pending and is not accepted until the workflow runs on GitHub.

## Toolchain contract

Repository declarations are authoritative for exact versions:

### Exact repository tools

| Tool | Required version | Authority |
| --- | --- | --- |
| Node.js | `24.16.0` | `.node-version` |
| pnpm | `11.19.0` | root `packageManager` |
| Rust | `1.97.1`, minimal profile with rustfmt and clippy | `rust-toolchain.toml` |
| Tauri CLI | `2.11.4` | `apps/desktop/package.json` |

Native minimums and CI's known versions are:

### Native minimums and validated versions

| Tool | Supported minimum | CI/validated version |
| --- | --- | --- |
| CMake | `3.25.0` | `4.4.2` |
| Ninja | `1.10.0` | `1.13.2` |
| Git | `2.40.0` | `2.50.1` locally |
| Xcode | `15.0.0` | `26.6.0` locally on macOS arm64 |
| Visual Studio / MSVC | VS 2022, MSVC v143, compiler `19.38.0` or newer | workflow declared; real Windows run pending |

CMake and Ninja are installed at the exact CI versions by the workflow. A
newer compatible local native tool is allowed when the doctor accepts it.

## Clean setup: macOS arm64

Install Xcode 15 or newer, then install Node, Rust, CMake, Ninja, and Git from
their official distributions. From a fresh clone:

```sh
corepack enable
corepack prepare pnpm@11.19.0 --activate
rustup toolchain install 1.97.1 --profile minimal --component rustfmt --component clippy
pnpm install --frozen-lockfile
node tools/scripts/doctor.mjs
```

`node tools/scripts/doctor.mjs --json` emits the same read-only checks for
automation. Run the doctor directly with Node on a fresh machine: invoking it
through pnpm would require a working pnpm shim before diagnosis begins.

## Clean setup: Windows x64

Install Visual Studio 2022 with **Desktop development with C++**, MSVC v143,
and a Windows SDK. Install exact Node and Rust plus the supported CMake, Ninja,
and Git versions. In an x64 Visual Studio Developer PowerShell:

```powershell
corepack enable
corepack prepare pnpm@11.19.0 --activate
rustup toolchain install 1.97.1 --profile minimal --component rustfmt --component clippy
pnpm install --frozen-lockfile
node tools/scripts/doctor.mjs
```

The shell must expose `cl.exe` for the Ninja presets. The checked-in CI imports
`Microsoft.VisualStudio.DevShell.dll` and requests `-arch=x64 -host_arch=x64`;
use the same architecture when reproducing a failure.

The doctor reads declarations and version probes only. It does not install or
upgrade tools, change PATH, write configuration, or require Python. Its pnpm
probe disables Corepack network access, and its Rust probes disable automatic
toolchain installation.

## Repository checks

Run all JavaScript contracts, tooling fixtures, documentation checks, and
frontend tests, followed by the explicit frontend gates:

```sh
pnpm contracts:check
pnpm test
pnpm --filter @ai-voice-studio/desktop lint
pnpm --filter @ai-voice-studio/desktop typecheck
pnpm --filter @ai-voice-studio/desktop build
```

Run the complete Rust workspace checks with the pinned toolchain:

```sh
cargo +1.97.1 fmt --all -- --check
cargo +1.97.1 check --locked --workspace --all-targets
cargo +1.97.1 clippy --locked --workspace --all-targets -- -D warnings
cargo +1.97.1 test --locked --workspace
```

Build and test both native configurations:

```sh
cmake --preset native-debug
cmake --build --preset native-debug --parallel 2
ctest --preset native-debug
cmake --preset native-release
cmake --build --preset native-release --parallel 2
ctest --preset native-release
```

The first configure fetches spdlog from its official repository at the pinned
commit and verifies its SHA-256. It does not search for a system or Homebrew
copy. CTest owns the process integration and benchmark gates, including Rust
tests that Cargo deliberately marks ignored because they require CMake-built
native fixtures. Run `ctest --preset ...`; do not interpret an ignored Cargo
integration test as an omitted gate.

## Native staging and runtime integration

Stage profile- and target-qualified sidecars, plugins, and configuration before
running the real desktop/native integration:

```sh
pnpm desktop:native:prepare
pnpm desktop:test:native
pnpm desktop:native:prepare:release
```

For a specific cross-platform layout, invoke the staging script explicitly:

```sh
node tools/scripts/stage-desktop-native.mjs --profile Release --target aarch64-apple-darwin
```

On Windows use target `x86_64-pc-windows-msvc`. Staging validates source
artifacts and writes only the ignored Tauri `binaries/` and `resources/`
directories. The sidecar is named `voice-runtime-<target>` (plus `.exe` on
Windows); the plugin is `libaivs_mock_voice_engine-<target>.dylib` on macOS and
`aivs_mock_voice_engine-<target>.dll` on Windows.

## Desktop development and packaging

After Debug native staging, start the Vite frontend and Tauri development host:

```sh
pnpm desktop:native:prepare
pnpm --filter @ai-voice-studio/desktop tauri dev
```

Build and verify the macOS application layout:

```sh
pnpm desktop:native:prepare:release
pnpm --filter @ai-voice-studio/desktop tauri build --bundles app --ci
node tools/scripts/verify-desktop-bundle.mjs "target/release/bundle/macos/AI Voice Studio.app" aarch64-apple-darwin
```

The Windows workflow builds a portable layout because installer validation is
outside Phase 0.5. After Release staging in an x64 developer shell:

```powershell
node tools/scripts/stage-desktop-native.mjs --profile Release --target x86_64-pc-windows-msvc
pnpm desktop:test:native
pnpm --filter @ai-voice-studio/desktop tauri build --no-bundle --ci
$layout = Join-Path $PWD "target/ci-windows-layout"
New-Item -ItemType Directory -Force "$layout/native", "$layout/config" | Out-Null
Copy-Item "target/release/ai-voice-studio.exe" "$layout/ai-voice-studio.exe"
Copy-Item "apps/desktop/src-tauri/binaries/voice-runtime-x86_64-pc-windows-msvc.exe" "$layout/voice-runtime.exe"
Copy-Item "apps/desktop/src-tauri/resources/native/aivs_mock_voice_engine-x86_64-pc-windows-msvc.dll" "$layout/native/aivs_mock_voice_engine-x86_64-pc-windows-msvc.dll"
Copy-Item "apps/desktop/src-tauri/resources/config/config.json" "$layout/config/config.json"
node tools/scripts/verify-desktop-bundle.mjs target/ci-windows-layout x86_64-pc-windows-msvc
```

The verifier requires the desktop executable, sidecar, target-qualified Mock
plugin, and `config/config.json` at their platform layout paths. Consult
[`build.yml`](../../.github/workflows/build.yml) for the reproducible file-copy
steps used to assemble `target/ci-windows-layout`.

## Configuration and logs

[`config/config.json`](../../config/config.json) is a shipped, strict schema-v1
resource, not a writable settings store. Runtime and plugin paths are supplied
by the desktop resource owner and are intentionally absent from JSON. Invalid,
unknown, duplicate, out-of-range, or audio-enabling fields fail closed. See
[`CONFIGURATION.md`](CONFIGURATION.md) for the complete policy.

The host writes `runtime-host.jsonl`; the native Runtime writes
`voice-runtime.jsonl` and mirrors JSONL records to stderr. Their directory is
resolved from `debug.development_log_directory` beneath the application-data
root. stdout is binary RuntimeMessage IPC and must never be redirected into a
text log. Debug/trace needs both the configured level and explicit debug
authority. Phase 0.5 has no rotation or upload; stop both processes before
manually removing old development logs.

## CI gates and current limitation

[`build.yml`](../../.github/workflows/build.yml) defines bounded macOS 15 arm64
and Windows x64 MSVC jobs. Both run locked JS, frontend, Rust, CMake Debug and
Release, CTest, real native integration, staging, and Tauri layout validation;
caches are not correctness prerequisites. The macOS-equivalent commands have
been run locally. There is no Git remote in this workspace, so the workflow
has not executed on a GitHub Windows runner: Windows acceptance is pending and
not accepted. Triggering `push`, `pull_request`, or `workflow_dispatch` in a
GitHub repository is the remaining external action.

## Troubleshooting

- **Doctor reports a missing or wrong tool:** install the declared version and
  open a new shell. The doctor intentionally performs no repair.
- **`cl.exe` is missing:** re-enter an x64 Visual Studio Developer PowerShell
  with the C++ workload installed.
- **spdlog download fails:** restore network access to the pinned official
  archive or reuse an already populated CMake build cache; do not substitute an
  unverified system package.
- **Desktop reports missing native resources:** build CMake first and rerun the
  staging command for the exact profile and Rust target triple.
- **Tauri build cannot locate the sidecar:** verify the target-qualified file in
  `apps/desktop/src-tauri/binaries/` and rerun staging; do not copy a bare
  `voice-runtime` by hand.
- **Runtime start fails:** inspect the two JSONL logs and native stderr, confirm
  the shipped configuration is unchanged, and keep stdout reserved for IPC.
- **Cargo shows ignored native tests:** this is expected; CTest injects the real
  sidecar/plugin/fixture paths and executes those cases.

See the [architecture overview](../architecture/ARCHITECTURE.md) for ownership
and trust boundaries and the [documentation index](../README.md) for contracts
and decision history.
