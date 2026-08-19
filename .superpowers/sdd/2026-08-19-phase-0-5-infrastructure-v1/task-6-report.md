# Task 6 report — VoiceEngine C ABI v1 and ADR-002

## Status

Completed on `codex/phase-0-5-infrastructure`. The change defines only the
public in-process ABI contract; it does not implement a Runtime, loader, Mock
Engine, IPC, Rust FFI, Tauri integration, device access, or model execution.

## ABI decisions

- `runtime/api/voice_engine.h` is a C17/C++20 header with exactly one plugin
  export, `aivs_voice_engine_get_api`, and a versioned function table for
  initialize, shutdown, get-engine-info, load-model, prepare-stream,
  process-audio, reset, and get-metrics.
- Every extensible structure and the table lead with `uint32_t struct_size` and
  `uint32_t abi_version`; extensions are append-only, and all reserved fields
  are zero-initialized/returned as zero. The opaque handle is released by a
  successful, non-idempotent shutdown and must never be dereferenced afterward.
- Byte/string views and all PCM/result buffers are caller-owned and carry
  length/capacity plus written-or-required counts. UTF-8 is length-delimited.
  Phase 0.5 uses IEEE-754 float32 interleaved PCM and deterministic one-input-
  frame-to-one-output-frame capacity semantics.
- Canonical JSON remains the source of truth. The generator now produces a
  C mapping in addition to Rust/C++; it adds only engine errors 1301
  `InvalidArgument`, 1302 `InvalidState`, and 1303 `BufferTooSmall`, preserving
  all prior values.
- ADR-002 and the focused ABI contract document bind the boundary to
  `Tauri -> Rust RuntimeManager -> IPC -> C++ Runtime -> VoiceEngine C ABI -> Engine`;
  they define lifecycle, failure, ownership, and nonblocking realtime rules.

## Files

- ABI/header and CMake interface target: `runtime/api/voice_engine.h`,
  `runtime/CMakeLists.txt`.
- Canonical mapping changes: `core/contracts/error-codes.json`,
  `tools/scripts/contracts.mjs`, generated Rust/C++/C mappings, and generator
  tests.
- Real public-header C17/C++20 contract executables:
  `tests/contract/voice_engine_c_abi_c_test.c`,
  `tests/contract/voice_engine_c_abi_cpp_test.cpp`, and `tests/CMakeLists.txt`.
- Decisions and operating contract: `docs/adr/ADR-002-cpp-runtime-c-abi.md`,
  `docs/contracts/voice-engine-c-abi-v1.md`.

## RED/GREEN evidence

- RED: added the C/C++ public-header contract tests before the ABI header.
  `cmake --build --preset native-debug` failed as expected because
  `voice_engine.h` did not exist. Added a generator test for the C mapping;
  `node --test tools/scripts/contracts.test.mjs` failed as expected because
  `generated_contracts_c.h` did not exist.
- GREEN: added the pure C ABI header, canonical C generator output, minimal
  error extensions, CMake interface target, and documentation. The C test
  asserts prefix/layout and canonical C values; the C++ test asserts the real
  factory function type and public layout. Both compile and execute through
  CTest without an engine implementation.

## Verification

- `pnpm test` — passed: contracts drift check, contract generator tests 4/4,
  and doctor tests 7/7.
- `cargo +1.97.1 fmt --all -- --check`, `cargo +1.97.1 check --workspace`, and
  `cargo +1.97.1 test --workspace` — passed; Rust tests 2/2.
- Fresh Debug and Release CMake configure/build/CTest — passed; each CTest run
  passed 3/3 including the C17 and C++20 ABI executables.
- `git diff --check` and `git diff --cached --check` — passed.

## Commits

- `ff9fcee` — `feat(runtime): define VoiceEngine C ABI v1`

## Self-review

- The header exports no C++ types, platform handles, allocator ownership,
  dynamic strings, exceptions, `size_t`, `long`, or C/C++ `bool` values.
- The only concurrency permitted beside factory work is nonblocking metrics
  snapshotting during a single-caller process invocation; control calls remain
  serialized.
- The approved plan checkbox was not modified, and no future-task implementation
  was introduced.

## Concerns

- Native ABI checks ran on macOS arm64 only. The header uses Windows export and
  calling-convention branches, but an actual Windows compiler/runner remains a
  Task 18 CI acceptance gate.
