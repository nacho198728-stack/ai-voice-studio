# Task 19 report — engineering documentation and decision history

## Outcome

Phase 0.5 documentation now describes the repository that actually exists:
the Tauri/Rust control plane, isolated `voice-runtime` process, RuntimeMessage
IPC, VoiceEngine C ABI, deterministic Mock plugin/pipeline, lifecycle/reap
ownership, privacy and configuration boundaries, staging, tests, and CI. The
developer guide contains clean macOS and Windows commands and exact tool
versions. ADR-000 through ADR-002 contain explicit accepted status, context,
decision, rationale, alternatives, and consequences.

ADR-003 was not created. It remains a reserved number for the Phase 1 Audio
Engine. The main plan checkbox/progress ledger was not modified. Windows x64 is
documented as pending because this workspace has no Git remote and no real
Windows CI result.

## Audit performed

The audit covered the main Phase 0.5 plan; the original development/configuration
guides and ADRs; protocol and ABI contracts; Task 16, 17, and 18 evidence; all
top-level/module READMEs; workspace manifests; exact toolchain declarations;
CMake presets and test registration; the two-job CI workflow; native staging
and bundle verification scripts; Tauri commands/resources/navigation; Rust
RuntimeManager process/correlation/generation/reaper code; Runtime Session,
options, logging, loader, pipeline, and C ABI; Mock engine behavior; capability,
configuration, telemetry, contract implementations; and native/Rust/JavaScript
integration and benchmark tests.

Material drift found before implementation:

- `docs/architecture/ARCHITECTURE.md` was absent.
- DEVELOPMENT stopped at bootstrap/Debug and described Tauri as future work.
- ADR-000 described cross-platform CI as future work.
- ADR-002 falsely said Phase 0.5 did not load plugins, run the Runtime, or
  implement an engine, although the Mock chain was complete.
- The docs index and several module READMEs still described implemented areas
  as placeholders.
- Windows CI structure existed, but real Windows execution had not occurred.

## TDD evidence

A new dependency-free Node test validates required documents and ADR sections,
resolves repository-relative links, derives documented versions from repository
and CI declarations, validates documented root pnpm scripts and CMake presets,
requires real architecture facts and exclusions, rejects an ADR-003 file, and
requires the Windows limitation in both architecture and development guides.

Initial RED was intentional and repository-real: `pnpm test:docs` passed 1 of 5
tests and failed 4 because the architecture document did not exist and the
developer guide omitted Tauri CLI `2.11.4`. After documentation changes, a
case-normalization defect in the new exclusion assertion exposed itself at 4
of 5; the assertion list was corrected to compare lowercase values. Final
`pnpm test:docs` is 5 of 5 GREEN and is registered in root `pnpm test`.

### Independent-review fix round

The first independent review rejected the initial gate because its architecture
check was a keyword set, its version check searched the whole document, its
link and ADR-003 scopes were narrow, and ADR-002 implied completed Windows ABI
testing. The fix round again started RED:

- The unchanged keyword predicate accepted a fully reversed
  `Mock VoiceEngine -> ... -> Tauri` mutation; the dedicated mutation assertion
  failed exactly as expected.
- The expanded contract then passed 4 of 8 tests and failed 4 for the missing
  ordered-edge table, authority-bound version tables, source-bound public DTO
  tables, and Windows evidence table.

The final 8-test gate parses an explicit ordered edge table and rejects a full
reverse mutation; matches the five Tauri commands, every `CapabilityDto` field,
and every public Mock summary field to Rust source; ties loader/pipeline edges
and the 80-byte size to source; binds versions to their named DEVELOPMENT table
rows and source declarations; validates root and filtered pnpm scripts, Node
paths, Rust pins, CMake preset types, and CI-exact staging/Tauri/copy arguments;
walks every Markdown document under `docs` and every authored source-tree
README (excluding recorded generated/dependency/build trees), including
reference links and anchors; and rejects case/separator variants of any actual
ADR-003 filename.

ADR-002 now has a platform evidence table: macOS is the locally executed ABI
gate; Windows is explicitly source/config/fixture-only, pending Task 18, and not
accepted. A completion-claim mutation fails, and the gate checks the other
architecture/development/ADR documents for the same contradiction. ARCHITECTURE
now enumerates every public capability field—including schema, Runtime/protocol
versions, engine identity, separate availability values, and generation—and
documents `not_evaluated` semantics without an inaccurate `only` claim.

## Files changed

- Added `docs/architecture/ARCHITECTURE.md`.
- Rebuilt `docs/development/DEVELOPMENT.md`; reconciled one future-tense line in
  `docs/development/CONFIGURATION.md`.
- Reconciled `docs/adr/ADR-000-monorepo.md`,
  `ADR-001-tauri-rust-control-plane.md`, and
  `ADR-002-cpp-runtime-c-abi.md` with current implementation and CI status.
- Expanded `docs/README.md` into the documentation/decision index.
- Corrected stale repository/module descriptions in the root, apps, audio,
  core, and tests READMEs.
- Added `tools/scripts/docs-contract.test.mjs` and registered `test:docs`.
- Added this ignored brief/report as task evidence.

No production code, generated contract, schema, native build behavior, CI
behavior, Audio/AI/Python implementation, or user feature changed.

## Manual consistency findings

- The documented five-command Tauri allowlist and bounded DTO boundary match
  the adapter and frontend port; no raw IPC, stderr, path, PID, or PCM crosses
  to the webview.
- RuntimeMessage limits match generated Rust/C++ constants: 32-byte header,
  65,536-byte payload, 65,568-byte frame/feed, and 64 messages per feed.
- The 80-byte Mock summary, 128 stereo frames, checksum/metrics, and native-only
  PCM match `runtime/pipeline` and the benchmark gate.
- C ABI ownership, exact factory symbol, eight operations, append-only structs,
  UTF-8 lengths, and hot-path prohibitions match the header, loader, Mock, and
  contract tests.
- Runtime lifecycle text matches generation/correlation/timeouts and the
  cancellation-safe ActorAbortOwner/ReapBarrier/ProcessResources ownership.
  Persistent wait/kill errors use an independent reaper, 5 ms to 1 s capped
  exponential backoff, and one-minute fixed diagnostics until concrete reap.
- Configuration, capability, and logging descriptions match their strict,
  privacy-minimal, fail-closed implementations.
- macOS/Windows staging names and layouts match the scripts and workflow.

## Verification

All verification ran from the repository root on macOS arm64:

- `pnpm test`: GREEN; 39 JavaScript/frontend tests total, including 8 document
  contract tests.
- Desktop lint, typecheck, and Vite build: GREEN.
- `cargo +1.97.1 fmt --all -- --check`: GREEN.
- Cargo locked workspace check and clippy across all targets: GREEN.
- Cargo locked workspace tests: GREEN; 50 passed and 35 fixture-dependent tests
  intentionally ignored under bare Cargo.
- CMake configure/build and CTest Debug: GREEN, 25/25.
- CMake configure/build and CTest Release: GREEN, 25/25.
- Doctor: all applicable local tools PASS; MSVC SKIP on macOS as designed.
- `git diff --check`: GREEN.
- No main-plan diff, no ADR-003 file, and no residual `voice-runtime` or
  controlled-child fixture process were found.

The CTest runs include the real process/IPC/C ABI integration cases that Cargo
marks ignored and the bounded Mock benchmark gate.

## Remaining external gate

This result is ready for independent documentation review, but it does not
complete the main plan. To establish Windows acceptance, an authorized owner
must attach/push the repository to GitHub and run the checked-in workflow on a
real `windows-latest` x64 runner. No remote was created and nothing was pushed.
