# Task 15 report — Rust RuntimeMessage contract gate

## Status

Complete on macOS arm64. The Rust workspace now independently pins the frozen
RuntimeMessage v1 wire layout, every generated policy and ErrorCode mapping,
streaming and fatal-state behavior, and all generator outputs. The audit also
found and fixed one concrete bounded-retention defect shared by the Rust and
C++ decoders. Both now collect the untrusted header in a 32-byte inline array
and lazily create a fixed-size boxed frame array only after a complete non-empty
header passes every validation. The fixed logical storage is exactly
`MAX_FRAME_BYTES`, independent of container/allocator capacity behavior.

RuntimeMessage v1, its schema, shared golden fixtures, the main plan checkbox,
and all ADR files were left unchanged. ADR-003 was not created.

## Requirement-to-test map

| Required outcome | Concrete evidence |
| --- | --- |
| Exact 32-byte little-endian header and limits | `pins_the_exact_v1_header_layout_limits_and_little_endian_bytes` uses an independently written byte array to pin magic, wire version, kind, zero flags, protocol version, `u64` request ID, command, zero reserved field, signed `i32` ErrorCode bytes, payload length, and payload. It also pins schema/header/global payload/frame/input/message limits to `1/32/65536/65568/65568/64`. |
| All canonical ErrorCodes and signed wire handling | `pins_every_canonical_error_code_and_its_signed_wire_field` independently lists all 12 name/value/category mappings, checks generated reverse mapping, writes each value through a legal response, inspects bytes 24..28, and decodes it. Existing malformed-frame coverage rejects `42`, `-1`, `i32::MIN`/`0x80000000`, and `i32::MAX`; the high-bit frame is read as signed and rejected as unknown. |
| Every generated command policy and legal combination | `pins_and_exercises_every_generated_policy_boundary` compares `POLICIES` with an independent literal 13-row matrix, then encode/decodes each row at its exact payload maximum, rejects maximum+1, and flips its zero/nonzero request-ID rule. `rejects_every_illegal_kind_command_and_error_rule_combination` exhausts the 3 kinds x 5 commands x 2 success classes and permits exactly the 13 legal rows. |
| Hello/request/response and illegal encode inputs | The exhaustive matrix covers kind/command/error invariants; every legal row also exercises its request-ID invariant and limit. `enforces_command_limits_and_message_invariants_on_encode` retains command-specific, Hello, success/error response, zero/nonzero request ID, and incompatible protocol `0/2` rejection checks. Typed Rust enums prevent constructing unknown kind/command/ErrorCode values safely; decoder mutation tests cover their raw-wire unknown forms. |
| Golden exact bytes shared with C++ | `consumes_the_shared_literal_fixture_and_matches_the_encoder` and `consumes_literal_fixtures_for_every_kind_and_command` decode all checked-in `runtime-message-v1-*.hex` files to literal Rust messages and require exact Rust re-encoding. `all_literal_kinds_and_commands` in the C++ contract executable consumes the same fixture directory and also requires exact re-encoding. |
| Every representative split and coalescing shape | `decodes_minimum_and_maximum_frames_at_every_split_point` checks every split from 0 through frame length for a 32-byte Hello and a 65,568-byte maximum frame. `fixed_storage_is_lazy_reused_and_released_without_capacity_assumptions` feeds the maximum body one byte at a time. Sticky tests decode two coalesced body frames while proving one fixed-storage creation is reused. |
| Bounded retention and oversize-before-allocation | Rust uses `Option<Box<[u8; MAX_FRAME_BYTES]>>`; C++ uses `unique_ptr<array<uint8_t, kMaxFrameBytes>>`. The 32-byte header is inline and no fixed storage exists after 31 bytes, any invalid complete header, any oversized declaration, or a valid bodyless Hello. A legal non-empty header creates one fixed object, maximum bytewise and repeated/coalesced frames reuse it, and fatal/reset release it. Tests assert only fixed logical storage bytes (`0` or exactly `MAX_FRAME_BYTES`) and monotonic creation count, never `Vec`/`vector` capacity. |
| Feed byte/message bounds and atomicity | Existing resource tests accept exactly 64 messages, reject 65, reject 65,569 input bytes, and release fixed frame storage while returning no messages. `every_fatal_class_is_sticky_until_reset_and_discards_batch_prefixes` covers malformed, unsupported, payload-too-large, input-too-large, and message-count failures. A call containing valid frames followed by an invalid frame returns only `Err` in Rust and an empty `messages` vector plus the same error in C++; the schema's per-feed atomicity rule therefore matches both APIs. |
| Fatal/reset and EOF | The fatal-class test proves the original error repeats from both `feed` and `finish` until explicit reset, then verifies normal decoding resumes. `distinguishes_clean_and_truncated_eof` keeps empty EOF clean and makes partial header/body EOF terminal `MalformedFrame/Truncated`. |
| Allocation/error atomicity | Rust has no stable deterministic allocator-failure injection at this boundary; bounded validation/error atomicity is exercised instead. C++ deterministically injects failure at fixed-storage creation, payload materialization, first result growth, and later result growth after a valid prefix. Every case returns terminal `InternalError/AllocationFailure`, no messages, zero fixed storage, and requires reset. |
| Canonical generator drift | `check detects drift in every generated protocol artifact without rewriting it` now corrupts each of the five Rust/C++/C generated outputs in isolation and requires read-only `check` failure without repair. Schema validator mutation tests retain frozen layout, discriminants, policy matrix, fatal-error, allocation, and limit checks. `pnpm contracts:generate` produced no repository changes and `pnpm contracts:check` passed. |

## RED/GREEN evidence

- Baseline focused Rust protocol tests passed 9/9, confirming the existing
  codec behavior while exposing missing independent matrix/split/drift gates.
- Rust bounded-retention RED: the new maximum-frame bytewise test failed at
  `decoder.buffered_capacity() <= MAX_FRAME_BYTES`; geometric `Vec` growth had
  retained more than one legal frame's capacity.
- C++ bounded-retention RED: the matching maximum-frame bytewise assertion
  aborted the focused CTest at the same capacity bound.
- The provisional `reserve_exact`/`reserve` GREEN only demonstrated current
  macOS allocator/libc++ behavior. Independent review correctly rejected it:
  both APIs guarantee a capacity lower bound, not the claimed upper bound.
- Fix-round RED: new Rust and C++ tests required fixed logical storage byte and
  creation-count APIs, and both failed to compile because the vector-backed
  decoders did not provide allocator-independent storage semantics.
- Fix-round GREEN: inline header arrays plus lazy fixed boxed arrays satisfy the
  bound structurally. Rust passed 17/17; focused C++ passed in Debug/Release,
  including the four revised allocation-failure cases; full CTest passed 23/23
  in both configurations.
- Policy, ErrorCode, exact-byte, split, fatal/reset, and per-output generator
  additions protect already-correct frozen behavior and passed immediately; no
  false implementation RED is claimed for those coverage-only additions.

## Files

- Rust decoder: `core/contracts/src/runtime_message.rs`.
- C++ decoder: `core/contracts/include/ai_voice_contracts/runtime_message.hpp`.
- Rust contract gate: `core/contracts/tests/runtime_message.rs`.
- C++ bounded-retention parity: `tests/contract/runtime_message_cpp_test.cpp`.
- Generator drift gate: `tools/scripts/contracts.test.mjs`.
- Evidence: `.superpowers/sdd/2026-08-19-phase-0-5-infrastructure-v1/task-15-report.md`.

## Verification

- Focused Rust protocol suite — passed 17/17.
- Focused C++ RuntimeMessage CTest — passed in Debug and Release; Release keeps
  its translation-unit assertions active under `NDEBUG`.
- Fresh Debug configure/build/CTest — passed 23/23.
- Fresh Release configure/build/CTest — passed 23/23.
- `pnpm contracts:generate` — completed with no generated-file changes.
- `pnpm contracts:check` — passed; generated mappings are current.
- Generator behavior tests — passed 6/6.
- Root `pnpm test` — passed: contracts 6/6, doctor 7/7, native staging 3/3,
  bundle verification 2/2, and desktop Vitest 8/8.
- `cargo fmt --all -- --check` — passed.
- `cargo check --workspace --locked` — passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` — passed.
- `cargo test --workspace --locked` — passed; CTest-owned native fixture cases
  remain intentionally ignored by generic Cargo and passed in both CTest runs.
- `git diff --check`, staged diff check, plan-preservation check, and ADR-003
  absence check — passed before the implementation commit.

## Commits

- `b71bcea` — `test(contracts): close RuntimeMessage Rust gate`.
- `8323766` — `docs: record RuntimeMessage contract gate evidence`.
- `e78770f` — `fix(contracts): use fixed RuntimeMessage storage`.
- Fix-round report updates are committed separately because `.superpowers/` is
  intentionally ignored by normal repository rules.

## Limitations

- Native execution evidence is macOS arm64 only. Windows x64/MSVC remains the
  independent Task 18 CI/acceptance gate.
- Stable Rust does not expose the deterministic allocation-failure seam used by
  the C++ test executable. Rust's fixed frame object has a statically known
  logical length, but process allocation exhaustion itself is not injected or
  claimed.
- This task did not add or change IPC transport, RuntimeManager, Tauri, audio,
  engine, model, Python, cloud, ABI, or protocol/schema semantics.

## Review fix round 1

- Removed every decoder/resource assertion based on Rust `Vec::capacity()` or
  C++ `std::vector::capacity()`. No cross-allocator physical-capacity claim
  remains.
- Rust now uses a `[u8; HEADER_SIZE]` inline header plus lazy
  `Option<Box<[u8; MAX_FRAME_BYTES]>>`; construction uses safe boxed-slice to
  boxed-array conversion and no `unsafe`.
- C++ uses the same state model with an inline header array and lazy
  `unique_ptr<array<uint8_t, kMaxFrameBytes>>` inside the existing `bad_alloc`
  boundary.
- A partial, invalid, oversized, or bodyless header performs no fixed-storage
  creation. The first legal non-empty header creates exactly one logical fixed
  object; bytewise, repeated, and coalesced frames reuse it. Fatal/reset release
  it, while a lifetime creation counter makes reuse and post-reset reallocation
  observable without inspecting allocator internals.
- The C++ allocation matrix now covers every allocation site and the important
  prefix-atomicity shape: fixed storage, payload vector, first result vector,
  and a later result-vector growth after one valid frame. All return no messages,
  release fixed storage, lock the original allocation error, and recover only
  after reset.
- Fresh fix-round verification passed: Rust protocol 17/17, Debug and Release
  CTest 23/23 each, generator 6/6, root pnpm tests, Cargo fmt/check/clippy/test,
  and diff/plan/ADR-003 checks. RuntimeMessage schema and wire bytes were not
  changed.
