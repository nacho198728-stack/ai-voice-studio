# ADR-002: Isolate the C++ Runtime behind a stable VoiceEngine C ABI

- Status: Accepted
- Date: 2026-08-19

## Context

VoiceEngine implementations are dynamically loaded native code that may fail
while loading, validating state, or processing buffers. Those failure modes
must not share the Tauri/Rust control process. Independently built Runtime and
engine modules also cannot safely exchange C++ standard-library objects,
exceptions, or allocator ownership across compiler and platform boundaries.

Phase 0.5 now implements and tests this path:

`Rust RuntimeManager -> RuntimeMessage IPC -> C++ voice-runtime -> VoiceEngine C ABI -> Mock VoiceEngine`

The Mock implementation proves loading, lifecycle, processing, reset, metrics,
errors, and packaging. It is not an Audio Engine or AI model. ADR-003 remains
reserved for the Phase 1 Audio Engine decision.

## Decision

Run native engine code only inside the separately supervised C++ Runtime.
`runtime/api/voice_engine.h` is the stable pure-C VoiceEngine ABI v1. Each plugin
exports exactly one symbol, `aivs_voice_engine_get_api`, which negotiates the
canonical version and fills a versioned function table with eight operations:
initialize, shutdown, get engine info, load model, prepare stream, process
audio, reset, and get metrics.

The ABI uses fixed-width integers, `struct_size` and `abi_version` prefixes,
zeroed reserved fields, caller-owned buffers, explicit lengths/capacities, and
an opaque plugin-owned handle destroyed by successful shutdown. UTF-8 is
length-delimited. Every operation returns a canonical integer error; no C++
exception or Rust panic may cross the boundary.

The Runtime owns dynamic-library loading and serializes control operations per
handle. After `prepare_stream`, one caller at a time may call `process_audio`;
that path must not allocate, block, perform I/O, log, read environment state,
or throw. `get_metrics` may run concurrently only through nonblocking atomic
snapshots.

Phase 0.5 supplies a deterministic Mock plugin and MockPipeline. The pipeline
processes a fixed 128-frame stereo buffer and transports only an 80-byte summary
with frame count, checksum, and metrics over RuntimeMessage. PCM never leaves
the native process.

## Rationale

Process isolation contains library crashes and malformed native behavior below
the control plane. A small append-only C ABI avoids compiler, STL, exception,
and allocator compatibility problems while remaining usable from C and C++ on
macOS and Windows. Explicit ownership and sizes make adversarial validation
possible. The prepare/process split establishes a realtime-compatible boundary
without prematurely choosing the Phase 1 device/stream architecture.

The deterministic Mock makes the contract executable: dynamic loading, exact
transforms and checksums, reset semantics, metrics, error handling, simulated
work, and unique exports are verified without introducing AI or audio devices.

## Alternatives considered

### Rust FFI directly to engine plugins

This would load untrusted/native engine failure modes into the control process
and weaken the process boundary established by RuntimeManager.

### C++ virtual interface or shared C++ SDK

Virtual tables, STL types, RTTI, exceptions, and allocator choices are not a
portable ABI across compiler versions and platforms.

### One process per engine behind another IPC protocol

The C++ Runtime already supplies the required process isolation. A second
process protocol would add lifecycle and serialization complexity before there
is evidence it is needed.

### Define a device or Audio Engine API now

Device selection, clocks, buffering, realtime threads, and recovery belong to
Phase 1. The current PCM structure is a plugin data contract, not permission to
open devices or accept ADR-003 early.

## Consequences

- Rust and Tauri never load, store, or dereference a VoiceEngine library or
  handle; only `voice-runtime` does so.
- Runtime and plugins can be compiled independently while preserving one tested
  ABI on macOS and Windows.
- ABI evolution is append-only. Existing fields, operations, versions, and
  canonical error values cannot be silently reinterpreted.
- The plugin owns its opaque handle; callers own all strings, PCM, and output
  buffers. Shutdown, error, and buffer-capacity behavior must remain explicit.
- Native integration, export inspection, adversarial compile fixtures, and
  deterministic benchmark tests are required compatibility gates.
- The Mock's processing and latency controls are test infrastructure only; they
  do not claim product audio, model loading, inference quality, or performance.
- ADR-003 must separately decide the Phase 1 Audio Engine and may not weaken the
  process, ABI, realtime, or no-PCM-over-IPC guarantees recorded here.
