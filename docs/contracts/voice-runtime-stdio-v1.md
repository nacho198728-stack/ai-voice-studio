# Voice Runtime stdio command contract v1

## Scope

`voice-runtime` is a C++20 isolated process. Standard input and standard output
carry only RuntimeMessage v1 frames. Standard error is reserved for bounded
diagnostics. Phase 0.5 can load the deterministic Mock VoiceEngine through the
frozen C ABI, but does not access audio devices, parse real models, send
PCM/model bytes, or implement restart behavior.

After command-line/plugin setup succeeds, the process writes one Hello frame
before reading standard input. All JSON below is compact UTF-8 with the shown
key order, no insignificant whitespace, and base-10 locale-independent unsigned
integers. Runtime and protocol versions come from the generated canonical
contracts.

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
  broader capability model. Without a plugin, backend and engine are
  `unavailable`; after successful Mock setup they are `mock` and
  `aivs-mock-v1`.
- RunMockPipeline request: exactly empty. Non-empty control bytes return
  `InvalidArgument` with `{"error":"invalid_mock_pipeline_request"}`. Without a
  plugin, the response remains `EngineUnavailable` with
  `{"error":"engine_unavailable"}`.
- RunMockPipeline success: exactly 80 little-endian bytes, with no PCM. Offsets
  are: schema version `u32` at 0 (value 1), result size `u32` at 4 (80), frames
  `u32` at 8, channels `u32` at 12, checksum `u64` at 16, Runtime-measured
  elapsed microseconds `u64` at 24, exact algorithmic latency frames `u64` at
  32, processed stream generation `u64` at 40, then cumulative process calls,
  input frames, output frames, and process errors as `u64` at 48, 56, 64, and
  72. Phase 0.5 uses 128 frames, two channels, and zero algorithmic latency.
  Checksum is FNV-1a 64 over each output float's IEEE-754 bits serialized least
  significant byte first; the fixed input and sign-bit-flip Mock transform
  produce `0x3ecd5190f6f4f725`.
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
  failure, invalid CLI/setup, plugin load/negotiation/initialization failure, or
  unexpected exception. Setup failures occur before Hello, emit a bounded
  diagnostic only to stderr, and write zero stdout bytes.

## Mock plugin setup

`voice-runtime` accepts no implicit plugin source. The optional CLI is
`--plugin <absolute-path>` plus optional
`--mock-work-iterations <0..1000000>` in either order. Unknown, duplicate,
missing, relative, or malformed arguments are rejected; configuration is never
read from environment variables or a search path. The Mock configuration sent
to the plugin is the strict UTF-8 form `{"work_iterations":N}`. The simulated
model contract is identifier `mock-v1` with an empty model-data view.

The Mock accepts float32 interleaved PCM at 8–192 kHz, one or two channels, a
nonzero stream id, and 1–4096 maximum frames. It precomputes state at prepare,
flips only the sign bit of each float during process (including exact in-place),
and performs optional bounded deterministic CPU work. The hot call allocates
nothing, takes no blocking lock, and performs no I/O, logging, clock, or
environment access; Runtime measures time around it.

The native adapter derives its 2,048-byte read size from the shared 32-byte
header and 64-message feed cap. That keeps every feed below both the shared
65,568-byte byte ceiling and the message-count ceiling, even when minimum-size
frames are sticky. The decoder remains the sole frame-retention owner and
retains at most one contract-valid frame. On Windows, stdin/stdout are switched
to binary mode. On POSIX, SIGPIPE is ignored so a closed stdout pipe is reported
as exit `3` rather than terminating asynchronously.
