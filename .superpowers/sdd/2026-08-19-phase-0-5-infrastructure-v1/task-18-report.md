# Task 18 report — cross-platform build workflow

## Status

The CI implementation and local macOS arm64 evidence are complete. Task 18 is
not accepted yet because this repository has no Git remote and therefore no
real `windows-latest` execution result. The main plan checkbox/progress ledger
was deliberately not edited. ADR-003 was not created.

No Audio Engine or device API, AI/model runtime, ONNX, Python, user feature,
release, signing, deployment, or remote mutation entered scope.

## Audit result

Before this task, `.github/workflows/build.yml` and the workflows directory did
not exist. The repository already had the cross-platform implementation seams:

- Ninja Debug/Release CMake presets and platform-neutral artifact directories;
- CTest gates for C/C++ ABI, Mock unique export, real dynamic loading, real
  Runtime smoke, RuntimeManager process/hostile framing/reap behavior, and the
  desktop shutdown race;
- exact Node, pnpm, and Rust declarations plus compatible CMake/Ninja doctor
  rules;
- target-qualified Tauri sidecar staging for macOS and Windows, including
  `.exe`, `.dll`, and `.dylib` naming;
- a bundle verifier whose Windows branch was implemented but lacked a fixture.

No production Runtime, Mock, Desktop, or staging fix was required. The only
behavioral coverage addition outside the workflow contract is a Windows x64
portable-layout fixture for the existing bundle verifier.

## Workflow contract

The new workflow has two independent, 45-minute-bounded jobs and read-only
repository permissions:

- `macos-arm64` runs on `macos-15`, which GitHub currently documents as a
  standard M1/arm64 runner. It also rejects any machine where `uname -m` or the
  Rust host is not arm64/aarch64. It builds a real Tauri `.app` and verifies the
  executable, sidecar, Mock dylib, and config inside the generated bundle.
- `windows-x64` runs on the required `windows-latest`, rejects a non-x64
  process/Rust host, locates Visual Studio with `vswhere`, and enters the x64
  MSVC developer shell before the Ninja builds. Full Debug and Release CTest
  run, followed by an explicit focused Debug rerun for unique DLL export,
  dynamic loading, Runtime smoke, RuntimeManager child/gate, and desktop exit
  race. Tauri uses its documented `--no-bundle` mode because the checked-in
  `app` target is macOS-specific; CI then constructs and verifies the portable
  Windows executable/sidecar/DLL/config layout.

Both jobs run frozen pnpm installation, contracts, the complete root pnpm test
suite, frontend lint/typecheck/build, Rust fmt/check/clippy/test with the locked
toolchain, and CMake configure/build/CTest in both configurations. The staged
native Tauri command test executes the real Runtime → C ABI → Mock chain and
requires ordered process reap.

## Dependency and cache policy

All action references are immutable 40-character commits, with the reviewed
release recorded beside each reference:

- `actions/checkout` v6.0.2:
  `de0fac2e4500dabe0009e67214ff5f5447ce83dd`;
- `actions/setup-node` v6.4.0:
  `48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e`;
- `lukka/get-cmake` v4.4.2 peeled commit:
  `fffaaafeea488556c2c12dad60690008bc1caacb`.

The first two actions are GitHub-owned. `get-cmake` is the sole third-party
action and supplies the otherwise missing exact CMake `4.4.2` and Ninja
`1.13.2` install boundary on both architectures. Its current action metadata
uses Node 24. setup-node package-manager caching and both get-cmake cache modes
are disabled; a cache miss or empty cache cannot change correctness.

Node `24.16.0`, pnpm `11.19.0`, Rust `1.97.1`, CMake `4.4.2`, and Ninja `1.13.2`
are all checked at runtime before build commands execute.

## TDD evidence

The production change named by the workflow tests is the presence and semantic
content of `.github/workflows/build.yml`. The test parses the actual YAML rather
than grepping lines, and independently asserts runners, permissions, deadlines,
action pins, cache policy, exact tool versions, every shared gate, MSVC setup,
the focused native Windows gates, and platform-specific Tauri layout commands.

RED: before creating the workflow, all four workflow tests failed with the
expected `ENOENT` for `.github/workflows/build.yml`. The independent Windows
bundle fixture passed 3/3 at this point, proving that existing path behavior did
not require a production fix.

GREEN: after adding only the workflow, the parsed workflow contract passed 4/4.
The root test command now always includes this gate. The exact YAML also passed
`actionlint` 1.7.12 on darwin/arm64 after its official release archive checksum
was verified.

## Local verification — macOS arm64

- Exact host/tool guard: arm64, Node 24.16.0, pnpm 11.19.0, Rust 1.97.1
  aarch64 host, CMake 4.4.2, Ninja 1.13.2; doctor passed.
- `pnpm install --frozen-lockfile`: passed.
- Contracts/root pnpm/frontend: 31/31 tests passed; ESLint, TypeScript, and Vite
  build passed.
- Rust: fmt, locked all-target check, clippy with warnings denied, and workspace
  tests passed; 67/67 executed tests passed (fixture-dependent cases are driven
  by CTest).
- Native Debug: configure/build passed; CTest 25/25.
- Native Release: configure/build passed; CTest 25/25.
- Staged native Tauri command: 1/1, including real handshake, Mock command,
  ordered shutdown, and reap.
- Tauri release `.app`: built successfully and its application, sidecar, Mock
  dylib, and config layout were verified.
- `git diff --check`: passed after the final report/test edits.
- Runtime/fixture process and `aivs-runtime-gate-*` temp leak scan: clean.

## Files

- `.github/workflows/build.yml`
- `tools/scripts/build-workflow.test.mjs`
- `tools/scripts/verify-desktop-bundle.test.mjs`
- `package.json` and `pnpm-lock.yaml` for the exact test-only YAML parser
- this brief/report evidence pair

## External acceptance still required

The local machine cannot substitute for Windows. The minimum external actions
are:

1. create or select a GitHub repository for this existing local repository;
2. add that repository as a Git remote and push this branch;
3. allow the workflow's `windows-x64` job to run on a real hosted runner;
4. retain the successful job URL/log as Task 18 evidence, then and only then
   update the main plan checkbox/progress ledger.

This task did not create a remote, push, or claim a Windows result.
