# VoiceEngine C ABI v1 contract

`runtime/api/voice_engine.h` is the normative C17/C++20 API. Canonical
versions/errors are generated from JSON; `AIVS_VOICE_ENGINE_ABI_V1_VERSION` is
immutable, while `CURRENT_VERSION` is a moving preferred version. This contract
does not implement a loader, Runtime, engine, or device.

Every extensible structure is append-only, starts with `struct_size` and
`abi_version`, and has zero reserved fields. V1 initializers always use the
immutable v1 constant. After negotiation, nested v1 structures use the selected
v1 ABI exactly; unknown values, bad prefixes, and nonzero reserves are
`INVALID_ARGUMENT`.

## Factory

The sole export is `aivs_voice_engine_get_api(request, out_api,
out_api_capacity_bytes)`. The factory request structure always has ABI v1,
even when a future host offers `[1,2]`; its min/max fields are the caller range.
The plugin selects `min(caller_max, plugin_max)` when it is at least
`max(caller_min, plugin_min)`, otherwise returns `UNSUPPORTED_VOICE_ENGINE_ABI`.
Thus a `[1,2]` host accepts a v1-only plugin without retrying malformed v2
structures.

Capacity is explicit and is never read from `out_api`. Success requires enough
capacity for `AIVS_VOICE_ENGINE_API_V1_SIZE`, returns that exact size and the
selected ABI, and populates all eight non-null pointers and zero reserves.
`aivs_voice_engine_api_is_complete_for_version` validates the selected
version-specific table shape, so v1 bytes cannot be called v2.

On every factory failure (null/malformed request, invalid range, no overlap, or
too-small capacity), a non-null output is cleared by writing exactly
`min(out_api_capacity_bytes, v1_size)` zero bytes. Null/zero capacity writes
nothing; below-prefix, prefix-only, mid-slot, v1-minus-one, v1, and larger
capacities use the identical bounded rule. Too-small capacity returns
`BUFFER_TOO_SMALL` when the request is otherwise valid.
For partial clears, callers must discard the bytes and reinitialize before any
typed field read: all-zero bytes are not portable null function pointers. With a
full table capacity the factory additionally assigns every pointer to real C
`NULL`, producing a portable typed empty table. Entry `struct_size` is ignored.

## PCM, output, and generation rules

All buffers are caller-owned, length-delimited, never retained, and valid only
for their call. Input PCM `samples` is NULL exactly when `frame_count` is zero;
mutable output `samples` is NULL exactly when `frame_capacity` is zero. The
caller guarantees accessible storage. The callee validates fixed-width
arithmetic and uintptr range wrap only; it cannot portably inspect allocation
bounds. Exact in-place PCM is valid, partial overlap is invalid, and disjoint
ranges are valid. Zero-frame calls are valid after normal state/format checks.

V1 PCM is interleaved IEEE-754 binary32. Processing success writes exactly one
output frame per input frame, copies input metadata, returns the current
generation, and sets both counts to input frame count. On `BUFFER_TOO_SMALL`,
PCM/metadata are unchanged, required frames is reported, and processed/generation
are zero. Every other non-success also leaves PCM/metadata byte-for-byte
unchanged and zeros all three counts/generation; the header helper implements
that reset rule. The helper accepts declared outer/nested writable capacities:
below the outer output prefix it writes nothing; otherwise it normalizes only
fully covered scalar fields. Malformed short outputs are discarded/reinitialized,
not typed-read after failure.

Each handle owns a monotonically increasing epoch: every successful prepare and
reset advances it, never reuses it while live, and returns it. At `UINT64_MAX`,
prepare/reset return `INVALID_STATE` and preserve state. Stale or mismatched
process generations return `INVALID_STATE`.

## Lifecycle, concurrency, ownership

| State | Allowed calls | Success transition | Other calls |
| --- | --- | --- | --- |
| No handle | factory, initialize | Initialized | no handle operation is callable |
| Initialized | info, load model, metrics, shutdown | Model loaded / invalid | all others `INVALID_STATE` |
| Model loaded | info, prepare, metrics, shutdown | Prepared / invalid | all others `INVALID_STATE` |
| Prepared | process, reset, metrics, shutdown | Prepared/Model loaded/invalid | all others `INVALID_STATE` |

Initialize returns a non-null handle only on success; failures null it. Load
failure preserves Initialized, prepare failure preserves Model loaded, and
process/reset failure preserves Prepared. Info is all-or-nothing across both
buffers; metrics failures zero counters. Successful shutdown destroys the
handle; failed shutdown leaves it fully live and retryable.

`prepare_stream` is non-idempotent: success preallocates/warmups and advances
generation. Its `algorithmic_latency_frames` is exact engine algorithmic delay
in output PCM frames, excluding host/device/buffering latency; failure zeros
latency and generation. Reset is non-idempotent, clears stream history, returns
Model loaded, advances generation, and zeros its result on failure. Info buffer
shortage writes neither byte buffer and reports both required sizes; other info
failure writes no bytes and zeros both counts. Process is repeatable only for
the current generation. Metrics is repeatable; shutdown is non-idempotent.

Deterministic error precedence is: pointer/prefix/reserved/version/enum/format/
arithmetic/overlap, then lifecycle/generation, then capacity, then engine error.
The supported address model is flat 64-bit Windows x64/macOS arm64: the caller
owns accessible-storage truth; the callee uses numeric `uintptr_t` range/wrap
and overlap checks under that model.

The Runtime serializes factory and initialize per module. Independent handles
may run concurrently; per handle controls serialize, one process caller is
allowed, and only nonblocking metrics may overlap process. After prepare,
process performs no allocation, blocking lock, I/O, logging, environment lookup,
or escaping exception/panic. The Runtime pins module/table/function pointers
until no handle or in-flight call remains, then may unload.
