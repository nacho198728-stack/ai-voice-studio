# Task 13 report — Tauri desktop shell and ADR-001

## Status

Complete on macOS arm64. `apps/desktop` is a pnpm/Vite package and Tauri 2
Cargo workspace member. Its release application starts the independently
packaged C++ Runtime, reaches the C ABI Mock plugin through `RuntimeManager`,
returns bounded status/capability/summary DTOs, and performs ordered shutdown.
The main plan checkbox remains unchecked for independent parent review.

No Audio Engine, device API, JUCE, ONNX, AI/model behavior, Python, cloud,
account, file picker, playback, training, installer, signing, updater, or other
user workflow was introduced. ADR-003 was not created.

## Composition and boundary

- The composition root is Tauri `setup` -> `DesktopApplication` (retained
  `TelemetryGuard` + `CommandService`) -> `RuntimeControlPlane` -> the existing
  `RuntimeManager`. Initialization is polled on Tauri's async runtime because
  RuntimeHost requires an active Tokio handle.
- Exactly five input-free snake-case commands are registered:
  `start_runtime`, `get_runtime_status`, `get_runtime_capabilities`,
  `run_mock_pipeline`, and `stop_runtime`. Unknown commands fail and every
  non-null/non-empty or raw request payload is rejected as bounded
  `invalid_payload`.
- DTOs are explicit camel-case structures. Status excludes PID, stderr, exit
  internals, and raw manager errors. Mock output contains only input/output frame
  counts, a fixed-width hexadecimal checksum, elapsed microseconds, process
  call/error metrics, and stream generation. No PCM/sample array is present in
  the Rust or TypeScript command contract.
- The window renders the six Runtime states and only five lifecycle/query
  controls, bounded capability data, and the latest Mock summary. A single
  pending operation disables conflicting actions; success and bounded-error
  paths restore the controls.
- The main capability is local-only with an empty plugin permission list. CSP
  blocks connections, media, frames, objects, base changes, and form targets;
  only self scripts/styles and self/data images are allowed. Devtools are
  disabled and no shell, HTTP, filesystem, dialog, process, updater, clipboard,
  or other Tauri plugin is installed.

## Native staging and packaging

- `tools/scripts/stage-desktop-native.mjs` maps CMake Debug/Release outputs to
  Rust-target-qualified sidecar/plugin names, with explicit macOS/Linux/Windows
  extensions and Windows DLL layout. It obtains the default target from
  `rustc -vV`; runtime code never searches `PATH`, the current directory,
  Homebrew, or environment variables.
- Development paths are anchored to compile-time `CARGO_MANIFEST_DIR`. Release
  paths are derived only from the installed executable and Tauri resource
  directory. Pure Rust tests cover Unicode macOS paths and Windows `.exe`/`.dll`
  paths.
- Staged native files are ignored build outputs. No executable or dynamic
  library is tracked.
- Verified release artifacts:
  - `target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio`
  - `target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/voice-runtime`
  - `target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib`
  - `target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/config/config.json`
- Bundle verification has a regression case for the rejected accidental
  `Contents/Resources/resources/...` nesting.

## Exact dependencies

- Rust: `tauri = 2.11.5`, `tauri-build = 2.6.3`; test feature uses the same
  Tauri patch. `png = 0.18.1` generates the minimal icon required by the official
  Tauri build without committing binary source assets.
- JavaScript: `@tauri-apps/api = 2.11.1`, `@tauri-apps/cli = 2.11.4`,
  `vite = 8.2.1`, `vitest = 4.1.11`, `typescript = 5.9.2`,
  `eslint = 10.8.1`, `@eslint/js = 10.0.1`, `typescript-eslint = 8.67.0`, and
  `jsdom = 30.0.1`.
- TypeScript 5.9.2 is deliberately used instead of registry-latest TypeScript 7
  because the exact `typescript-eslint` peer range is below 6.1. All versions
  are exact and both lockfiles are updated.

## RED/GREEN evidence

- Frontend RED first failed because the application module did not exist.
  GREEN added the typed invoke adapter, six-state rendering, pending exclusion,
  success/error recovery, capability/summary rendering, and responsive styles.
  A later RED found the status surface lacked live-region semantics; GREEN added
  `aria-live` and `aria-atomic`. Result: 2 files, 7 tests.
- Rust adapter RED first failed because the desktop crate and DTO/control-plane
  port did not exist. GREEN added bounded DTO/error mapping, pure platform path
  resolution, validated manager-config construction, and five service methods.
  Result: 5 adapter tests.
- Tauri IPC RED first failed because the real handler registration did not
  exist. GREEN registered only the five typed commands. An adversarial RED then
  proved Tauri otherwise ignored extra request fields; GREEN added centralized
  empty-body validation. Result: 1 mock-runtime IPC test covering all five
  commands, repeated lifecycle error, payload rejection, unknown privileged
  names, and absence of PCM/sample fields.
- Staging RED failed because the script did not exist. GREEN added deterministic
  macOS/Windows Debug/Release plans, minimal copying, and invalid/missing-input
  rejection. Result: 3 tests.
- Bundle RED failed because verification did not exist. The first real release
  bundle then revealed resources under an extra `resources/` directory. GREEN
  switched to Tauri's resource map and added the nested-layout regression.
  Result: 2 tests and a verified `.app`.
- Native integration RED failed because no desktop-owned application/service
  existed. GREEN starts staged `voice-runtime`, handshakes over IPC, reads the
  C++ capability result, executes the C ABI Mock pipeline, checks 128 input and
  128 output frames, checksum `3ecd5190f6f4f725`, one call, zero errors, Rust and
  C++ structured logs, then stops and reaps.
- An actual release launch exposed a composition-root defect not visible in
  Tokio tests: synchronous Tauri setup constructed `RuntimeManager` without an
  active Tokio handle. RED was captured by changing the native test contract to
  await initialization (the sync result was not a Future). GREEN made desktop
  initialization async and polls it through Tauri's async runtime. The rebuilt
  `.app` then completed the full interactive smoke and exited with status 0.

## Verification

- Frontend `lint`, `typecheck`, Vitest, and Vite build — passed; 2 files/7 tests,
  0 failures, production assets emitted without missing-asset warnings.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed with no
  warnings.
- `cargo check --workspace --all-targets` — passed.
- `cargo test --workspace` — passed. Desktop adapter 5/5 and Tauri IPC 1/1;
  the explicit staged-native test remains intentionally ignored by the generic
  workspace invocation and is run separately below. Existing workspace unit,
  integration, and compile-fail doctests passed; CTest-owned native-path cases
  remained ignored in this generic invocation.
- `pnpm desktop:native:prepare && pnpm desktop:test:native` — passed; the explicit
  real RuntimeManager -> IPC -> C++ Runtime -> C ABI -> Mock integration was 1/1
  and produced both owned Rust and C++ lifecycle logs before reap.
- Fresh Debug configure/build/CTest — passed 18/18.
- Fresh Release configure/build/CTest — passed 18/18.
- `pnpm contracts:check` — passed without generated drift.
- Root `pnpm test` — passed: contracts 6/6, doctor 7/7, staging 3/3, bundle 2/2,
  frontend 7/7.
- `tauri build --debug --no-bundle` — passed at
  `target/debug/ai-voice-studio`.
- Release `tauri build --bundles app` and `pnpm desktop:verify:bundle` — passed at
  `target/release/bundle/macos/AI Voice Studio.app` with the four expected owned
  artifacts listed above.
- Real release GUI smoke through macOS accessibility/computer-use — passed at
  normal and approximately 410 px widths: stopped initial state, Start ->
  connected generation 1, capability `mock/aivs-mock-v1`, Mock 128 -> 128 and
  checksum/metrics, Stop -> stopped, disabled-state transitions, narrow
  single-column wrapping/vertical scrolling, and clean application exit 0.
- Final `git diff --check` and format check — passed. Process scan after GUI and
  integration shutdown found no residual `voice-runtime` child.

## Files

- Workspace and locks: `.gitignore`, `Cargo.toml`, `Cargo.lock`, `package.json`,
  `pnpm-lock.yaml`, `apps/desktop/package.json`.
- Frontend/build: `apps/desktop/index.html`, `tsconfig.json`, `vite.config.ts`,
  `eslint.config.js`, and `apps/desktop/src/{api,app,main,styles}.*` including
  focused tests.
- Tauri adapter: `apps/desktop/src-tauri/Cargo.toml`, `build.rs`,
  `tauri.conf.json`, `capabilities/main.json`, `src/lib.rs`, `src/main.rs`, and
  `tests/{adapter,command_ipc,native_command_service}.rs`.
- Staging/bundle evidence: `tools/scripts/stage-desktop-native.mjs` and its test;
  `tools/scripts/verify-desktop-bundle.mjs` and its test.
- Decision: `docs/adr/ADR-001-tauri-rust-control-plane.md`.
- This evidence: `.superpowers/sdd/2026-08-19-phase-0-5-infrastructure-v1/task-13-report.md`.

## Limitations

- Execution and GUI evidence is macOS arm64 only. Pure path/staging tests cover
  Windows naming, but Windows x64/MSVC compilation, launch, DLL loading, process
  shutdown, and Tauri behavior remain Task 18 and are not claimed here.
- The produced `.app` is an unsigned local build. Installer generation, signing,
  notarization, release entitlements, and auto-update are outside Phase 0.5.
- Product/runtime versions remain the development placeholders defined by the
  current repository contracts (`0.0.0-dev` / `0.0.0`).
