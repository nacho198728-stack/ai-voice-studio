# Task 17 report — Mock Pipeline benchmark gate

## Status

Complete on macOS arm64. The repository now has the main plan's real
`tests/benchmark` directory and one independently named, bounded CTest gate for
the production Mock Pipeline. No Mock Engine or Runtime production change was
required: the audit and new external gate confirmed Task 8 behavior rather than
duplicating it.

The main plan checkbox/progress ledger was not edited, ADR-003 was not created,
and no Audio Engine, device API, AI/model runtime, ONNX, Python, JUCE, Tauri/UI,
cloud, installer, or user feature entered scope.

## Audit and decision

Task 8 already established the dynamically loaded production Mock shared
library, complete VoiceEngine v1 C ABI implementation, explicit loader,
deterministic sign-bit transformation, bounded work iterations, reset and
metrics behavior, fixed 80-byte pipeline summary, Session dispatch, and real
Runtime process smoke. Its tests covered those pieces in separate lifecycle,
contract, and smoke targets.

The Task 17 gap was structural and evidentiary: `tests/benchmark` did not exist
and CTest had no independent benchmark gate collecting the fixed-vector,
timing, reset, metrics, and IPC-summary acceptance contract. The plan explicitly
names that directory and test class, so reusing the Task 8 targets would not
close the item.

The new `aivs_mock_pipeline_benchmark_gate` links the Runtime loader/pipeline,
not the Mock implementation. It receives the built shared-library path and
therefore loads and invokes the real production C ABI table. CTest labels it
`benchmark` and enforces a 20-second process timeout.

## Requirement-to-evidence map

- **Real dynamic Mock/C ABI pipeline:** the gate loads the production plugin by
  absolute path through `VoiceEngineModule`, initializes the strict production
  configuration, loads `mock-v1`, prepares, processes, resets, retrieves
  metrics, and unloads through the real C ABI-backed instance.
- **Literal per-sample/per-byte output:** a hand-pinned eight-value float bit
  vector repeats across 128 stereo frames. Every output bit pattern and each of
  its four little-endian bytes is compared with literal expected arrays.
- **Fixed checksum:** both direct C ABI output and the Session summary must equal
  the external literal `0x3ecd5190f6f4f725`; the expected value is never
  generated from the implementation under test.
- **Configured work lower bound:** three calls at the maximum supported
  `work_iterations=1000000` must take at least 150 microseconds total. This is a
  deliberately conservative lower bound for three million stateful iterations;
  scheduling noise can only increase it, while the CTest 20-second timeout
  supplies the non-hang upper bound.
- **Reset semantics:** generation starts at 1, reset advances it to 2, the old
  process request fails in the unprepared state, re-prepare advances to 3, and
  the same input produces byte-identical output and the same fixed checksum.
- **Exact metrics/failure path:** initial metrics are all zero. One success plus
  a too-small output produces calls/frames/errors `1/128/1`; post-reset stale
  processing adds exactly one error; the second success ends at calls/input/
  output/errors `2/256/256/2`. Both prepare results pin zero algorithmic
  latency. The configured-work engine independently ends at `3/384/384/0`.
- **Summary and no PCM over IPC:** a real `Session` dispatches a
  `RunMockPipeline` RuntimeMessage and preserves request ID/command/success. Its
  payload is exactly 80 bytes and the gate decodes literal schema size, 128
  frames, 2 channels, checksum, latency, generation, and metrics offsets. Exact
  fixed size proves no 256-sample PCM buffer crossed the RuntimeMessage boundary.
- **Repeatability/deadline:** the focused test passed ten consecutive times in
  both Debug and Release; each invocation is independently bounded by CTest.

## TDD evidence

RED was the missing plan artifact itself. Before adding test code or CMake
wiring, the exact gate lookup
`ctest --test-dir build/native-debug -N -R '^aivs_mock_pipeline_benchmark_gate$'`
reported no matching test, and the assertion for `Total Tests: 1` failed.

GREEN added only `tests/benchmark/mock_pipeline_benchmark_test.cpp` and the
independent target/CTest registration. The behavioral gate passed without a
production fix, confirming the implementation was already sufficient and the
missing acceptance boundary was the real gap.

## Verification

- Focused Debug benchmark CTest: passed; repeated 10/10.
- Focused Release benchmark CTest: passed; repeated 10/10.
- Warning-flags build of the benchmark target: passed.
- Full Debug build/CTest: 25/25 passed.
- Full Release build/CTest: 25/25 passed.
- Rust workspace fmt/clippy/check/test: passed; 67/67 executed tests passed
  (fixture-dependent integration cases remained intentionally ignored outside
  their CTest drivers).
- `pnpm contracts:check` and root `pnpm test`: passed; contracts were current
  and 26/26 JavaScript/TypeScript tests passed.
- `git diff --check`: passed. Final process/temp scan found no live
  `voice-runtime`/child-fixture process and no `aivs-runtime-gate-*` directory.

## Files

- Gate implementation:
  `tests/benchmark/mock_pipeline_benchmark_test.cpp`.
- CMake/CTest registration and 20-second deadline: `tests/CMakeLists.txt`.
- Task brief and evidence:
  `.superpowers/sdd/2026-08-19-phase-0-5-infrastructure-v1/task-17-brief.md`
  and `task-17-report.md`.

## Limitation

Native Windows x64 execution remains Task 18. The gate uses only APIs already
implemented for both platforms and asserts the two supported targets' explicit
little-endian byte contract, but this macOS result does not claim Windows
`LoadLibraryW` or MSVC execution evidence.
