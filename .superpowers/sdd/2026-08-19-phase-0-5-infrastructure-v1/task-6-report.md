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

## Review fix round 1

- Added an explicit inclusive factory request range and deterministic
  highest-overlap selection rule. The table now has separate entry capacity
  (`struct_size`), exact success shape (`AIVS_VOICE_ENGINE_API_V1_SIZE`), an
  explicit selected ABI version, zero reserved fields, and eight required
  non-null pointers. Header-only contract helpers let C tests cover too-small
  and larger capacity, no overlap, and a complete in-test v1 function table
  without implementing or loading a plugin.
- Added caller-owned `aivs_prepare_stream_result_t` (exact algorithmic latency
  in output frames plus stream generation) and `aivs_reset_result_t`; reset now
  takes a closed set of explicit reasons and returns incremented generation.
  Undefined v1 flags and feature flags were removed.
- Expanded the focused ABI document with all-or-nothing failure states,
  multi-buffer info capacity semantics, failed-shutdown retry ownership,
  pointer/zero-frame/no-retention/overflow/overlap PCM rules, deterministic
  error precedence, factory/initialize serialization, independent-handle
  concurrency, and module/table lifetime requirements.
- Added the shared 64-bit v1 layout manifest, compiled by both real C17 and
  C++20 contract executables. It freezes size, alignment, and every field
  offset for all public structures and all eight table slots; language-specific
  assertions freeze every function pointer type. Float checks now cover storage
  size, radix, precision, and exponent range in the header and both tests.
- ADR-002 now carries Accepted status/date and a rationale aligned with
  ADR-000's isolation, shared-contract, and explicit-boundary principles.

### Review-fix RED/GREEN evidence

- RED: the shared layout manifest and C/C++ tests were added before the new
  factory/prepare/reset ABI definitions. Debug build failed as expected with
  missing `aivs_voice_engine_factory_request_t`,
  `aivs_prepare_stream_result_t`, and `aivs_reset_result_t`, alongside layout
  drift for the removed speculative fields.
- GREEN: after the minimal ABI/header changes, both public-header executables
  compile and run. They validate exact 64-bit layout, factory range/capacity
  behavior, complete/non-complete stub tables, and zero/maximum/overflow PCM
  byte arithmetic without introducing an engine implementation.

### Review-fix commit

- `22425f1` — `fix(runtime): complete VoiceEngine ABI v1 contract`

## Review fix round 2

- Added canonical immutable `voice_engine_abi.v1_version` and generated C/Rust/
  C++ mappings. All v1 initializers, factory request structure version, and v1
  completeness use it rather than mutable current version.
- Factory capacity is now an explicit argument. A public bounded-clear helper
  defines exact failure writes for every error class; C canary tests cover null,
  zero, below-prefix, prefix, mid-slot, v1-minus-one, v1, and larger capacity.
- Added header-owned process failure, generation advance, pointer/count,
  range-compatibility, and capacity helpers. Tests cover future `[1,2]` to v1
  fallback, v1-vs-v2 table shape, result reset behavior, generation exhaustion,
  exact/partial/disjoint overlap, wrap, and undersized capacity.
- The ABI contract now makes all process failures atomic, generations monotonic
  and non-reusing, and accessible-storage responsibility caller-owned. ADR-002
  now matches per-module factory/initialize serialization and independent
  handles.

### Review-fix commit

- `4ca0e78` — `fix(runtime): freeze VoiceEngine ABI v1 contract`

## Review fix round 3

- Factory output capacity is now unambiguously explicit; bounded clearing uses
  byte canaries including true mid-pointer capacity 12, and full capacity gets
  typed NULL assignments while partial output must be discarded/reinitialized.
- Process failure normalization now requires explicit outer/nested capacities
  and writes only fully covered fields. The contract restores complete lifecycle
  transitions, latency units, info/metrics/prepare/reset failure rules, and
  deterministic error precedence.
- Generator validation keeps frozen v1 independent of the active compatibility
  floor; tests generate/check a v1=1, active=[2,2] fixture. The supported flat
  64-bit address model and numeric wrap helper are explicit.

### Review-fix commit

- `c375177` — `fix(runtime): harden VoiceEngine ABI failure contract`
