# Task 12 report — versioned configuration and capability model

## Status

Complete. The repository now has a strict versioned default/development
configuration, focused Rust configuration and capability crates, and explicit
RuntimeManager mappings from validated settings plus caller-owned resource
paths. The plan checkbox, ADRs, native Runtime behavior, logging, Tauri, audio,
hardware inspection, model behavior, and user features were not modified.

## Schema and model decisions

- `config/config.json` is schema version 1 and contains bounded Runtime
  timeout/queue/stderr/restart settings, `backend.kind=mock`, bounded Mock work
  iterations, debug/log-level intent, and explicitly disabled/unconfigured
  audio placeholders. Runtime/plugin paths are intentionally absent.
- `ai-voice-config` rejects unknown, missing, duplicate, wrong-type, invalid
  enum, and unsupported-version input through private raw Serde DTOs. Validated
  public types have private fields/getters and no `Deserialize` implementation,
  so direct `serde_json` use or field mutation cannot bypass validation. Semantic
  validation shares RuntimeManager/native limits: 1..=300,000 ms timeouts,
  1..=64 in flight, 1..=256 queues, 0..=65,536 stderr bytes, and
  0..=1,000,000 Mock iterations. Audio must remain disabled, both device
  selections unconfigured, and sample-rate/buffer placeholders null.
- Load-from-bytes and load-from-path are read-only. Typed errors distinguish
  I/O, invalid document, unsupported schema, and semantic failures. There is no
  repair, mutation, implicit default, path search, or fallback.
- `ai-voice-capability` owns only generation-free platform, architecture,
  backend, Mock identity/availability, and validated native payload types. It
  exposes no manager-generation or profile-construction API. RuntimeHost owns
  the generation-bearing manager, Runtime, Engine, and `CapabilityProfile`
  output types and their private construction path.
- Query state is explicitly `not_evaluated`, `inconclusive`, or `observed`.
  Engine availability therefore distinguishes `available`, `unavailable`,
  `unknown`, and `not_evaluated`; `unknown` is emitted only for an inconclusive
  connected query. Runtime reachability is independently derived from manager
  health: a connected Runtime stays available even when its observed Mock
  engine is unavailable. Starting, stopping, stopped, crashed, and error states
  are fixed by a literal expected-profile matrix. No hardware/device/driver/
  benchmark or machine-identity claim is present.
- Native `GetCapabilities` parsing moved to the shared capability crate. It
  accepts only canonical JSON and the truthful pairs
  `mock/aivs-mock-v1` and `unavailable/unavailable`; Runtime/protocol versions
  come from `ai-voice-contracts`, not duplicated literals. Private raw DTOs
  prevent direct construction/deserialization of mismatched public native
  observations. Field-bounded errors identify Runtime version, protocol,
  platform, architecture, or backend/engine mismatch without payload values.
- `RuntimeManagerConfig::from_product_config` consumes the invariant-bearing
  product config and
  receives explicit Runtime/plugin paths from its caller. `RuntimeStatus` maps
  state/generation plus an actor-authorized query observation into the shared
  profile. `get_capabilities` and `capabilities_not_evaluated` return opaque
  actor-minted outcomes: observed data, query failures, and not-yet-evaluated
  state all carry the authoritative manager generation. Their constructors and
  fields are private, so callers cannot copy native data or relabel an old
  failure after restart. Profile construction rejects both kinds of stale
  outcome. The checked-in file never owns packaged resource paths.
- `core/model-package` and `core/telemetry` contain ownership documentation
  only. No package parser, logger, tracing subscriber, sink, directory, or file
  output was implemented.

## Files

- Default contract and ownership: `config/config.json`, `config/README.md`.
- Configuration crate/tests: `core/config/Cargo.toml`, `core/config/src/lib.rs`,
  `core/config/tests/product_config.rs`, `core/config/README.md`.
- Capability crate/tests: `core/capability/Cargo.toml`,
  `core/capability/src/lib.rs`, `core/capability/tests/capability.rs`,
  `core/capability/README.md`.
- Runtime mappings: `apps/runtime-host/Cargo.toml`,
  `apps/runtime-host/src/lib.rs`, `apps/runtime-host/src/manager.rs`,
  `apps/runtime-host/src/payload.rs`,
  `apps/runtime-host/tests/config_capability.rs`, and the typed expectation in
  `apps/runtime-host/tests/runtime_manager_process.rs`; controlled actor
  provenance cases are in `apps/runtime-host/tests/runtime_manager_fixture.rs`
  and `tests/fixtures/runtime_manager_child_fixture.cpp`.
- Workspace/lock: `Cargo.toml`, `Cargo.lock`.
- Policy/ownership docs: `core/README.md`,
  `docs/development/CONFIGURATION.md`, `core/model-package/README.md`, and
  `core/telemetry/README.md`.

## RED/GREEN evidence

- Configuration RED first failed to compile because `ProductConfig`, strict
  enums, and typed errors did not exist. After the initial parser, the boundary
  suite remained RED because `work_iterations=1,000,001` was accepted. GREEN
  added the missing native-limit validation; all five config integration tests
  then passed.
- Capability RED first failed to compile because the shared types, normalizer,
  and profile constructor did not exist. GREEN added the minimal stable schema,
  literal native normalization, canonical contract-version checks, and truthful
  availability mapping; all three capability integration tests passed.
- Runtime integration RED failed on absent config-to-launch construction,
  absent shared native parsing, absent status-to-profile mapping, and private
  native Engine identity. GREEN wired both crates into RuntimeHost, reused
  config-owned semantic limits, preserved explicit path authority, and passed
  all three mapping tests.
- Full-workspace regression exposed one stale Task 10 expectation that treated
  the native `unavailable/unavailable` payload as malformed. The shared model
  intentionally recognizes that truthful pair; the RuntimeHost payload test
  now asserts both allowed pairs and still rejects mismatches/malformed input.
- Review-fix RED failed on the absent explicit observation model, private/getter
  APIs, validated constructors, safe mismatch reasons, and generation-bound
  RuntimeHost result. GREEN adds a literal matrix for all six manager states,
  both native pairs, inconclusive and unqueried outcomes; private raw config and
  native DTOs; Serialize-only capability output; field-level safe errors; and
  actor-generation propagation. A real-child CTest now starts generation 1,
  captures capabilities, crashes/restarts into generation 2, rejects the stale
  observation, and accepts a fresh generation-2 query.
- Boundary review-fix coverage is table-driven across minimum, below-minimum,
  maximum, and above-maximum for each timeout, pending/queue bound, stderr
  retention, Mock work iterations, and the representational restart-attempt
  bound. The public no-Deserialize invariant has a compile-fail doctest.
- Re-review RED recorded two `compile_fail` examples unexpectedly compiling:
  public callers could relabel copied native data and failed/not-evaluated
  states. GREEN moves the generation-bound outcome into RuntimeHost with no
  public constructor, has the actor mint all three outcomes, and adds controlled
  restart coverage proving old successful and old failed observations are both
  stale. Two adversarial compile-fail tests prove rebinding is unavailable to
  external callers.
- Final-review RED recorded the remaining lower-level bypass: public
  `CapabilityEvaluation`, `ManagerCapability::new`, and
  `CapabilityProfile::from_evaluation` successfully assembled copied native
  data with an arbitrary current generation. GREEN removes that entire public
  construction chain, moves the generation-bearing profile and private truth
  table into RuntimeHost, and leaves the shared capability crate generation-
  free. A third adversarial compile-fail test covers the exact former bypass.

## Verification

- `cargo +1.97.1 fmt --all -- --check` — passed.
- `cargo +1.97.1 clippy --locked --workspace --all-targets -- -D warnings` —
  passed without warnings.
- `cargo +1.97.1 check --locked --workspace` — passed.
- `cargo +1.97.1 test --locked --workspace` — passed: 33 unit/integration tests
  plus four compile-fail doctests executed, 0 failed; 19 native-path/
  controlled-child cases remained CTest-owned.
- Checked-in default config path load and full malformed/boundary matrix —
  passed; the loader test's exact bytes before and after loading were identical,
  and loading a missing path did not create it. No independent checked-in byte
  snapshot fixture is claimed.
- Fresh Debug configure/build/CTest — passed 15/15, including real-child
  RuntimeManager, actor-minted not-evaluated state, stale failed-query restart,
  and controlled hostile-child regressions.
- Fresh Release configure/build/CTest — passed 15/15, including the same real
  child, provenance/restart, deadline, ordered-reap, and no-orphan coverage.
- `pnpm test` — passed: contract drift/generation 6/6 and doctor 7/7.
- `git diff --check` and staged diff check — passed.

## Commit

- `7d79170` — `feat(core): add config and capability contracts`
- `b57189b` — `fix(core): enforce capability observation invariants`
- `27db08f` — `fix(runtime): seal capability observation provenance`
- `45902b2` — `fix(runtime): make capability profiles actor-authorized`

## Self-review

- The configuration crate performs no path discovery or writes, and the
  RuntimeManager construction path cannot obtain Runtime/plugin locations from
  JSON. Existing direct `RuntimeManagerConfig::new` remains available for
  controlled tests and explicit programmatic callers.
- Numeric limits have one shared Rust authority between product config and
  RuntimeManager; native Runtime/protocol versions continue to come from the
  generated contracts crate.
- Capability JSON is covered by an independent literal fixture, native payload
  normalization rejects reordered/extra/invalid claims, and profile output has
  no CPU/GPU/RAM/NPU/audio/device/driver/benchmark/machine fields.
- Public composite configuration/native/profile types cannot be directly
  deserialized or field-mutated into an invalid state. Public capability output
  is Serialize-only. RuntimeHost's opaque observation has no public constructor,
  and its generation-bearing profile has no public constructor or public lower-
  level assembly function. Generation provenance is checked before an actor-
  authorized outcome can be projected into a profile.
- Dependencies remain pinned to existing workspace Serde versions; the lockfile
  adds only the two local crates and their already-locked dependencies.

## Concerns

- Execution evidence is macOS arm64 only. Windows x64/MSVC compilation and
  runtime validation remain the explicit Task 18 CI gate.
- The debug/log-level fields are intent only until Task 11; no logging fallback
  or initialization behavior exists in this task.
