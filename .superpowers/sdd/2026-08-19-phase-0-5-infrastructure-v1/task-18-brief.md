# Task 18 brief — macOS arm64 and Windows x64 CI

## Objective

Add `.github/workflows/build.yml` with two clean GitHub-hosted jobs: a real
macOS arm64 runner and a Windows x64 runner using MSVC. Both jobs must execute
the checked-in contracts, JavaScript/TypeScript checks, exact Rust toolchain
checks, CMake Debug/Release builds and CTest, and a Tauri build/layout check.
The Windows job must explicitly run the MSVC export, dynamic-library loading,
Runtime process shutdown/reap, and desktop shutdown-race gates.

## Audited baseline

- The repository has no Git remote and no existing `.github/workflows` file.
- `macos-15` is currently documented by GitHub as a standard arm64/M1 runner;
  the job will also fail closed unless `uname -m` and the Rust host are arm64.
- `windows-latest` is documented as x64; the job will verify the process and
  Rust host architectures and enter the installed Visual Studio x64 developer
  shell before invoking the Ninja presets.
- Node `24.16.0`, pnpm `11.19.0`, and Rust `1.97.1` are exact repository
  contracts. CMake `4.4.2` and Ninja `1.13.2` are the current validated pair;
  CI will install exactly those versions through an immutable action commit.
- `CMakePresets.json` already defines Ninja Debug and Release builds. CTest
  already owns the Mock DLL unique-export check, real dynamic loading, Runtime
  process smoke, RuntimeManager integration gate, and desktop exit race.
- `stage-desktop-native.mjs` already maps `.exe`/`.dll` and target-qualified
  Windows sidecars. `verify-desktop-bundle.mjs` supports a Windows portable
  root but lacks a Windows fixture.
- Tauri's checked-in `app` bundle target is macOS-specific. macOS CI can build
  and inspect the real `.app`; Windows CI must use the official `--no-bundle`
  build mode and verify an explicitly assembled portable resource layout.

## TDD contract

First add a parser-backed test for the real workflow file and register it in
the root test command. It will name the concrete break each assertion catches:
wrong runner architecture, floating action revision, cache dependence, missing
locked tool version, omitted Debug/Release gate, omitted MSVC/process gate, or
incorrect sidecar/resource layout command. Before the workflow exists, this
test must fail specifically because `.github/workflows/build.yml` is absent.

Add an independent Windows layout fixture to the existing bundle verifier
tests. It is an audit characterization of already implemented path behavior,
not the Task 18 RED.

## Action and cache policy

- Use GitHub-owned `actions/checkout` and `actions/setup-node`, pinned to full
  immutable commits with reviewed release comments.
- Use `lukka/get-cmake` only for the otherwise missing exact CMake/Ninja install
  boundary; pin its reviewed release to the peeled immutable commit.
- Disable setup-node package-manager caching and both get-cmake caches. A clean
  runner must download/install successfully; cache state is never correctness.
- Do not add upload, release, signing, deployment, or write permissions.

## Scope and completion boundary

Allowed changes are the workflow, its static/fixture tests and test dependency,
the Windows bundle-layout fixture, and the Task 18 report. Do not add Audio
Engine/device APIs, AI/model/ONNX/Python behavior, ADR-003, user features, or
main-plan checkbox/progress edits.

Local completion proves the workflow contract plus the full macOS-equivalent
command sequence. With no remote, it cannot produce a real Windows runner
result and therefore must not mark the plan's Task 18 checkbox complete. The
minimum external action is to create or choose a GitHub repository, add it as a
remote, push the branch, and let the `windows-x64` job finish successfully.
