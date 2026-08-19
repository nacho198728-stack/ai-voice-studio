# ADR-002: C++ Runtime to VoiceEngine C ABI

- Status: Accepted
- Date: 2026-08-19

## Context

VoiceEngine implementations may evolve independently from the desktop control
plane and may fail while loading or processing model data. The engine boundary
therefore belongs inside the isolated native Runtime, not in Rust or Tauri.
The required control and data path is:

`Tauri -> Rust RuntimeManager -> IPC -> C++ Runtime -> VoiceEngine C ABI -> Engine`

Phase 0.5 defines this boundary only. It does not load plugins, run a Runtime,
open audio devices, or implement an engine. ADR-003 remains reserved for the
Phase 1 Audio Engine decision.

## Decision

`runtime/api/voice_engine.h` is the stable, pure-C VoiceEngine ABI v1. A plugin
exports exactly one symbol, `aivs_voice_engine_get_api`, which negotiates the
canonical VoiceEngine ABI version and fills a versioned function table. The
table contains initialize, shutdown, get-engine-info, load-model,
prepare-stream, process-audio, reset, and get-metrics operations.

The canonical contract generator owns the ABI version and the fixed-width
error-code mapping for Rust, C++, and C. The engine range adds only
`InvalidArgument` (1301), `InvalidState` (1302), and `BufferTooSmall` (1303);
existing values remain unchanged.

## Rationale

Keeping engines behind the isolated C++ Runtime contains native model faults
outside the Tauri/Rust control plane. A language-neutral, append-only C ABI
lets independently compiled modules negotiate a stable version without sharing
a C++ standard-library or allocator ABI. Caller-owned buffers and fixed-width
results make ownership explicit, while the prepare/process contract preserves
the realtime path's no-allocation/no-blocking boundary. These choices carry the
same monorepo principles in ADR-000—reviewable shared contracts with explicit
module boundaries—into the Runtime-to-engine boundary.

## Consequences

- Engine implementations and the C++ Runtime can be built independently while
  retaining a portable ABI contract on macOS and Windows.
- The ABI intentionally cannot express C++ ownership, exceptions, STL objects,
  platform handles, or Rust/Tauri references. Every function returns the
  canonical `aivs_error_code_t`; exceptions and panics must be caught before
  crossing the boundary.
- Initial Phase 0.5 PCM is IEEE-754 float32, interleaved, with explicit sample
  rate, channel count, frame count/capacity, sequence, and sample-time fields.
  This is a data contract, not a device or Audio Engine implementation.
- ABI compatibility is append-only: every extensible structure and the API
  table lead with `struct_size` and `abi_version`; input reserved fields must
  be zero and implementations return output reserved fields as zero.

## Compatibility, ownership, and threading consequences

The Runtime owns plugin loading and calls the factory; no Rust or Tauri code
may load, store, or dereference this ABI. The plugin owns the opaque handle
until successful shutdown destroys it. All buffers, including UTF-8 strings,
PCM, and output capacity, remain caller-owned and use explicit byte or frame
counts. UTF-8 is length-delimited and never requires a NUL terminator.

The Runtime serializes factory and initialize calls per module; independent
handles may run concurrently. Control operations are serialized per live handle
and cannot overlap with processing. One caller at a time may invoke `process_audio` for a handle. After
successful `prepare_stream`, processing must not allocate, take a blocking
lock, perform I/O, log, read environment state, or propagate an exception.
`get_metrics` may run concurrently only with `process_audio`; it returns a
nonblocking best-effort atomic-counter snapshot.

## Alternatives considered

- **Rust FFI directly to engines:** rejected because it would put dynamic model
  code and native failure modes in the control process.
- **C++ virtual interface or a C++ SDK:** rejected because compiler, standard
  library, exception, and allocation ABI choices would leak across plugins.
- **IPC to each engine:** rejected for this phase because the Runtime already
  supplies process isolation; an in-process C boundary is the narrower plugin
  contract.
- **Stable concrete PCM/device API:** rejected because Phase 0.5 must not open
  audio devices or commit the Phase 1 Audio Engine design.
