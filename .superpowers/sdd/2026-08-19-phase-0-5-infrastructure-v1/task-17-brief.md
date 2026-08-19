# Task 17 brief — Mock Pipeline benchmark gate

## Goal

Close only the Phase 0.5 `tests/benchmark` plan item with an independent,
repeatable native gate proving that the built Mock shared library executes the
real VoiceEngine C ABI pipeline. Preserve the established Task 8 implementation
unless a test-first benchmark exposes a production defect.

## Audit baseline

Task 8 already provides the production Mock plugin, explicit-path dynamic
loader, `MockPipeline`, fixed 80-byte little-endian summary, Runtime integration,
and broad ABI/lifecycle tests. Existing tests establish individual parts of the
required behavior, but the repository has no `tests/benchmark` directory, no
independent benchmark executable, and no bounded CTest target collecting the
pipeline-specific acceptance evidence required by the main plan.

## Required gate

- Dynamically load the built production Mock library and negotiate its real C
  ABI table; do not link the engine implementation into the test.
- Feed a literal fixed float-bit vector and assert every output sample and each
  output byte against hand-derived literals. Assert a fixed externally pinned
  checksum, never a checksum computed from the implementation under test.
- Exercise zero and maximum supported work-iteration configurations. The
  configured-work call must exceed a conservative steady-clock lower bound,
  while CTest supplies a hard upper timeout so a regression cannot hang CI.
- Assert exact metrics deltas for successes and failures: calls, input/output
  frames, errors, and the fixed zero algorithmic latency.
- Assert reset invalidates the prior prepared generation, advances generation,
  permits re-prepare, and reproduces the exact output/checksum afterward.
- Run the production `MockPipeline` summary path and assert exact frame count,
  channel count, checksum, generation/metrics fields, and exactly 80 payload
  bytes. The fixed-size schema is the proof that no PCM crosses the control
  boundary.
- Register one independently named CTest under `tests/benchmark`, with a finite
  timeout, and demonstrate repeated Debug and Release execution.

## Scope limits

No Audio Engine, CoreAudio, WASAPI, JUCE, ONNX, AI/model runtime, Python,
hardware/device access, Rust/Tauri feature, IPC redesign, cloud, installer, or
user feature. Do not create ADR-003 and do not edit the main plan checkbox or
progress ledger. Test harness code may duplicate platform loading glue, but it
must not duplicate Mock/Runtime production logic.

## TDD and verification

First demonstrate the missing benchmark CTest as RED, then add the smallest
test/registration needed for GREEN. If the behavioral test exposes a production
failure, add only the minimal Mock/Runtime fix after preserving the failing
case. Verify focused repeat runs, full Debug/Release CMake/CTest, Rust
fmt/clippy/check/test, pnpm contracts/tests, diff cleanliness, and process leak
scan. Record the requirement-to-evidence map and RED/GREEN results in
`task-17-report.md`; the parent owns plan completion.
