# Phase 0.5 architecture

## Scope and status

Phase 0.5 implements an offline desktop control plane, an isolated native
Runtime, a stable plugin boundary, and a deterministic Mock VoiceEngine used by
contract, integration, and benchmark gates. It proves lifecycle and data-plane
boundaries without implementing product audio or inference. Windows x64 support
is encoded in build and fixture paths, but real Windows execution is pending
and not accepted until the checked-in CI workflow runs on a GitHub runner.

The implemented control chain is:

```text
Tauri webview
  -> five typed commands
  -> Rust CommandService / RuntimeManager actor
  -> RuntimeMessage v1 over child stdin/stdout
  -> C++ voice-runtime Session
  -> MockPipeline
  -> dynamic loader
  -> VoiceEngine C ABI
  -> Mock VoiceEngine
```

The Mock pipeline returns only a fixed 80-byte summary containing frame counts,
metrics, and a checksum. PCM remains inside the native process.

## Repository responsibilities

| Directory | Responsibility |
| --- | --- |
| `apps/desktop` | Tauri composition root, five-command adapter, bounded public DTOs, frontend presentation, resource resolution, and close coordination |
| `apps/runtime-host` | Rust RuntimeManager actor, native process supervision, RuntimeMessage correlation, timeouts, generation tracking, capability observations, and reap ownership |
| `core/contracts` | Canonical versions, error codes, and RuntimeMessage schema plus generated Rust/C++ artifacts |
| `core/config` | Strict read-only schema-v1 parsing, semantic validation, and debug policy |
| `core/capability` | Privacy-minimal capability normalization and public profiles |
| `core/telemetry` | Rust JSONL telemetry initialization and redaction boundary |
| `runtime` | C++ executable, framed stdio Session, logging/options, Mock pipeline, dynamic loader, and pure-C VoiceEngine ABI |
| `engines/mock` | Deterministic test plugin implementing VoiceEngine ABI v1 |
| `audio` | Reserved Phase 1 ownership boundary only; no implementation |
| `tests` | C/C++ contracts, process/runtime integration, fixtures, and benchmark gate |
| `tools` | Contract generation, doctor, staging, bundle verification, CI and documentation fixtures |
| `docs` | Current architecture, contracts, development workflow, and accepted decision history |

`backend` and `core/model-package` remain reserved boundaries with no Phase 0.5
product behavior.

## Desktop trust boundary

The webview has exactly five input-free native commands: start, status,
capabilities, Mock pipeline, and stop. It receives stable bounded DTOs and
public error codes, never raw IPC, native stderr, host paths, process IDs, PCM,
or arbitrary payloads. There is no generic shell, filesystem, dialog, network,
clipboard, updater, or plugin command.

The explicit Tauri window accepts only the packaged local origin, or the exact
loopback Vite origin in development. CSP blocks network connections, media,
frames, objects, and forms; an independent navigation guard rejects external or
lookalike origins. Devtools are disabled. The frontend owns presentation state
only and never duplicates lifecycle authority.

Tauri owns one CommandService and telemetry guard. Its first exit request is
held while bounded shutdown completes; subsequent requests join the same close
operation. The Rust control process never loads or dereferences a VoiceEngine
plugin.

## Runtime process and lifecycle

RuntimeManager is a bounded Tokio actor with states `Stopped`, `Starting`,
`Connected`, `Stopping`, `Crashed`, and `Error`. It launches `voice-runtime`
using explicit executable/plugin/log arguments, clears inherited environment,
uses binary stdio on Windows, and ignores SIGPIPE on Unix. Startup succeeds only
after a valid Hello frame. Requests receive actor-minted correlation IDs,
deadlines, and generation ownership; unmatched, duplicate, oversized, malformed,
or stale replies cannot satisfy another request.

Stop is an idempotent join. Exactly one native Shutdown request owns a bounded
stop context, and concurrent Stop callers join its waiter set. Normal exit,
abnormal exit, timeout, hostile framing, stderr flood, manager drop, and actor
abort all preserve one invariant: accepted replies do not resolve or close
before the concrete child process is reaped.

The normal async path kills and awaits the child without blocking a Tokio
worker. If native wait operations repeatedly fail, process ownership, accepted
reply senders, and a cancellation-safe ReapBarrier move to an independent
standard thread. That reaper retries fail-closed with exponential backoff from
5 ms capped at 1 s and rate-limited fixed diagnostics until it proves the child
reaped. If thread creation itself fails, the current owner continues the same
bounded-backoff fail-closed loop. `ProcessResources` supplies the last-resort
Drop path; already-reaped normal paths are constant-time and do not double wait
or double kill.

Each successful start advances a generation. Capability observations and
pending replies are generation-bound, so a restart cannot surface stale state.

## RuntimeMessage IPC

[`voice-runtime-stdio-v1.md`](../contracts/voice-runtime-stdio-v1.md) defines the
binary protocol. Every frame has a 32-byte little-endian header, `AVRM` magic,
wire version 1, bounded message type, correlation ID, payload length, and
checksum. Payloads are at most 65,536 bytes; one feed accepts at most 65,568
bytes and 64 messages. stdout is protocol-only. stderr is bounded diagnostic
input to the host and cannot become a reply channel.

Implemented messages are Hello, Ping, GetCapabilities, RunMockPipeline, and
Shutdown. Native protocol failures use canonical bounded error payloads. The
host applies separate startup, request, shutdown, and kill/reap deadlines and
maps internal errors to stable public codes before anything reaches Tauri.

## VoiceEngine C ABI and plugin ownership

[`voice-engine-c-abi-v1.md`](../contracts/voice-engine-c-abi-v1.md) defines the
stable pure-C boundary. A library exports exactly
`aivs_voice_engine_get_api`; the returned v1 function table has eight
operations: initialize, shutdown, get engine info, load model, prepare stream,
process audio, reset, and get metrics. `struct_size`, `abi_version`, fixed-width
fields, zeroed reserved fields, and canonical integer errors support append-only
compatibility without sharing a C++ standard library or allocator ABI.

The Runtime owns the dynamic library and serializes control operations. The
plugin owns an opaque handle until successful shutdown; callers own every
buffer. UTF-8 is length-delimited. Exceptions must not cross the ABI. After
prepare, process audio must not allocate, block, perform I/O, log, inspect the
environment, or throw; metrics are nonblocking atomic snapshots.

The Mock VoiceEngine performs a deterministic sign-bit transform on float32
samples, supports bounded configurable simulated work, reset/generation
semantics, and exact calls/frames/errors/latency metrics. MockPipeline processes
128 stereo frames (256 samples) and sends only the 80-byte fixed summary over
IPC. It is verification infrastructure, not a voice-conversion model.

## Configuration, capability, errors, and logging

[`CONFIGURATION.md`](../development/CONFIGURATION.md) is the detailed policy.
The shipped `config/config.json` is strict schema v1: unknown, duplicate,
missing, wrong-type, invalid-path, unsupported, and out-of-range content fails
closed. Audio must remain disabled/unconfigured, the backend is Mock, resource
paths are absent, and trace/debug is clamped unless an explicit debug bit also
grants authority. There is no implicit cwd, PATH, home, environment, or system
plugin search.

Capabilities report only normalized platform, architecture, Runtime state,
backend, and Mock availability. They do not inspect CPU/GPU/NPU/RAM, hardware,
audio devices, drivers, benchmarks, or machine identity. Observations are
generation-bound and distinguish not evaluated, unknown, unavailable, and
available.

Errors are canonical integers internally and bounded stable DTOs at the webview
edge. The host writes `runtime-host.jsonl`; native code writes
`voice-runtime.jsonl` and stderr JSONL. stdout stays binary IPC. Debug authority
is validated twice, file-sink failure cannot corrupt stderr or IPC, and Phase
0.5 performs no log rotation or upload.

## Build, packaging, and verification

CMake/Ninja build Debug and Release native artifacts. Staging copies the exact
target-qualified sidecar, Mock library, and configuration into ignored Tauri
directories; release lookup uses only installed executable/resource locations,
while development uses compile-time repository paths. Runtime startup never
searches the current directory or PATH.

CTest proves ABI compilation/layout, unique exports, real dynamic loading,
RuntimeMessage framing, native Session behavior, process shutdown/reap, hostile
input, Mock metrics, and repeatable benchmark behavior. Rust integration tests
that need CMake artifacts are ignored under bare Cargo and run by CTest with
explicit fixture paths. JavaScript tests cover contract generation, doctor,
staging, bundle layout, CI structure, documentation drift, and frontend logic.

## Explicit exclusions and Phase 1 seam

Phase 0.5 has no audio device opening, no Audio Engine, no AI or model inference,
no Python runtime, no cloud/account service, no installer/signing/notarization,
and no user-facing voice-conversion workflow. The VoiceEngine PCM structures are
a plugin data contract, not a device API.

ADR-003 is reserved for the Phase 1 Audio Engine decision and intentionally does
not exist yet. Phase 1 may attach device/stream ownership to the `audio/`
boundary and feed prepared native buffers through the existing Runtime/plugin
seam. It must define realtime threading, buffering, clocking, device recovery,
permissions, and shutdown without moving engine loading into Tauri/Rust,
serializing PCM over RuntimeMessage, or weakening the v1 C ABI guarantees.

## Decision and contract map

- [ADR-000](../adr/ADR-000-monorepo.md): monorepo and top-level ownership.
- [ADR-001](../adr/ADR-001-tauri-rust-control-plane.md): Tauri/Rust control plane.
- [ADR-002](../adr/ADR-002-cpp-runtime-c-abi.md): isolated C++ Runtime and C ABI.
- [RuntimeMessage v1](../contracts/voice-runtime-stdio-v1.md): process protocol.
- [VoiceEngine C ABI v1](../contracts/voice-engine-c-abi-v1.md): plugin protocol.
- [Development guide](../development/DEVELOPMENT.md): reproducible commands.
