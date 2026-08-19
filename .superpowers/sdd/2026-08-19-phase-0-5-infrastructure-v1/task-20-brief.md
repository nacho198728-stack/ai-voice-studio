# Task 20 brief — local final acceptance and pending report

## Objective and status rule

Produce a fresh macOS arm64 Phase 0.5 acceptance evidence draft at
`docs/development/PHASE-0.5-ACCEPTANCE.md`. The report maps every plan and
verification requirement to commands, results, versions, and artifacts, but
must remain `PENDING`/`BLOCKED`: this workspace has no Git remote or real
Windows x64 workflow URL. Do not check Task 18 or Task 20, write “Phase 0.5
complete,” create ADR-003, create a remote, or push.

## Audit baseline

- The main plan has Tasks 1–17 and 19 checked. Task 18 is intentionally
  unchecked because its workflow has never run on Windows; Task 20 is the
  current unchecked final gate.
- Task reports 1–19 and their implementation/review commits record the
  monorepo, workspaces/toolchain, contracts, native Runtime/Mock, bounded IPC,
  RuntimeManager, logging, config/capability, desktop shell, focused contract/
  process/benchmark gates, CI definition, and current architecture documents.
- The checked-in workflow defines macOS arm64 and Windows x64 MSVC jobs. The
  latter is configuration only until GitHub produces a successful job URL.
- Current local bundle layout has exactly the desktop executable, sidecar,
  target-qualified Mock dylib, config, Info.plist, and icon. It must be rebuilt
  before evidence is recorded; an old bundle is not acceptance evidence.
- Prior interactive GUI evidence exists, but later attempts reported a locked
  macOS session. Task 20 must distinguish a fresh visual check, a launch/quit
  smoke, and the non-visual Tauri invoke integration rather than merging them.

## Fresh macOS gates

Run the read-only doctor; frozen pnpm install; contracts, documentation,
workflow, staging, bundle, and frontend suites; frontend lint/typecheck/build;
Rust fmt/locked check/clippy/test; native Debug and Release configure/build and
CTest; focused benchmark repeats; staged real Tauri invoke integration; Release
native staging; Tauri `.app` build and bundle verification; actual app launch
and bounded quit; process/temp leak checks; and final diff/plan/ADR checks.

Every potentially hanging process or GUI step receives a deadline and cleanup
guard. A locked or inaccessible GUI must be reported as “not visually
reverified”; automated invoke/launch evidence is not a visual substitute.

## Exclusion audit

The absence claim must combine independent evidence rather than keyword grep:

1. production Cargo and pnpm dependency trees;
2. CMake target/link graph for Runtime and Mock;
3. rebuilt `.app` file inventory and file types;
4. Mach-O dynamic dependency lists for the desktop, sidecar, and plugin;
5. undefined/global symbol inspection for Python, PyTorch, CUDA, ONNX Runtime,
   and real audio-device API entry points;
6. package/archive/model-extension inspection and source/build ownership review;
7. functional evidence that PCM is native-only and the public result is the
   fixed 80-byte summary.

A small audited bundle-inspection script and fixture tests may be added if they
make these checks repeatable. Frameworks such as WebKit/CoreVideo pulled by the
Tauri shell must be reported accurately and not mislabeled as audio-device
access; the absence conclusion concerns bundled runtimes/models and direct
device API imports/targets.

## Deliverables

- `docs/development/PHASE-0.5-ACCEPTANCE.md` with requirement matrix, fresh
  macOS results, Windows status plus an empty evidence-URL field, exact absolute
  and repository-relative artifact paths, versions, limitations, and Phase 1
  inputs.
- An acceptance verifier/test if needed for reproducible bundle exclusion and
  report drift checks.
- `task-20-report.md` with RED/GREEN and raw command/result summary.
- One focused commit for independent review; no main-plan/progress mutation.
