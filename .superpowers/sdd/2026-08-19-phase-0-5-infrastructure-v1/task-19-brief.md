# Task 19 brief — engineering documentation and decision history

## Objective

Bring the Phase 0.5 engineering documentation into exact agreement with the
implemented repository. Complete the developer guide, add the missing system
architecture document, and reconcile ADR-000 through ADR-002. Each ADR must
retain an explicit status, context, decision, rationale, alternatives, and
consequences. ADR-003 remains only a reserved number for the Phase 1 Audio
Engine; no ADR-003 file is created.

## Audit findings

- `docs/architecture/ARCHITECTURE.md` does not exist.
- `docs/development/DEVELOPMENT.md` contains the original tool bootstrap and
  Debug native commands, but omits complete contracts/frontend/Rust/Release,
  staging, Tauri, bundle verification, Windows PowerShell, CI limitations, and
  practical troubleshooting. Its final paragraph still says the Desktop shell
  will be introduced, although the Tauri application is implemented.
- ADR-000 still describes repository-wide CI as future work even though the
  workflow exists locally.
- ADR-001 is substantially aligned with the current five-command, local-only,
  bounded-lifecycle adapter, but must link to the completed architecture and
  accurately distinguish locally validated macOS packaging from pending real
  Windows execution.
- ADR-002 contains a stale Phase 0.5 statement that no plugin, Runtime, or
  Engine implementation exists. The current Runtime dynamically loads the
  deterministic Mock plugin, runs the C ABI pipeline, and returns an 80-byte
  summary while keeping PCM native-only.
- `docs/README.md` does not index the development guide, architecture,
  configuration policy, protocol/ABI contracts, or ADRs.
- Current Task 16–18 evidence confirms cancel-safe child reap/reply ownership,
  the independent Mock benchmark gate, and a committed two-platform CI
  workflow. There is still no Git remote and therefore no real Windows CI
  result; documentation must not mark that acceptance gate complete.

## Source-of-truth boundaries

- Tool declarations: `.node-version`, root `packageManager`,
  `rust-toolchain.toml`, `CMakeLists.txt`, `CMakePresets.json`, and
  `.github/workflows/build.yml`.
- Runtime/protocol/ABI: `core/contracts`, `runtime/api/voice_engine.h`,
  `runtime/session`, `runtime/process`, `runtime/loader`, `runtime/pipeline`,
  `engines/mock`, and `apps/runtime-host`.
- Desktop/packaging: `apps/desktop`, `tools/scripts/stage-desktop-native.mjs`,
  and `tools/scripts/verify-desktop-bundle.mjs`.
- Configuration/capability/logging: `config/config.json`, `core/config`,
  `core/capability`, `core/telemetry`, and native Runtime logging.
- Verification status: CTest registration plus final Task 16, 17, and 18
  reports. Windows remains explicitly unexecuted.

## Automated documentation contract

Add a repository test that checks the required documents and ADR sections,
resolves every repository-relative Markdown link, verifies documented exact
tool versions against their declarations, verifies documented pnpm scripts and
CMake presets exist, rejects an ADR-003 file, and requires an explicit Windows
CI limitation. Register it in root `pnpm test` so drift cannot be silent.

The initial expected RED is the missing architecture document and incomplete
developer/ADR contract. Documentation changes then make the gate GREEN.

## Scope

Allowed tracked changes are Markdown documentation, its index/link updates, and
the minimal automated documentation-contract test plus root test registration.
Do not change production code, schemas, generated contracts, build behavior,
the main plan checkbox/progress ledger, or CI behavior. Do not add audio device
access, an Audio Engine, AI/model runtime, ONNX, Python, cloud, account, or user
features.

Completion produces local documentation evidence and a commit for independent
review. It does not claim Windows acceptance or Phase 0.5 final acceptance.
