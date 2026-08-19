# Task 11 report — unified structured logging

## Status

Complete. Rust RuntimeHost and the C++ Runtime now share a bounded UTF-8
JSONL logging contract while native stdout remains reserved for RuntimeMessage
frames. The approved plan checkbox, `progress.md`, and ADR files were not
modified; ADR-003 remains reserved for Phase 1.

## Contract and ownership

- Every record has RFC 3339 UTC `timestamp`, non-empty bounded `component`,
  lowercase `level`, and bounded `message`, with optional unsigned
  `request_id` and `generation`. Components are limited to 64 UTF-8 bytes and
  messages to 512 UTF-8 bytes. Call sites use fixed messages and do not include
  PCM, payload bodies, secrets, model contents, or resource paths.
- `core/telemetry` owns the process-global Rust `tracing` subscriber. It writes
  `runtime-host.jsonl` through a bounded `tracing-appender` worker and mirrors
  the same JSONL to stderr. Its custom formatter normalizes every direct
  `tracing` event into the closed schema, derives lowercase level from
  metadata, bounds formatting without first allocating unbounded strings, and
  discards unknown fields. Directory/file errors occur before global
  registration, repeated registration is typed and deterministic, and the
  returned guard drains accepted records on drop.
- The native Runtime owns an independent, non-global, synchronous spdlog
  logger. It appends `voice-runtime.jsonl`, mirrors JSONL to stderr, and never
  writes text to stdout. Logger construction and destruction are explicit;
  initialization is no-throw, errors are bounded/actionable, and destruction
  flushes both sinks. Invalid UTF-8 components fail before directory creation;
  malformed message sequences become U+FFFD before UTF-8-safe byte bounding.
- RuntimeManager emits actor lifecycle/request events without adding locks or
  changing selection/reap ordering. It explicitly passes the resolved log
  directory, validated logging policy, explicit debug-authority bit, and
  manager generation to each child. Rust telemetry cannot accept a bare level
  as authority; a disabled verbose request clamps to info. Native option
  parsing rejects missing debug authority and false+trace/debug before logger
  creation. The manager generation is used only for logging; the existing
  process-local Hello generation remains canonical `1`, so IPC is unchanged.
- `debug.development_log_directory` is a bounded portable relative path.
  RuntimeHost resolves it only against an explicit absolute host-owned base.
  Absolute, drive-qualified, backslash, empty, dot, parent, NUL, reserved
  characters/control characters, Windows device stems (including extension
  and superscript-digit forms), trailing dot/space, and oversized paths fail
  validation on every platform. Trace/debug is representable in logging
  authority only with `debug.enabled=true`.
- Rust uses Unicode `PathBuf`/`OsStr`; native Windows argument parsing retains
  wide paths and spdlog is compiled with `SPDLOG_WCHAR_FILENAMES`. No logging
  setting is inferred from the environment or current working directory.
- Mock `process_audio` has no logger dependency or call. Configure-time guards
  pin its complete target dependency surface; a SHA-256 contract pins the
  entire reviewed callback body and exact relaxed atomic updates. The dynamic
  test invokes success/failure paths six times under an OS-level stderr byte
  counter, asserts zero bytes, and then verifies exact process/frame/error
  metrics.

## Reproducible dependencies and licensing

- Rust exact pins: `tracing=0.1.44`, `tracing-subscriber=0.3.23`, and
  `tracing-appender=0.2.5`; package checksums are committed in `Cargo.lock`.
- Native exact pin: official spdlog 1.16.0 commit
  `486b55554f11c9cccc913e11a87085b2a91f706f`, fetched from the official GitHub
  commit archive with SHA-256
  `d2fef585c9879dd239dc498e2e8a1e22982b3ed67b2d14e78622b7ef25bdfdfa`.
  CMake does not discover or fall back to a system installation.
- `THIRD_PARTY_NOTICES.md` records tracing, spdlog, bundled fmt, source,
  versions, integrity pin, copyrights, and MIT terms.

## Files

- Rust telemetry: `core/telemetry/Cargo.toml`, `core/telemetry/src/lib.rs`,
  `core/telemetry/tests/telemetry.rs`, workspace manifests and lockfile.
- Configuration and host integration: `config/config.json`,
  `core/config/src/lib.rs`, config tests, `apps/runtime-host/src/manager.rs`,
  RuntimeHost integration tests and documentation.
- Native logging and process integration:
  `runtime/logging/include/ai_voice_runtime/logging.hpp`,
  `runtime/logging/runtime_logger.cpp`, Runtime options/stdio sources, root and
  Runtime CMake files.
- Native contracts: `tests/runtime/runtime_logging_test.cpp`,
  `tests/runtime/stdio_runtime_test.cpp`,
  `tests/runtime/voice_runtime_smoke.cmake`, and
  `tests/runtime/mock_process_audio_no_logging.cmake`.
- Documentation and attribution: root, config, Runtime, telemetry, RuntimeHost,
  and development READMEs plus `THIRD_PARTY_NOTICES.md`.

## Red-green evidence

- Configuration RED lacked a development-directory resolver and effective
  level gate. GREEN validates portable paths, Unicode and byte bounds,
  absolute host bases, disabled-debug clamping, and enabled trace passthrough.
- Rust telemetry RED failed with the crate absent. A subsequent schema RED
  exposed tracing's uppercase built-in level; GREEN disables that field and
  emits the shared lowercase field. File-failure RED found that
  `rolling::never` panicked; GREEN uses the fallible builder, returns a typed
  file error, and leaves global registration untouched.
- Native RED failed because `ai_voice::runtime_logging` and injected `LogSink`
  did not exist. GREEN covers schema/escaping/gating, optional correlation,
  append/flush, repeated isolated instances, and initialization recovery.
- Real-child RED exited at CLI parsing until log directory, level, and
  generation became required explicit inputs. Its first integration attempt
  also exposed that substituting manager generation into Hello broke the
  established protocol. GREEN keeps Hello generation `1` while logger records
  use the host generation.
- The real-child smoke cases decode stdout before inspecting stderr/file JSONL,
  exercise events before and during IPC, Unicode plugin/log paths, malformed
  CLI, missing/unsupported/failing plugins, and a blocked log directory.
  Every setup/logging failure keeps stdout empty.
- Review P1 RED proved native `--log-level debug` worked without debug
  authority. GREEN replaces raw Rust/native authority with validated policy,
  requires explicit `--debug-enabled`, rejects an inconsistent native pair in
  a real process, and proves disabled Rust debug requests filter helper and
  direct events.
- Review schema RED showed a direct `tracing` event could omit required fields
  and flatten arbitrary data. GREEN's schema formatter normalizes empty/
  oversized fields, supplies a fixed missing message, ignores secrets/unknown
  fields, admits only unsigned correlation, and keeps all records parseable.
- Review portability RED accepted `CON`; GREEN rejects the complete requested
  reserved-device set case-insensitively (including extensions), trailing
  dot/space, control characters, and Windows reserved punctuation independent
  of the build platform. Follow-up review added Windows' `COM¹/²/³` and
  `LPT¹/²/³` Unicode aliases to the same platform-independent rejection set.
- Review UTF-8 RED accepted an invalid native component. GREEN validates
  overlong, surrogate, out-of-range, lone-continuation, and truncated sequences;
  native tests parse every JSONL line and cover escaped controls plus exact
  accepted/rejected 512-byte multibyte boundaries.

## Verification

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed without
  warnings.
- `cargo check --workspace --all-targets` — passed.
- `cargo test --workspace` — passed: 36 Rust unit/integration tests plus four
  doctests, 0 failed; 19 native-path/controlled-fixture tests remain
  intentionally ignored by Cargo and are exercised through CTest.
- Debug configure/build/CTest — passed 17/17, including unified logging,
  stdout protocol isolation, real RuntimeManager child cases, hostile-child
  cases, and the Mock hot-path source guard.
- Release configure/build/CTest — passed 17/17 with the same coverage.
- `pnpm contracts:check` — passed; generated contracts are current.
- `pnpm test` — passed: Node contract and doctor suites 13/13.
- `git diff --check` and staged diff check — passed.
- Final process-table inspection found no `voice-runtime` or Runtime fixture
  process remaining.

## Commit

- `466b527` — `feat(logging): add unified structured telemetry`
- `9d99ac6` — `docs(logging): record task 11 evidence`
- `0d418eb` — `fix(logging): guarantee valid UTF-8 JSONL`
- `065a68c` — `fix(logging): enforce validated logging authority`
- `94bc56c` — `fix(config): reject superscript device aliases`

## Self-review and concerns

- The logging API does not enter the Mock engine target, no audio/model/UI/
  cloud work was added, and no log record crosses the binary IPC channel.
- Phase 0.5 deliberately has append-only development files with no rotation,
  retention, upload, or centralized redaction service. Call-site discipline is
  documented; production retention policy remains future work.
- C++ logging is synchronous because no audio callback exists in Phase 0.5.
  Phase 1 must define a realtime-safe telemetry transport before an audio hot
  path may log.
- Execution evidence is macOS arm64. Unicode/wide-argument and spdlog filename
  branches are source-covered, but real Windows x64/MSVC validation remains the
  Task 18 CI gate.
