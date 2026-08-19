# VoiceEngine C ABI v1 contract

The normative public header is `runtime/api/voice_engine.h`. Canonical version
and error values are generated from JSON into
`core/contracts/include/ai_voice_contracts/generated_contracts_c.h`. Windows
plugin implementations define `AIVS_VOICE_ENGINE_BUILD` to export the sole
factory symbol. This contract defines no loader, Runtime, engine, or device.

## Structure and version rules

All public structures are append-only and start with `struct_size` and
`abi_version`; all reserved fields are input-zero/output-zero. Unless an
operation says otherwise, a v1 input or output prefix must have
`abi_version == api.abi_version` and a `struct_size` covering every v1 field it
uses. Nested request/result/buffer structures obey the same exact selected-
version rule. An unknown enum/format/layout/reset-reason value, nonzero
reserved field, mismatched prefix, or insufficient known structure size is
`AIVS_ERROR_INVALID_ARGUMENT`.

The v1 structures are intentionally flag-free: a field that has no stable v1
meaning is reserved or omitted and can be appended only in a future ABI.

## Factory negotiation

`aivs_voice_engine_get_api(request, out_api)` is the only plugin export.
`request.minimum_abi_version` and `maximum_abi_version` are the caller's
inclusive supported range; `request` itself uses the current v1 structural ABI
and has zero reserves. The plugin applies the exact overlap algorithm:

1. Reject malformed ranges (zero or minimum greater than maximum) with
   `AIVS_ERROR_INVALID_ARGUMENT`.
2. Compute `lower = max(caller_minimum, plugin_minimum)` and
   `upper = min(caller_maximum, plugin_maximum)`.
3. If `lower > upper`, return `AIVS_ERROR_UNSUPPORTED_VOICE_ENGINE_ABI`.
4. Otherwise select `upper`, the highest common version. A Runtime therefore
   supplies its full supported range once; it does not guess/retry individual
   versions.

`out_api->struct_size` is caller-owned writable table capacity on entry, and
must be at least `AIVS_VOICE_ENGINE_API_V1_SIZE`; `out_api->abi_version` is
zero on entry. A larger capacity is valid and receives only the selected v1
prefix. On success, the plugin returns exactly
`struct_size == AIVS_VOICE_ENGINE_API_V1_SIZE`, the selected `abi_version`,
zero reserves, and all eight v1 pointers non-null. `aivs_voice_engine_api_v1_is_complete`
is the header-owned check for those v1 invariants.

If the table capacity is smaller than the v1 complete-table size, the factory
returns `AIVS_ERROR_BUFFER_TOO_SMALL`; a capacity smaller than the eight-byte
prefix is `AIVS_ERROR_INVALID_ARGUMENT` and cannot be safely modified. On any
other factory failure, a table with a writable prefix is cleared to
`struct_size == 0`, `abi_version == 0`, null function pointers, and zero
reserved fields; no bytes outside the supplied capacity are written. A capacity
smaller than the v1 table is cleared only within its supplied capacity. A caller
reinitializes the output table before retrying. The static helpers in the public
header expose the mandated selection and v1-capacity checks for contract tests.

## Buffers and PCM

All pointers are caller-owned, valid only for the duration of the call, and
never retained by the plugin. A byte view's `data` is NULL exactly when its
`size_bytes` is zero. A mutable byte buffer's `data` is NULL exactly when its
`capacity_bytes` is zero. UTF-8 is length-delimited and may contain NUL bytes.

Phase 0.5 PCM is IEEE-754 binary32 float, interleaved. For nonzero frames,
`samples` is non-NULL; for zero frames it is NULL. Zero-frame processing is
valid and succeeds with zero output frames after normal lifecycle/format
validation. `sample_rate_hz` and `channel_count` are positive. Input format and
layout equal `AIVS_PCM_FORMAT_FLOAT32` and `AIVS_PCM_LAYOUT_INTERLEAVED`.

Before accessing PCM, implementations must validate exact multiplication
`frame_count * channel_count * 4` (or output capacity equivalent) without
overflow and reject bytes not representable in the process address space. The
public helper `aivs_voice_engine_pcm_byte_count_is_addressable` captures this
numeric rule; implementations must also validate their actual accessible range.
Only exactly in-place PCM (`input.samples == output.samples`) may overlap; any
other overlapping byte range is `AIVS_ERROR_INVALID_ARGUMENT`.

For `process_audio`, output `samples` and `frame_capacity` are caller input.
On success the plugin writes the input rate/channel/format/layout, sequence,
and sample-time to output, leaves capacity unchanged, sets both
`frames_written_or_required` and `processed_frame_count` to input frame count,
and returns the current stream generation. Input metadata and generation must
match the prepared stream. If capacity is insufficient, no PCM or output
metadata is changed and `frames_written_or_required` alone reports input frame
count; `processed_frame_count` and result generation are zero.

## Lifecycle, outputs, and errors

| State | Allowed calls | Success transition |
| --- | --- | --- |
| No handle | factory, initialize | Initialized |
| Initialized | info, load model, metrics, shutdown | Model loaded or invalidated |
| Model loaded | info, prepare stream, metrics, shutdown | Prepared or invalidated |
| Prepared | process, metrics, reset, shutdown | Model loaded or invalidated |

`initialize` is valid from No handle only and returns a non-NULL opaque handle
only on success; on every failure `result.engine` is NULL. `load_model` is not
a replacement call. `prepare_stream` returns exact algorithmic latency in
output PCM frames and a nonzero stream generation; failure zeros both. The
generation identifies the prepared state. `reset` requires one of
`CALLER_REQUEST`, `DISCONTINUITY`, or `RECOVERY`, clears stream history, returns
to Model loaded, and returns an incremented generation; failure returns zero.
Failed load leaves the handle Initialized; failed prepare leaves it Model
loaded with no prepared stream; failed process and reset leave the prior
Prepared state unchanged (apart from the observable error counter).

`get_engine_info` is all-or-nothing across both string buffers. If either
capacity is insufficient, it writes no identifier bytes, reports each required
byte count in its corresponding `written_or_required_bytes`, and returns
`AIVS_ERROR_BUFFER_TOO_SMALL`. Other failures write no bytes and set both counts
to zero. Successful metrics writes a coherent nonblocking snapshot; failure
zeros every counter. No error writes beyond caller capacity or leaves partial
metadata/data visible. Output structural prefixes and caller storage descriptors
remain valid for retry.

`shutdown` has no output. A successful shutdown destroys and invalidates the
handle; it is not callable again. A failed shutdown leaves the handle fully
live, owned by the plugin, and retryable: the plugin must not partially release
handle resources before returning failure. Invalid live-state transitions return
`AIVS_ERROR_INVALID_STATE`. For valid calls, argument/prefix/reserved/format/
arithmetic/overlap validation precedes lifecycle validation, which precedes
capacity validation, which precedes engine-internal errors; this is the v1 error
precedence.

## Concurrency and lifetime

The Runtime serializes factory and initialize calls per module. A plugin must
support multiple sequentially initialized independent handles; calls on separate
handles may proceed concurrently. Per handle, control calls are serialized, one
caller may process, and no control call overlaps processing. Only metrics may
overlap processing and it is a nonblocking atomic-counter snapshot. After a
successful prepare, processing performs no allocation, blocking lock, I/O,
logging, environment lookup, or exception/panic propagation.

The Runtime pins the plugin module and its returned API table/function pointers
until there are no live handles and no in-flight calls. It may unload only after
that point; plugins may assume their table and code remain valid throughout
every call. No dynamic error string or free-form logging crosses the realtime
path.
