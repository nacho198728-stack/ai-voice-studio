# Task 10 report — actor-owned Rust RuntimeManager

## Status

Complete. `apps/runtime-host` now owns the isolated `voice-runtime` child
through one Tokio actor and exposes async start, stop, status, capability,
Ping, and Mock pipeline operations. The approved plan checkbox and ADRs were
not modified.

## Manager and lifecycle decisions

- `RuntimeManager` is a cloneable bounded command handle. Its actor exclusively
  owns `tokio::process::Child`, lifecycle/correlation state, and a bounded
  outbound queue. One actor-owned writer task exclusively owns ChildStdin and
  reports write completion/failure through the bounded process-event channel,
  so an OS pipe write can never block actor deadlines or exit observation. The
  final handle disappearing closes the command channel; the actor aborts the
  writer, kills, and waits for the child, with `kill_on_drop` as the teardown
  safety net.
- A dedicated stdout task feeds the shared bounded RuntimeMessage decoder in
  fixed 2,048-byte reads and sends decoded events through a bounded channel.
  A separate stderr task continuously drains into a capped newest-byte tail,
  so diagnostics cannot block protocol progress or enter stdout. Reap gives
  both readers a short bounded drain-to-EOF window with an absolute first-
  priority deadline and a separate 64-event stdout discard cap. Hitting the
  stdout cap aborts that reader while stderr/writer joining retains the
  remaining absolute window; task abort remains the final fallback.
- Configuration requires bounded absolute Runtime/plugin paths, rejects
  embedded NUL, zero/oversized timeouts, invalid queue/pending limits, excessive
  stderr retention, and Mock work beyond the native contract before spawn.
  Launch uses no shell or environment lookup and clears the inherited
  environment completely; explicit paths are sufficient on the currently
  verified platform, so no loader-affecting or secret-bearing variables are
  allowlisted. Windows uses `CREATE_NO_WINDOW`.
- States are `stopped`, `starting`, `connected`, `stopping`, `crashed`, and
  `error`. Status retains manager generation, live PID, canonical Hello,
  last exit/error, stderr tail, and an explicitly disabled finite restart-policy
  seam with observed manual restart count.
- Every spawn attempt receives a monotonic nonzero manager generation. Start
  publishes `connected` only after the first frame is a structurally valid,
  exact canonical Hello with Runtime/protocol version, generation `1`, and
  `health=starting`.
- Request IDs are monotonic and nonzero through `u64::MAX`; the following
  allocation fails explicitly. Pending calls are capped and correlate response
  ID plus exact echoed command. Unknown, duplicate, mismatched, malformed, or
  invalid-success responses invalidate the stream and force termination/reap.
- Handshake, stdin write, command response, shutdown response/exit, pending
  count, queues, frames, paths, and stderr are bounded. The writer queue has
  `max_in_flight + 1` slots, preserving one Shutdown slot even when every
  normal request is pending. The actor selects deadline, exit poll, process
  events, then ordinary commands in priority order. A timeout fails the
  expired caller with timeout context, fails other pending calls as unavailable,
  kills/reaps the child, and publishes `error` without stale correlation state.
- Shutdown owns a reserved correlation context independent of the normal
  pending bound. It enters `stopping`, rejects new normal requests, lets
  already-written requests settle, and publishes `stopped` only after the
  successful Shutdown response, ordered stdout EOF, clean child exit, bounded
  pipe drain, and reap. Process-exit observation never discards queued stdout.
  Repeated stop while stopped is idempotent; a stop during start cancels and
  reaps; stop/request/start state conflicts return deterministic errors.
  Unexpected idle exit publishes `crashed`, fails pending calls, and permits a
  later explicit manual start. No automatic or infinite restart was added.

## Typed payload contracts

- Hello and GetCapabilities use Serde with unknown-field rejection, exact
  compact serialization comparison (therefore no reordered/trailing bytes),
  and canonical Runtime/protocol/backend/engine validation.
- Mock success is exactly 80 little-endian bytes. Parsing validates schema and
  size, 128 frames, two channels, checksum `0x3ecd5190f6f4f725`, zero latency,
  nonzero stream/call generations, checked cumulative frame counts, zero
  process errors, and no trailing bytes.
- Ping remains bounded opaque bytes. No PCM, model data, C ABI/FFI, Tauri,
  logging/config framework, audio access, Python, or AI dependency was added.

## Files

- Runtime host API/actor: `apps/runtime-host/src/lib.rs`,
  `apps/runtime-host/src/manager.rs`.
- Typed payload parsing: `apps/runtime-host/src/payload.rs`.
- Pinned Rust dependencies: `apps/runtime-host/Cargo.toml`, `Cargo.lock`.
- Real-child integration and CTest wiring:
  `apps/runtime-host/tests/runtime_manager_process.rs`,
  `apps/runtime-host/tests/runtime_manager_fixture.rs`, `tests/CMakeLists.txt`,
  `tests/fixtures/runtime_manager_child_fixture.cpp`.

## Red-green evidence

- Initial RED: runtime-host library tests failed with unresolved
  `RuntimeManager`, state/status/error types, request ID/pending/state helpers,
  stderr tail, and all payload parsers. GREEN: 8 initial behavior tests passed
  after the minimum manager/payload implementation.
- Timeout refinement RED: the real-child deadline test unexpectedly received a
  successful Ping because a 5 ms periodic timeout check let the response win.
  GREEN: actor scheduling now selects the exact nearest deadline with deadline
  priority; the bounded 1,000,000-iteration Mock request times out, invalidates
  the stream, and proves the PID is reaped in both Debug and Release.
- Final Rust unit set passes 12 tests covering state transitions, configuration
  and NUL rejection, request-ID exhaustion, pending bound/correlation/
  duplicate/mismatch/timeout failure, command-specific success payloads,
  canonical JSON, fixed binary summary, and stderr retention.
- CTest executes five real-child cases using target-resolved native paths:
  start/Hello plus concurrent Ping/capability plus checksum pipeline plus clean
  stop, final-handle drop cleanup, idle crash detection and explicit restart,
  timeout invalidation/no-orphan, and bounded pre-Hello stderr diagnostics.
- Review-fix RED reproduced the reported failures with a BUILD_TESTING-only
  child fixture: inherited environment leaked, full-pending stop returned
  `Capacity`, stdout-close-then-hang exceeded the earlier handshake deadline,
  and response/exit ordering and completion barriers failed. GREEN covers 50
  immediate response-plus-exit shutdowns per run, handshake timeout/wrong
  pre-Hello/truncation, shutdown and request timeouts, stdout-close hang,
  full/cancelled pending stop, fatal correlation with a saturated event queue,
  final stderr drain, and an empty child environment. Each error-return test
  asserts the PID is already gone; timeout/protocol exit reasons are retained.
- Re-review RED established each remaining starvation path independently.
  Eight inherited-stdout descendants kept the bounded event channel hot and
  showed cleanup waiting for the drain timer without an independent work cap.
  A unit test filled a real OS ChildStdin pipe before dispatch and proved the
  synchronous actor write masked an already-existing 50 ms pending deadline
  beyond 500 ms. A Hello-then-no-read fixture drives maximum bounded pipe
  pressure using only the 64 allowed Ping requests. Finally, an exited owned
  child with a silent inherited stdout holder stayed `connected` for 750 ms
  under 256 concurrent status
  producers. GREEN uses the absolute/capped drain, dedicated bounded writer,
  and deadline → exit → event → command scheduling. All initiating
  errors still return only after PID disappearance and final state publication.

## Verification

- `cargo +1.97.1 fmt --all -- --check` — passed.
- `cargo +1.97.1 clippy --locked --workspace --all-targets -- -D warnings` —
  passed without warnings.
- `cargo +1.97.1 check --locked --workspace` — passed.
- `cargo +1.97.1 test --locked --workspace` — passed: 23 Rust unit/integration
  tests run, 0 failed; five native-path and 12 controlled-fixture cases are
  intentionally CTest-owned.
- `pnpm contracts:check` and `pnpm test` — passed: contract generation current,
  Node suites 13/13.
- Fresh Debug configure/build/CTest — passed 15/15, including RuntimeManager
  real-child and controlled-fixture tests.
- Fresh Release configure/build/CTest — passed 15/15, including timeout,
  ordered shutdown, environment isolation, final stderr, and no-orphan coverage.
- Debug real-child plus controlled-fixture CTests passed five consecutive runs;
  each controlled run includes 50 immediate shutdown races, continuous
  inherited stdout, blocked stdin, and 256-producer command starvation cases.
  A final process-table check found no fixture, Runtime, or controlled sleep
  process remaining.
- `git diff --check` and staged diff check — passed.
- Local Rust target inventory contains only `aarch64-apple-darwin`; Windows
  source coverage includes Tokio's Windows process API and
  `CREATE_NO_WINDOW`, but real MSVC execution remains Task 18.

## Commit

- `8f7ae60` — `feat(runtime-host): add actor-owned RuntimeManager`
- `9f77de9` — `fix(runtime-host): enforce ordered reap barriers`
- `6f43096` — `fix(runtime-host): prevent actor deadline starvation`

## Self-review

- Process, pending map, request IDs, transitions, and deadlines have one actor
  owner. The bounded writer task owns only ChildStdin; stdout/stderr/writer
  helpers report events but cannot mutate lifecycle/correlation state.
- Fatal protocol and timeout paths clear all pending calls before a later
  explicit start, and every cleanup path drops stdin, signals the child, waits,
  reaps, bounded-drains/joins pipe tasks, snapshots diagnostics, publishes
  state, and only then releases the initiating lifecycle reply.
- Codec policy remains the frame-size/direction authority; command parsers add
  only payload semantics. No protocol schema, native Runtime, Mock plugin, ADR,
  plan checkbox, desktop, logging, config/capability module, or audio code was
  changed.

## Concerns

- Execution evidence is macOS arm64 only. Windows x64/MSVC compile and process
  lifecycle validation remains the explicit Task 18 CI gate.
- Phase 0.5 exposes only the finite restart-policy seam and observed explicit
  restarts; it intentionally performs no automatic restart.
