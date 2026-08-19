# Task 20 report — local final acceptance draft

## Status

PENDING / BLOCKED only by the absent real Windows x64 workflow result. Fresh
macOS arm64 evidence is green. Task 18 and Task 20 remain unchecked; the plan
and progress ledger were not edited. No remote, push, reserved ADR file,
Audio Engine, AI/model runtime, Python, cloud/account, player, or user feature
was added.

## Audit result

The main plan, reports and review records for Tasks 1–19, implementation
commits, DEVELOPMENT, ARCHITECTURE, ADR-000/001/002, the workflow, build
presets, staging/bundle scripts, and existing artifacts were inspected before
writing the gate. Tasks 1–17 and 19 have recorded acceptance; Task 18 remains
pending because this repository has no Git remote and therefore no successful
`windows-latest` job URL. An old local app was present but was deliberately not
used as final evidence; Release native artifacts and the Tauri app were rebuilt.

The only implementation gap was repeatable final-package exclusion evidence.
Existing tests verified required files but did not reject an unexpected model,
audio asset, archive, symlink, non-arm64 Mach-O, forbidden dynamic dependency,
or direct Python/AI/audio-device symbol in the finished app. The acceptance
report also had no executable contract preventing a fabricated Windows result
or premature plan mutation.

## TDD evidence

### Bundle exclusion gate

RED 1: `inspect-phase05-bundle.test.mjs` failed with `ERR_MODULE_NOT_FOUND`
before the inspector existed.

RED 2: the first minimal implementation failed immediately because an ESM
strict-mode function parameter used the reserved identifier `arguments`. It was
renamed without changing behavior; the five initial tests then passed.

RED 3: the freshly rebuilt real app exposed an over-broad audit rule. A Rust
configuration type named `AudioDeviceSelection` was mistaken for a direct
audio-device API. A fixture containing the exact real symbol was added first
and failed 5/6. The matcher was then narrowed to concrete `AudioObject`,
`AudioUnit`, `AudioQueue`, and `AudioDevice` operations while retaining all
direct forbidden-import fixtures. The suite passed 6/6 and the real app audit
passed.

The checked behavior uses hand-authored hostile fixtures and the real bundle:
exact inventory, positive sizes and SHA-256 digests, no symlinks, `file` type,
exact `arm64` from `lipo`, complete `otool -L` dependencies, and both undefined
and global `nm` symbols. It does not infer absence from documentation text.

### Report status gate

RED 1: `phase05-acceptance.test.mjs` failed with `ERR_MODULE_NOT_FOUND` before
the validator existed.

RED 2: after the validator was added, all three tests failed because
`docs/development/PHASE-0.5-ACCEPTANCE.md` did not exist. This was the expected
missing deliverable, not a parse error.

GREEN: after recording the evidence, 3/3 passed. Mutation cases reject a fake
Windows PASS/URL, checked Task 18, a missing requirement row, and an artifact
whose absolute path no longer corresponds to its repository-relative path.
The accepted structure requires exactly 13 verification rows, six app files,
PENDING overall status, the sole Windows blocker, known limitations, and
Phase 1 inputs.

RED 3: final diff review found that the first validator incorrectly compared
the recorded macOS absolute artifact path with the current checkout root. A
new test validated the same report from `/github/workspace/other-checkout` and
failed, demonstrating that CI would reject valid local evidence. The report
now records its evidence root explicitly, and the validator uses POSIX evidence
paths independently of the executing checkout. The report suite passed 4/4.

## Fresh macOS arm64 results

- Host: macOS 26.5.2 build 25F84, arm64.
- Doctor: Node 24.16.0, pnpm 11.19.0, rustc/Cargo 1.97.1, CMake 4.4.2,
  Ninja 1.13.2, Git 2.50.1, and Xcode 26.6 passed; MSVC was correctly skipped.
- Frozen install and contracts: passed without lockfile drift.
- Root pnpm gate: contracts 6/6, doctor 7/7, staging 3/3, bundle 3/3,
  workflow 4/4, docs 8/8, Task 20 gates 21/21, and frontend 8/8.
- Frontend lint, typecheck, and Vite production build: passed.
- Rust pinned fmt, locked check, all-target clippy with warnings denied, and
  workspace tests: passed. Fixture-dependent ignored Cargo tests were executed
  by CTest with real native artifacts.
- Native Debug: configure/build passed, CTest 25/25 in 5.47 seconds.
- Native Release: configure/build passed, CTest 25/25 in 5.77 seconds.
- Benchmark: Debug 10/10 and Release 10/10 repeated with 20-second per-test
  deadlines.
- RuntimeManager process gate: Debug 3/3 and Release 3/3 repeated with the same
  deadlines.
- Release staging and Tauri native command integration: 1/1; the real
  RuntimeManager spawned `voice-runtime`, completed Hello, invoked the C ABI
  Mock, shut down, and reaped.
- Tauri Release app: rebuilt successfully; required layout verifier and the new
  exact exclusion inspector passed.
- App launch/quit: PID 40001 remained live through the startup guard and exited
  status 0 after the bundle-identifier quit request, before the 10-second
  deadline. The trap retained terminate/kill cleanup ownership.
- Leak audit: no matching desktop or `voice-runtime` PID and no matching temp
  fixture directory remained.

The host reported `CGSSessionScreenIsLocked=Yes`. The launch/quit and invoke
results are non-visual evidence; this task does not relabel them as a fresh GUI
visual check. Task 13 retains the earlier visual record.

## Exclusion evidence

- Cargo metadata traversal followed normal/build edges from all six workspace
  members: 432 reachable packages, zero exact forbidden runtime/device family
  matches.
- The pnpm production tree contains only `@tauri-apps/api`, with no forbidden
  package match.
- Ninja Release link commands show the sidecar's Runtime sources/static
  libraries and pinned spdlog, and the Mock dylib's single Mock source plus C
  ABI headers. No prohibited target participates.
- The rebuilt 11 MiB app contains exactly six files. Three Mach-O files are
  exactly arm64; the other files are Info.plist, icon, and strict JSON config.
- The desktop links only Apple UI/system frameworks and system libraries; the
  sidecar only libc++/libSystem; the Mock plugin only its self rpath plus
  libc++/libSystem. No forbidden dependency or concrete import appeared in
  3,294 total global observations (3,268 unique) and 457 total undefined
  observations (433 unique).
- Tracked file inventory found no Python, model, audio-media, or archive
  extension. The `audio` and `core/model-package` directories contain README
  ownership markers only.
- The real integration and fixed 80-byte result contract prove that PCM stays
  inside the native Mock pipeline and only summary fields cross IPC.

The first source-ownership command mistakenly named a nonexistent nested
`tests/benchmark/CMakeLists.txt` and exited 2 after reporting that path error.
It was rerun against the actual `tests/CMakeLists.txt` and
`tests/benchmark/mock_pipeline_benchmark_test.cpp`; the corrected audit passed.

## Packaging observation

The local no-identity app has a linker ad-hoc signature and launches cleanly,
but strict deep `codesign` verification reports unsealed resources. Signing,
notarization, and installers are outside this infrastructure gate and were not
silently added as post-build mutations. The acceptance draft records this as a
non-gating distribution limitation.

## Files

- `docs/development/PHASE-0.5-ACCEPTANCE.md`
- `docs/development/PHASE-0.5-ACCEPTANCE-EVIDENCE.json`
- `docs/README.md`
- `tools/scripts/inspect-phase05-bundle.mjs`
- `tools/scripts/inspect-phase05-bundle.test.mjs`
- `tools/scripts/phase05-acceptance.mjs`
- `tools/scripts/phase05-acceptance.test.mjs`
- `package.json`
- this ignored brief/report evidence pair

## External blocker

The minimum remaining action is to select a GitHub repository, add and push to
its remote, run the existing workflow on a real Windows x64 hosted runner, and
retain the successful `windows-x64` job URL/log. Only a subsequent reviewed
governance change may update Task 18 or Task 20. This task did not perform any
external mutation or claim a Windows result.

## Independent review fix

The first independent review correctly rejected the executable evidence
contract even though the live-bundle evidence itself was sound. Three issues
were fixed with new failing tests first:

1. A forged 64-hex digest, positive byte count, non-empty type, or tool version
   could satisfy the old report gate. The live checker now writes a committed,
   deterministic JSON manifest containing exact artifact metadata, binary
   dependency lists, inspection provenance, counts, exclusions, product/
   contract versions, and declared plus doctor-observed tool versions. The
   report gate loads the manifest, independently parses repository authorities,
   and compares every artifact, dependency, binary count, aggregate count, and
   version table cell. Adversarial report mutations cover SHA-256, bytes, type,
   Node `0.0.0`, and every other recorded version; a manifest authority
   mutation also fails.
2. The old abstract evidence validator treated omitted symbol arrays as empty.
   The collector now mints module-private observations only after each actual
   lipo/otool/nm call returns. Validation requires the exact tool, arguments,
   completed flag, raw output, and a lossless reparse match. Tests distinguish
   successful empty output from missing evidence, tool failure, and a forged
   completed-looking object.
3. The report's `2963` global count was incorrect. The generated manifest and
   structured report now record 3,294 total/3,268 unique global observations
   and 457 total/433 unique undefined observations. Per-binary counts and all
   dynamic dependencies are likewise machine-bound.

The manifest omits checkout paths and timestamps. It is regenerated only from
a freshly built Release app using the documented `--write-manifest` command;
identical live evidence must serialize byte-for-byte identically.

## Final gate follow-up

The final P2 review found that numeric validity alone did not make the symbol
count object a closed schema: an extra key was accepted, while a missing key
was rejected only incidentally during later arithmetic. A failing contract test
first recorded the obsolete `undefinedObservations` / `globalObservations`
names, followed by adversarial RED cases for missing and extra fields.

Every per-binary `symbolCounts` object and the aggregate `symbolTotals` object
now has exactly `undefinedTotal`, `undefinedUnique`, `globalTotal`, and
`globalUnique`. Every value must be a nonnegative JavaScript safe integer;
unique cannot exceed total; and aggregate totals must equal the sum of the
three per-binary totals. Aggregate unique values retain their actual cross-file
union meaning (433 undefined and 3,268 global), so they are deliberately not
the sum of the three per-binary unique counts. The report table labels that
scope and remains cell-for-cell bound to the manifest.

The new mutation suite synchronizes the corresponding report row before it
tests deletion of each key, an extra key, `-1`, `NaN`, a fractional value, an
unsafe integer, unique greater than total, and aggregate-total drift. All are
rejected by the manifest contract before report prose can legitimize them.
