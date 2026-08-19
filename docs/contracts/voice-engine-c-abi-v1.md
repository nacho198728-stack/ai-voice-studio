# VoiceEngine C ABI v1 contract

The public source of this ABI is `runtime/api/voice_engine.h`; canonical
version and error values are generated in
`core/contracts/include/ai_voice_contracts/generated_contracts_c.h`.
When building a Windows plugin implementation, define
`AIVS_VOICE_ENGINE_BUILD` so its sole factory symbol receives export visibility.

## Calling and failure rules

Initialize every public input/output structure with its `AIVS_*_INIT` macro or
zero it before setting its `struct_size` and `abi_version`. A caller may pass a
larger structure in a compatible revision; implementations inspect only fields
covered by the reported size. Reserved fields are zero on input and output.

Every operation returns a canonical `aivs_error_code_t`. For a failed call, an
implementation sets writable count results (`written_or_required_bytes`,
`frames_written_or_required`, and `processed_frame_count`) to zero before
validation, except `BufferTooSmall`, which reports the required capacity and
does not write output data. It never writes data beyond caller capacity. No
engine allocation may become caller-owned, and no exception or panic may cross
the ABI.

String and byte views are `(data, size_bytes)`, can contain embedded NUL bytes,
and are valid only for the call. Result strings use a caller-provided mutable
byte buffer. PCM samples are caller-owned IEEE-754 binary32 floats.

## Lifecycle

| State | Allowed operation | Successful transition |
| --- | --- | --- |
| No handle | factory query, `initialize` | Initialized |
| Initialized | `get_engine_info`, `load_model`, `get_metrics`, `shutdown` | Model loaded, or handle invalid after shutdown |
| Model loaded | `get_engine_info`, `prepare_stream`, `get_metrics`, `shutdown` | Stream prepared, or handle invalid |
| Stream prepared | `process_audio`, `get_metrics`, `reset`, `shutdown` | Model loaded after reset, or handle invalid |

`load_model` is not a replacement operation: calling it outside Initialized,
including after a prior successful load, returns `AIVS_ERROR_INVALID_STATE`.
`prepare_stream` is valid only after a model load. `reset` is valid only for a
prepared stream and returns to Model loaded. A successful `shutdown` releases
the opaque handle; it is not idempotent because no call, including another
shutdown, may dereference the invalidated handle. Invalid live-state
transitions return `AIVS_ERROR_INVALID_STATE`.

## Phase 0.5 processing

`prepare_stream` fixes float32 interleaved sample rate, channel count, and a
maximum input frame count. A following `process_audio` must match that format,
layout, rate, channels, and maximum frame count. `sequence` and `sample_time`
are explicit metadata; the engine must reproduce them in successful output.

For this Phase 0.5 contract, a successful process call produces exactly one
output frame for every input frame. If output capacity is insufficient, it
returns `AIVS_ERROR_BUFFER_TOO_SMALL`, sets
`frames_written_or_required` to the input frame count, leaves PCM output
unchanged, and sets `processed_frame_count` to zero. This deterministic count
rule makes the future Mock Engine testable without defining an Audio Engine.

## Concurrency

The Runtime serializes all control calls (`initialize`, `get_engine_info`,
`load_model`, `prepare_stream`, `reset`, and `shutdown`) for a handle. At most
one `process_audio` caller may use a handle. No control call may overlap a
process call. Only `get_metrics` may overlap processing, and it must return a
nonblocking snapshot suitable for atomically maintained counters. The realtime
process path has no allocation, blocking lock, I/O, logging, environment lookup,
or escaping exception after stream preparation succeeds.
