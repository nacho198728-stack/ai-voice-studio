# Task 9 report — RuntimeMessage v1 and bounded IPC framing

## Status

Complete. This task defines and implements the transport-neutral wire codec
only. It does not start a process, perform Runtime I/O, dispatch commands,
load an engine, or implement Mock behavior. The approved plan checkbox was
not modified.

## Wire decisions

- `core/contracts/runtime-message-v1.json` is the canonical machine-readable
  source. The existing contract generator validates it and emits deterministic
  Rust/C++ constants and enum mappings; read-only drift checking covers both
  new outputs.
- RuntimeMessage v1 uses a fixed 32-byte header followed by `payload_length`
  control bytes. Every integer is encoded explicitly in little-endian order;
  native structs, alignment, and host endianness are never serialized.
- Header layout: `AVRM` magic (0..4), `wire_version:u16` (4),
  `message_kind:u8` (6), zero flags (7), `protocol_version:u32` (8),
  `request_id:u64` (12), `command:u16` (20), zero reserved (22), canonical
  `ErrorCode:i32` (24), and `payload_length:u32` (28).
- Kinds are `Hello=1`, `Request=2`, and `Response=3`. Commands are
  `None=0` (Hello only), `Ping=1`, `GetCapabilities=2`,
  `RunMockPipeline=3`, and `Shutdown=4`. The validator freezes every v1
  assignment; renumbering requires a new wire version.
- Hello requires request ID zero, command None, and Success. Requests and
  responses require a nonzero request ID and a Phase 0.5 command; requests
  require Success, while responses carry a canonical success/error code and
  echo their request command. Correlation with the original request remains
  the later RuntimeManager's stateful responsibility.
- The global control payload ceiling is 65,536 bytes and maximum frame size is
  65,568 bytes. Hello is capped at 1,024 bytes, Ping at 256 bytes, error
  diagnostics at 4,096 bytes, GetCapabilities requests and successful Shutdown
  messages are empty, and other control results stay under the global ceiling.
  `run_mock_pipeline` owns parameters/result summaries only; PCM, model bytes,
  and audio buffers are forbidden.
- The request ID, error class, and payload limit for all 13 legal
  kind/command/success-or-error combinations live in a structured schema
  policy matrix. Rust and C++ validation consume generated tables; neither
  codec carries a hand-written command policy. In particular, successful Ping
  payloads use the 256-byte limit while Ping error responses use the generic
  4,096-byte error limit.
- Decoders collect only the bytes still needed for one legal frame. They
  validate the complete header before accepting body bytes, so an oversized
  advertised length is rejected before body allocation. Fatal errors clear
  partial bytes, lock the decoder in a failed state, and repeat the original
  canonical framing error until explicit reset. A feed containing a fatal
  frame returns no messages from that call.
- Each feed accepts at most 65,568 input bytes and returns at most 64 messages.
  Sticky frames remain supported within those generated limits. Batch overflow
  is terminal `FrameTooLarge`; fatal paths release partial/output capacity
  rather than retaining a large allocation.
- C++ decoder allocation failure is caught at the boundary and becomes stable
  terminal `InternalError/AllocationFailure`. The partial frame, accumulated
  output, and remainder of the current input are discarded; reset is required
  before reuse. Signed ErrorCode wire bits use C++20 `std::bit_cast`, including
  malformed high-bit values.
- Empty EOF is clean; partial header/body EOF is `MalformedFrame`. Bad magic,
  kind, command, flags, reserved values, request/error invariants, and unknown
  ErrorCodes are `MalformedFrame`; wire/protocol mismatches are
  `UnsupportedProtocolVersion`; size violations are `FrameTooLarge`.
- C++ stdout is reserved exclusively for framed protocol bytes. Diagnostics
  and logs must use stderr or files. The codec itself performs no I/O.
- Payloads are opaque command-owned control bytes in v1. This avoids a C++ JSON
  dependency; a later command contract may require UTF-8 JSON without changing
  the envelope.

## Files

- Canonical schema and shared literal fixtures:
  `core/contracts/runtime-message-v1.json`,
  `core/contracts/fixtures/runtime-message-v1-*.hex`. Fixtures independently
  pin Hello, Request, Response and every v1 command discriminant.
- Generated cross-language values:
  `core/contracts/src/runtime_message_generated.rs`,
  `core/contracts/include/ai_voice_contracts/runtime_message_generated.hpp`,
  plus generator/validator updates in `tools/scripts/contracts.mjs`.
- Rust codec/API: `core/contracts/src/runtime_message.rs` and export wiring in
  `core/contracts/src/lib.rs`.
- C++20 dependency-free codec/API:
  `core/contracts/include/ai_voice_contracts/runtime_message.hpp`.
- Tests: `core/contracts/tests/runtime_message.rs`,
  `tests/contract/runtime_message_cpp_test.cpp`, generator tests, and CTest
  wiring.

## Red-green evidence

- Initial RED: `node --test tools/scripts/contracts.test.mjs` failed because
  `runtime_message_generated.rs` did not exist and malformed RuntimeMessage
  schemas were not validated.
- Initial RED: `cargo +1.97.1 test -p ai-voice-contracts --test runtime_message`
  failed with unresolved import `ai_voice_contracts::runtime_message`.
- Initial RED: Debug build of `aivs_runtime_message_cpp_test` failed because
  `ai_voice_contracts/runtime_message.hpp` did not exist.
- Refinement RED: Rust and C++ failed-state tests observed `InvalidState`
  instead of the required locked original `FrameTooLarge`; implementations
  were changed to retain the original fatal framing error.
- Refinement RED: C++ legal-then-fatal sticky input still returned the earlier
  message; the decoder was changed to make each fatal feed atomic, matching
  Rust behavior.
- GREEN: Rust protocol integration tests pass 7/7, C++ RuntimeMessage CTest
  passes in Debug and Release, generator behavior tests pass 6/6, and the
  shared literal fixture is independently consumed by both languages.
- Review round 1 RED: schema generation rejected the new structured policy and
  feed limits until validator/renderers supported them; Rust/C++ tests failed
  to compile without generated batch constants, `BatchTooLarge`, and capacity
  observability.
- Review round 1 RED: C++ deterministic allocation tests failed to compile
  without `AllocationFailure`; test-local `new` injection covers header growth,
  body growth, payload materialization, and result-vector growth.
- Review round 1 RED: C++ high-bit test failed to compile without the constexpr
  bit-pattern decoder. GREEN uses `std::bit_cast<int32_t>` and rejects
  `0x80000000` as an unknown canonical ErrorCode.
- Review round 1 GREEN: bounded many-frame/limit+1/fatal-capacity tests,
  structured policy mutation tests, all literal fixtures, four allocation
  failure points, and high-bit malformed decoding pass in both applicable
  language suites.

## Verification

- `cargo +1.97.1 fmt --all -- --check` — passed.
- `cargo +1.97.1 check --workspace` — passed without warnings.
- `cargo +1.97.1 test --workspace` — passed: Rust tests 11/11, doctests clean.
- `pnpm contracts:check` — passed; generated mappings are current.
- `pnpm test` — passed: contract generator 6/6 and doctor 7/7.
- `cmake --fresh --preset native-debug`, Debug build, and Debug CTest —
  passed 4/4.
- `cmake --fresh --preset native-release`, Release build, and Release CTest —
  passed 4/4. RuntimeMessage assertions remain active under `NDEBUG`.
- Malformed, exact-limit/limit+1, oversize-before-body, chunking, sticky,
  truncation, failed/reset, and shared-fixture cases all passed in the language
  suites.
- `git diff --check`, staged diff check, and plan-preservation check — passed.

## Commits

- `3d97018` — `feat(contracts): add bounded RuntimeMessage v1 framing`
- `f862c52` — `docs: record RuntimeMessage framing evidence`
- `a65ac02` — `fix(contracts): harden RuntimeMessage framing bounds`
- This report is recorded in a separate evidence commit because `.superpowers/`
  is intentionally ignored by the normal repository rules.

## Self-review

- The schema is the numeric authority; Rust and C++ constants/enums are
  generated and drift checked rather than duplicated manually. All v1
  discriminants and every legal command policy are mutation-tested.
- Both codecs use explicit byte reads/writes and the same validation order,
  limits, failed-state semantics, golden fixture, and canonical ErrorCode
  mapping.
- The decoder never reserves from an unvalidated advertised payload length and
  retains at most one legal frame. Total input/output work per feed is also
  explicit and bounded. Exact maximum frames, message batches, and
  maximum-plus-one failures are covered.
- Rust drops its local output Vec on fatal `Err`; C++ swaps error outputs and
  partial buffers with empty vectors, yielding zero retained capacity. C++
  allocation failure follows the same terminal/reset discipline without an
  allocation in the cleanup path.
- No Runtime/process/Mock/VoiceEngine/Tauri/logging/config/audio/AI/ONNX/Python
  behavior or external C++ dependency was introduced. ADR files and the plan
  checkbox are untouched.

## Concerns

- Native execution evidence is macOS arm64 only. Real Windows x64/MSVC
  validation remains the explicit Task 18 CI gate.
- Response command echo is structurally required by the message contract, but
  verifying it against the originating request requires RuntimeManager
  correlation state and remains Task 10 work.
