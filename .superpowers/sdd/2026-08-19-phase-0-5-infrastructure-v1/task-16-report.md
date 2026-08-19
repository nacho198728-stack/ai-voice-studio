# Task 16 report — Runtime process integration gate

## Status

Complete on macOS arm64. The process-integration gate now combines the existing
real-child and controlled-child suites with a dedicated eleven-case gate for
the remaining hostile process scenarios. Review found and fixed two related
production teardown ownership defects. The main plan checkbox was not modified
and ADR-003 was not created.

Real Windows process execution remains Task 18; this report claims only source
and argument-construction coverage for Windows.

## Coverage audit and focused additions

Task 7, Task 10, and Task 13 already supplied most of the required evidence:

- the real built `voice-runtime` emits one compatible Hello, answers concurrent
  Ping/GetCapabilities and Mock pipeline calls, returns the canonical C ABI Mock
  summary, accepts Shutdown, closes stdout, exits successfully, and is reaped;
- controlled fixtures already cover pre-Hello EOF/truncation/wrong direction,
  handshake/request/shutdown timeout, post-connect EOF, wrong response command,
  blocked stdin, saturated queues, repeated/concurrent Stop, cancelled pending
  callers, manager-handle drop, command/event starvation, final stderr drain,
  remote errors, and explicit restart/capability staleness;
- native stdio/session tests already distinguish read/write/protocol exit codes,
  and runtime option plus desktop artifact tests cover wide Windows arguments,
  `.exe`/`.dll` layout, and Unicode path preservation.

The new `runtime_manager_gate` adds only the missing observable process cases:

1. A copied real Runtime and Mock plugin launch from Unicode executable/plugin
   paths, write to a Unicode log directory, complete Hello, capability, Unicode
   Ping correlation, ordered Shutdown, EOF, exit 0, and concrete-PID reap.
2. Pre-Hello bad magic, oversized Hello header, unsupported protocol version,
   and malformed canonical Hello payload all fail closed and return only after
   the captured PID is gone.
3. Unknown response ID, post-connect Request, duplicate Hello, and a closed
   child stdin writer poison the stream with the correct Protocol/Process class
   and return only after reap. Existing coverage retains wrong-command and
   post-connect EOF cases.
4. A valid response followed by a duplicate response invalidates the stream;
   malformed successful capability payload becomes an actor-bound
   `Inconclusive/Payload` observation only after process cleanup.
5. A 1 MiB stderr burst cannot block Hello or Ping, never enters stdout, and
   retains exactly the configured newest 127 bytes including a terminal marker.
6. A native exit code 7 maps to `Crashed`, `success=false`, concrete code 7, and
   a reaped PID. A generation-1 response flood cannot satisfy or poison a fresh
   generation-2 Ping/capability result after explicit restart.
7. Dropping the owning Tokio runtime aborts the internal manager actor while a
   manager handle remains alive; the child is synchronously killed and reaped,
   verified through its concrete PID.
8. Cross-runtime callers prove that an accepted Ping, capability observation,
   concurrent Stop waiters, and a command accepted into an undriven one-slot
   Actor queue cannot resolve or close before the exact child PID disappears.

Every new async case has a ten-second outer deadline, two-second bounded polls,
and a concrete-PID cleanup guard. The runtime-abort case installs its PID guard
before allowing the owner thread to drop the Tokio runtime. Review-round abort
cases retain and join the owner thread, and the queued case proves queue
acceptance before triggering abort. The Unicode test also has a directory
cleanup guard. CTest adds an independent 90-second process boundary.

## Production defect and fix

The first runtime-abort RED proved that Tokio's `kill_on_drop(true)` kills an
owned child but does not synchronously wait for it. On POSIX the concrete PID
remained observable as an unreaped zombie after the owning runtime aborted the
actor.

Review-round RED then proved a stricter production defect: Rust field/local
drop order closed an accepted Ping reply before the same PID was reaped. The
process could also be temporarily moved out of `Actor` across async waits, so an
Actor destructor could not impose the required reply barrier. Finally, the
initial fallback treated `try_wait` errors as completion and abandoned the
child after two seconds to Tokio's orphan queue even though that runtime could
already be gone.

`Actor::drop` now explicitly transfers the process, original command receiver,
pending/start/stop contexts, active status sender, and deferred accepted replies
to one `ActorAbortOwner`. It aborts pipe helpers, retries kill and nonblocking
wait without treating errors as reap, and only fails/drains replies after reap.
If the two-second synchronous window expires, the entire owner moves to an
independent `std::thread`; a failed thread spawn retains ownership and continues
reaping on the current thread. Normal async paths retain `ProcessResources` and
all reply contexts in Actor fields until wait and pipe drain complete.

`ProcessResources::drop` is an independent last defense with the same retry
semantics. Its normal already-reaped path is constant time. An unreaped child
that exceeds the bounded synchronous window is moved to its own standard-thread
reaper and never depends on a destroyed Tokio driver or orphan queue. A fake
reap driver covers initial/poll `try_wait` errors, `start_kill` errors, timeout
classification, and delayed completion ownership without platform-specific or
unsafe APIs.

## RED/GREEN evidence

- Initial gate RED: six of seven new cases failed. The real Unicode process
  chain passed immediately because the production path already supported it.
  Hostile fixture names otherwise behaved as normal children, so malformed,
  correlation, flood, nonzero-exit, and stale-generation assertions failed for
  the intended missing-fixture reason.
- Runtime-abort RED independently captured PID survival after Tokio runtime
  destruction. This was a production failure, not a fixture-only gap.
- Review-round RED captured an accepted Ping returning `ManagerClosed` while
  its concrete PID was still observable. This isolated reply/process ownership
  order from the earlier zombie-only failure.
- GREEN: the controlled child gained only deterministic modes needed by the
  tests, generation parsing from the actual manager argument, and no manager
  logic duplication. The dedicated gate passes 11/11 in Debug and Release.
- The four cross-runtime cancellation barriers passed ten consecutive focused
  gate runs. Three synchronous reaper tests cover error retry, honest timeout,
  and delayed detached-owner completion.
- The three focused RuntimeManager process suites passed five consecutive Debug
  runs: real child 5/5 each run, legacy controlled fixture 17/17 each run, and
  the new gate 7/7 each run.

## Requirement-to-evidence map

- **Real start/Hello/capabilities/correlation/stop/reap:** existing
  `runtime_manager_process` plus the new real Unicode gate; status retains
  manager generation, canonical Hello, requested-shutdown exit, no PID, and
  successful native exit only after reap.
- **Pre-Hello failures:** legacy handshake timeout, stdout EOF/hang,
  wrong-direction and truncation; new bad magic, oversized frame,
  wrong-version frame, and malformed Hello payload; real missing-plugin nonzero
  setup failure preserves bounded stderr.
- **Post-connect and correlation failures:** legacy crash/EOF and wrong command;
  new unknown/duplicate IDs, wrong direction, duplicate Hello, malformed
  capability payload, and concrete writer failure.
- **Timeout/stderr/native exits:** legacy request/shutdown/Hello deadlines and
  stdout drain starvation; new real 1 MiB stderr flood and explicit code-7 exit
  mapping.
- **No orphan before replies:** legacy stop, timeout, drop, concurrent Stop,
  desktop-exit race, and queue-starvation tests; new actor/runtime abort fallback
  plus accepted request/capability/Stop/queued-command reply barriers and
  per-case concrete PID guards. No process-name-only assertion is used as
  primary evidence.
- **Generation/restart:** legacy successful and failed capability observations
  reject stale generation 1 after restart; new queued generation-1 response
  flood is ignored and generation 2 returns fresh Ping/capability data.
- **Unicode/Windows:** real macOS Unicode executable/plugin/log arguments now run
  end-to-end. Existing native wide-argument tests and desktop artifact/staging
  tests cover Windows Unicode path parsing and `.exe`/`.dll` construction.
  Actual Windows launch, pipe behavior, termination, and reap are not claimed.

## Verification

- Review-fix Debug build and CTest: 24/24 passed.
- Review-fix Release build and CTest: 24/24 passed.
- Focused eleven-case RuntimeManager gate repeated ten times in Debug: all ten
  invocations passed.
- `cargo +1.97.1 fmt --all -- --check`: passed.
- `cargo +1.97.1 clippy --locked --workspace --all-targets -- -D warnings`:
  passed without warnings.
- `cargo +1.97.1 check --locked --workspace --all-targets`: passed.
- `cargo +1.97.1 test --locked --workspace`: 63 passed, 0 failed; native-path
  integration cases remain intentionally ignored here and run by CTest.
- `pnpm contracts:check`: passed with no generated drift.
- Root `pnpm test`: 26 passed, 0 failed.
- Final concrete process-table scan found no `voice-runtime` or
  `aivs_runtime_manager_child_fixture` process. No gate Unicode temp directory
  remained.
- `git diff --check`: passed.

## Files

- Production abort fallback: `apps/runtime-host/src/manager.rs`.
- Dedicated process gate: `apps/runtime-host/tests/runtime_manager_gate.rs`.
- Deterministic hostile child modes:
  `tests/fixtures/runtime_manager_child_fixture.cpp`.
- CTest registration/deadline: `tests/CMakeLists.txt`.
- This evidence:
  `.superpowers/sdd/2026-08-19-phase-0-5-infrastructure-v1/task-16-report.md`.

## Scope and limitations

No audio device, Audio Engine, AI model/runtime, Python, UI feature, IPC
transport replacement, cloud, installer, plan checkbox, or ADR was added. The
new fixture behavior is BUILD_TESTING-only. Production behavior changes only in
RuntimeManager teardown ownership and fail-safe reap; normal async start,
request, orderly Shutdown, wait, and pipe-drain behavior is retained.

Windows source and construction tests compile only when Task 18 runs on the real
Windows x64/MSVC runner. This macOS task does not claim Windows process-launch,
DLL loading, pipe, task termination, or reap evidence.
