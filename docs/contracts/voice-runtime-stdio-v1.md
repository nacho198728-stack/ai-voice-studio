# Voice Runtime stdio command contract v1

## Scope

`voice-runtime` is a C++20 isolated process. Standard input and standard output
carry only RuntimeMessage v1 frames. Standard error is reserved for bounded
diagnostics. Phase 0.5 does not load a VoiceEngine, access audio devices, parse
models, send PCM/model bytes, or implement restart behavior.

The process writes one Hello frame before reading standard input. All JSON below
is compact UTF-8 with the shown key order, no insignificant whitespace, and
base-10 locale-independent unsigned integers. Runtime and protocol versions
come from the generated canonical contracts.

## Payloads

- Hello (`request_id=0`, `command=None`, `Success`):
  `{"runtime_version":"<semver>","protocol_version":<u32>,"generation":<u64>,"health":"starting"}`.
  The initial process generation is `1`. After the frame is built, the session
  enters `running/healthy`.
- Ping request/response: the response is a byte-for-byte echo of the request
  payload, including empty and non-UTF-8 payloads. RuntimeMessage limits both
  successful sides to 256 bytes; no command parser retains a second copy after
  dispatch completes.
- GetCapabilities request: empty. Its successful response is compact JSON:
  `{"platform":"<macos|windows|linux|unknown>","architecture":"<arm64|x86_64|unknown>","runtime_version":"<semver>","protocol_version":<u32>,"backend":"unavailable","engine":"unavailable"}`.
  This is process/build identity only, not a hardware, device, benchmark, or
  broader capability model.
- RunMockPipeline: parameters remain opaque bounded control bytes. Until Task 8
  installs Mock, every valid request returns `EngineUnavailable` with
  `{"error":"engine_unavailable"}`. No pipeline work occurs.
- Shutdown request and successful response: empty. The Runtime enters stopping
  before returning the response; the adapter writes and flushes the entire
  response, marks the session stopped, and exits without dispatching later
  frames from the same read.
- Session-state errors use `{"error":"runtime_unavailable"}` or
  `{"error":"runtime_shutting_down"}` and the canonical matching ErrorCode.
  Invalid inbound Hello/Response has no contract-valid correlated response, so
  it is a protocol failure and produces no extra stdout bytes.

Every response copies the valid request's `request_id` and `command` exactly.
All output is validated by the shared RuntimeMessage encoder before writing.

## State and exit contract

The session states are `starting`, `running`, `stopping`, `stopped`, and
`error`. Its snapshot contains the process generation, state, health, and exit
reason. Exit reasons are `none`, `clean_eof`, `requested_shutdown`,
`protocol_failure`, `output_failure`, and `unexpected_failure`; generation is
never advanced by this process skeleton. These values leave an explicit future
recovery seam without implementing restart.

Process exit values are:

- `0`: clean EOF with no partial frame, or requested shutdown after the success
  response was fully written and flushed;
- `2`: malformed, oversized, unsupported, truncated, or wrong-direction input;
- `3`: stdout full-frame write or flush failure;
- `4`: stdin failure, codec/output construction invariant failure, stdio setup
  failure, or unexpected exception.

The native adapter derives its 2,048-byte read size from the shared 32-byte
header and 64-message feed cap. That keeps every feed below both the shared
65,568-byte byte ceiling and the message-count ceiling, even when minimum-size
frames are sticky. The decoder remains the sole frame-retention owner and
retains at most one contract-valid frame. On Windows, stdin/stdout are switched
to binary mode. On POSIX, SIGPIPE is ignored so a closed stdout pipe is reported
as exit `3` rather than terminating asynchronously.
