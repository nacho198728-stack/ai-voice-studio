# Task 7 report — isolated C++20 voice-runtime process skeleton

## Status

Complete on `codex/phase-0-5-infrastructure`. The implementation adds the real
`voice-runtime` executable, a transport-neutral session/command core, and a
bounded stdio adapter. It does not load a plugin, call the VoiceEngine ABI,
process audio/model bytes, implement Mock behavior, restart, logging, Tauri, or
the broader capability model. The approved plan checkbox was not modified.

## State, payload, and exit decisions

- `SessionSnapshot` carries generation, health, exit reason, and explicit
  `starting/running/stopping/stopped/error` state. The native process starts at
  generation 1. Hello is built from canonical generated Runtime/protocol
  versions while health is `starting`, then the session enters
  `running/healthy`.
- Ping echoes at most the contract's 256 bytes. GetCapabilities returns only
  platform, architecture, canonical versions, and truthful unavailable
  backend/engine fields. RunMockPipeline always returns canonical
  `EngineUnavailable`. Shutdown transitions to stopping before returning an
  empty success response; only a completed write+flush transitions to stopped.
- All fixed structured payloads are compact deterministic UTF-8 JSON assembled
  without a general parser or locale-sensitive formatting. Exact layouts for
  Task 10 are in `docs/contracts/voice-runtime-stdio-v1.md`. RuntimeMessage
  continues to own validation and payload ceilings; no PCM/model/audio bytes
  are introduced.
- Valid request responses copy request ID and command exactly. Inbound Hello or
  Response cannot receive a contract-valid correlated response, so it produces
  no extra stdout frame and terminates as a protocol failure. Requests after
  stopping begins are not dispatched by the adapter; the core's deterministic
  state rejection is `RuntimeShuttingDown`.
- Exit 0 means clean EOF or fully flushed requested shutdown; 2 means malformed,
  oversized, unsupported, truncated, or wrong-direction protocol input; 3
  means stdout write/flush failure; 4 means stdin/setup/allocation/invariant or
  unexpected failure. Diagnostics are fixed and capped at 512 bytes on stderr.
- Native reads are derived from the shared minimum-frame and 64-message feed
  limits (2,048 bytes). This safely supports partial/sticky input, including
  more than 64 frames over multiple feeds, while the decoder alone retains at
  most one legal frame. Windows stdin/stdout use binary mode; POSIX ignores
  SIGPIPE so closed stdout maps to exit 3.

## Files

- Runtime core/process: `runtime/session/**`, `runtime/process/**`,
  `runtime/main.cpp`, and `runtime/CMakeLists.txt`.
- Contract documentation: `docs/contracts/voice-runtime-stdio-v1.md`.
- Native tests and CMake wiring: `tests/runtime/**` and
  `tests/CMakeLists.txt`.

## RED/GREEN evidence

- Initial RED: fresh Debug generation failed because the referenced
  `voice-runtime` target did not exist. GREEN: after the minimal targets/core/
  adapter were added, session, stdio, and real executable smoke tests passed.
- Sticky-bound RED: 65 valid minimum Ping frames plus Shutdown in one transport
  read caused `BatchTooLarge`. GREEN: the adapter read size now derives to
  2,048 bytes and the full sequence yields Hello, all 65 responses, and the
  flushed Shutdown response.
- Allocation RED: deterministic decoder allocation failure was incorrectly
  mapped to protocol exit 2. GREEN: it is now an unexpected internal process
  failure (exit 4), while malformed/oversized/unsupported input remains exit 2.
- Tests exercise real `Session` code with literal RuntimeMessage requests. The
  focused CMake smoke launches the real process for EOF and Shutdown, validates
  every stdout byte through the shared decoder, requires empty normal stderr,
  uses a five-second bound, and leaves no `voice-runtime` process alive.

## Verification

- Fresh Debug configure/build/CTest: 7/7 passed.
- Fresh Release configure/build/CTest: 7/7 passed, including active assertions
  in session, stdio, and child-process smoke tests.
- `cargo +1.97.1 fmt --all -- --check`: passed.
- `cargo +1.97.1 check --workspace`: passed without warnings.
- `cargo +1.97.1 test --workspace`: passed, 11/11 Rust tests; doctests clean.
- `pnpm contracts:check`: passed; generated mappings current.
- `pnpm test`: passed; contract generator 6/6 and doctor 7/7.
- `git diff --check` and staged diff check: passed. Runtime outputs exist in
  both Debug and Release build trees; `pgrep -x voice-runtime` found no orphan.

## Commits

- `11d8867` — `feat(runtime): add isolated voice runtime process`

## Self-review

- `main` owns no command semantics. Session, codec loop, and native I/O are
  separate and independently testable; stdout has no text/log API use.
- Each output frame is encoder-validated, completely written, and flushed.
  Shutdown returns immediately after its success response, so later decoded
  sticky frames are ignored rather than dispatched.
- No version literal is duplicated in production code. The only architecture/
  platform branching reports compile-target identity and performs no probing,
  benchmark, device, engine, or model access.
- The task changes only Runtime code, focused contract documentation, and
  native tests/build wiring. ADRs, generated contracts, ABI, Rust, pnpm files,
  and the plan remain untouched.

## Concerns

- Native execution was verified on macOS arm64 only. Windows binary-mode and
  MSVC branches are implemented but require the real Windows x64 Task 18 CI
  gate before cross-platform acceptance.
