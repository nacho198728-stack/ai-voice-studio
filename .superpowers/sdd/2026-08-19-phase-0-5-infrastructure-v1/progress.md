# SDD ledger — plan: plans/2026-08-19-phase-0-5-infrastructure-v1.md

Spec: `/Users/alex/.codex/attachments/7abffca2-96cc-4fb4-b002-31d089ddbb80/pasted-text.txt`, supplemented by the user's ADR-000..002 requirement and explicit authorization to work in the current Git root.

Ruling: Work in the current checkout on `codex/phase-0-5-infrastructure` — the repository had an unborn HEAD, so a linked worktree could not be created; the user explicitly selected the current Git root — cost if wrong: changes are not isolated in a second directory, but remain isolated on a feature branch.

Ruling: Configuration/scaffolding files are exempt from test-first creation; every behavior-bearing Rust/C++/TypeScript unit follows red-green-refactor — TDD skill permits asking for exceptions, and generated/scaffolding files do not expose behavior by themselves — cost if wrong: some build metadata has validation only at integration-build time.

Ruling: ADR-000 is delivered with Monorepo initialization, ADR-001 with Tauri/Rust control-plane integration, ADR-002 with C++ Runtime/C ABI — matches the user's supplemental numbering — cost if wrong: later renaming would break historical links.

## Pre-flight conflict and interface scan

| Task | Produces / consumes | Shared interface or file | Finding / ruling |
|---|---|---|---|
| 1 | Produces top-level directories and metadata | Root paths consumed by all tasks | Consistent; empty directories need ownership README or `.gitkeep` until code arrives. |
| 2 | Produces pnpm/Cargo workspaces | Root metadata from Task 1; crates/apps from later tasks | Sequence after Task 1; workspace members may exist as minimal scaffolds before behavior. |
| 3 | Produces CMake targets/presets | Root layout from Task 1; C++ targets from Tasks 6–8 | Configure must succeed with placeholder targets, then later targets attach without duplicating build logic. |
| 4 | Produces pinned toolchain contract and doctor | Root package/build files from Tasks 2–3 | Environment installed first; doctor behavior must be tested against present/missing tools where feasible. |
| 5 | Produces version/ErrorCode sources | Consumed by Tasks 6–13 and tests 14–17 | Single source of truth; no magic duplicate versions in Rust/C++. |
| 6 | Produces VoiceEngine C ABI and ADR-002 | Consumed by Runtime Task 7, Mock Task 8, contract Task 14 | Must precede Runtime/Mock; pure C layout and ownership rules are binding. |
| 7 | Produces C++ Runtime process | Consumes Tasks 5, 6, 9; used by Rust Task 10 | Runtime can begin with test-driven command/state core before transport integration. |
| 8 | Produces Mock Engine plugin | Consumes Task 6; loaded by Task 7; tested by Tasks 14/17 | Deterministic pipeline only; no audio device or AI dependencies. |
| 9 | Produces RuntimeMessage/framing | Consumes Task 5; shared by C++ Task 7 and Rust Task 10 | Envelope is transport-neutral; stdout is protocol-only. |
| 10 | Produces Rust RuntimeManager | Consumes Tasks 7 and 9; used by Desktop Task 13 | State machine, process lifecycle and correlation require test-first behavior. |
| 11 | Produces tracing/spdlog schema | Touches Runtime, Rust host, config from Task 12 | Logging stdout conflict ruled: C++ stdout reserved for IPC, logs stderr/file. |
| 12 | Produces config/capability model | Consumed by Runtime/Rust/Desktop | Audio fields are placeholders only; no hardware benchmark or device access. |
| 13 | Produces Tauri shell and end-to-end control | Consumes Tasks 7–12 | UI receives only control/status/metrics; no PCM or model UI. ADR-001 completed here. |
| 14 | Produces C/C++ ABI contract tests | Verifies Tasks 6 and 8 | Test must compile/use real header/plugin, not grep source. |
| 15 | Produces Rust protocol tests | Verifies Tasks 5, 9 and Rust mappings | Literal fixtures and malformed inputs; no mirror assertions from production builders. |
| 16 | Produces Runtime integration tests | Verifies Tasks 7, 9, 10 | Must test real child process and graceful/abnormal exit without orphan. |
| 17 | Produces Mock Pipeline tests | Verifies Tasks 7–8 end-to-end | Output checksum/frames prove C ABI execution; no PCM in IPC response. |
| 18 | Produces macOS/Windows CI | Consumes all build/test commands | Windows runtime test must use actual runner; local macOS cannot close this gate. |
| 19 | Produces development/architecture/ADR docs | Documents Tasks 1–13 | ADRs ship with decisions, not retrospective prose; no source-text tests for human docs. |
| 20 | Produces acceptance report | Consumes all earlier evidence | Cannot claim Windows pass until remote CI actually runs; record pending limitation if no remote exists. |

Pre-flight result: no internal task contradiction after the rulings above. The only external gate is availability of a real Windows CI runner for final acceptance.

Ruling: The SDD `task-brief` helper cannot parse this validated checkbox-only plan because it requires `Task N` headings; create task briefs manually in this plan's ignored SDD workspace without changing the approved plan — cost if wrong: brief extraction is maintained manually, so each brief must cite the exact plan line and be checked during review.

## Progress

Environment setup: complete — rustc/cargo 1.97.1, rustup 1.29.0, CMake 4.4.2, Ninja 1.13.2, rustfmt/clippy installed and verified.

Task 1: complete — commit `c17baf6508da144a905491cee732dd616495beb0`; spec PASS, quality APPROVED, no findings. Monorepo ownership directories, root metadata and ADR-000 established.

Task 2: complete — commits `998860e87987885603d94ddf9e468e0f24ff63b6` and `861d6b38393a28ba32445d0a47b49d35f4564db3`; initial review found premature Rust toolchain pinning, fix-round re-review PASS and quality APPROVED. pnpm and Cargo workspaces compile and discover their minimal members; toolchain ownership remains deferred to Task 4.

Task 3: complete — commits `e53e6109c5c9aafb18e01cd419ce82f6ddc4e26d` and `550bab563ccdd20e0842d657e02c2683869e1497`; initial review found a duplicate CMake project version, fix-round re-review PASS and quality APPROVED. Debug/Release Ninja presets configure, build, and invoke CTest; CMake now derives the project version from root `VERSION`.

Task 4: complete — commits `53afdd872c50d6dd63b732b30eb20ee88a0cece7` and `c5e15877d63ed3c1e3f4a319e3436b46633ad90a`; initial review found auto-install, signal-exit, and MSVC-exit hazards, fix-round re-review PASS and quality APPROVED. Toolchain pins, platform setup guide, and read-only doctor are established; macOS checks pass, while real Windows validation remains assigned to Task 18 CI.

Task 5: complete — commit `b009a5c`; spec PASS and quality APPROVED with no findings. Canonical version/ErrorCode JSON, deterministic Rust/C++ generation, drift detection, and real mapping tests are established.

Task 6: complete — commits `ff9fcee`, `22425f1`, `4ca0e78`, and `c375177` plus evidence commits; final review after three fix rounds: spec PASS, quality APPROVED, no P0–P3 findings. VoiceEngine C ABI v1, immutable negotiation/layout rules, canonical C mapping, lifecycle/RT/failure contracts, and ADR-002 are frozen. Native execution is green on macOS arm64; Windows x64 remains a Task 18 CI gate.

Ruling: Execute RuntimeMessage/framing (plan item 9) before the C++ Runtime process (plan item 7) and Mock plugin (item 8) — the pre-flight interface scan already records that Runtime consumes the shared framing contract; defining a temporary transport inside Runtime would create avoidable protocol churn — cost if wrong: checkbox completion order differs from document order, but all validated outcomes remain unchanged.

Task 9: complete (executed before items 7–8) — commits `3d97018` and `a65ac02` plus evidence commits; fix-round re-review spec PASS and quality APPROVED with no findings. RuntimeMessage v1 schema, generated policy tables, bounded Rust/C++ codecs, golden fixtures, fatal/reset semantics, and allocation-failure handling are established.

Task 7: complete — commit `11d8867` plus evidence commit; spec PASS and quality APPROVED with no findings. The `voice-runtime` executable, transport-neutral Session state machine, bounded binary stdio adapter, deterministic Hello/capability/error payloads, exit semantics, and real child-process smoke are established.

Task 8: complete — commits `1be4308`, `8709064`, and `332be11` plus evidence commits; final review spec PASS and quality APPROVED with no findings. The production Mock VoiceEngine, explicit-path cross-platform loader, real C ABI pipeline, 80-byte checksum summary, strict Unicode CLI path, malformed-plugin matrix, and failure/lifetime tests are established. Native macOS arm64 is green; Windows x64 remains Task 18.

Task 10: complete — commits `8f7ae60`, `9f77de9`, and `6f43096` plus evidence commits; final review spec PASS and quality APPROVED with no findings. Rust RuntimeManager actor ownership, bounded writer/stdout/stderr tasks, strict response correlation, exact deadlines, ordered shutdown, reap-before-reply barriers, finite restart seam, and deterministic hostile-child tests are established.

Ruling: Execute minimal configuration/capability model (plan item 12) before unified logging (item 11) — logging level/directory initialization consumes the versioned debug/config contract named by item 11, while config must remain independent of logging — cost if wrong: checkbox completion order differs from document order without changing validated outcomes.

Task 12: complete — commits `7d79170`, `b57189b`, `27db08f`, and `45902b2` plus evidence commits; final gate review spec PASS and quality APPROVED with no findings. Versioned configuration, validated capability payloads, actor-authorized generation provenance, stale success/failure rejection, and compile-fail API boundaries are established. Native macOS arm64 is green; Windows x64 remains Task 18.

Task 11: complete — commits `466b527`, `0d418eb`, `065a68c`, `94bc56c`, and `892b6d9` plus evidence/follow-up commits; final gate review spec PASS and quality APPROVED with no findings. Rust tracing and instance-owned C++ spdlog emit the closed JSONL schema, verified policy gates verbose levels, Runtime stdout remains protocol-only, late sink failures are contained without path disclosure, portable log paths are validated, and Mock callback logging is prohibited by build/source/behavior contracts. Debug/Release CTest are 18/18 on macOS arm64; Windows x64 remains Task 18.

Task 13: complete — commits `2786fee`, `b45161d`, and `5ce689a`; final gate review spec PASS and quality APPROVED with no findings. The Tauri 2 shell, typed command adapter, guarded single window, bounded asynchronous exit, staged sidecar/resources, real invoke-to-C++/C-ABI Mock chain, stable-focus frontend, and ADR-001 are established. Concurrent Stop requests join one bounded reap completion and emit one native Shutdown. Debug/Release CTest are 19/19 and the unsigned macOS arm64 `.app` bundle is verified; Windows x64, signing, and notarization remain outside this task.

Task 14: complete — commits `dd12752`, `92f4ee5`, and `74cf631` plus evidence commits; final gate review spec PASS and quality APPROVED with no findings. Strict C17/C++20 external consumers independently freeze the 64-bit ABI layout, canonical scalar values, exact factory/operation signatures and calling convention, all initialization macros, version negotiation, failure atomicity, and the Mock plugin's sole export. Coordinated scalar-drift compile fixtures fail closed. Debug/Release CTest are 23/23 on macOS arm64; real MSVC execution remains Task 18.
