# ADR-000: Use one monorepo

- Status: Accepted
- Date: 2026-08-19

## Context

AI Voice Studio spans a Tauri application, Rust control plane, isolated C++
Runtime, stable C plugin ABI, Mock engine, shared contracts, cross-language
tests, and repository tooling. These parts must evolve together while
preserving explicit process and ownership boundaries. The project baseline and
plans already lived at the current Git root, so a second repository boundary
would split the authoritative history.

## Decision

Use the current Git repository as the single monorepo. Top-level ownership is
divided among `apps`, `core`, `audio`, `runtime`, `engines`, `backend`, `tests`,
`docs`, and `tools`. pnpm, Cargo, and CMake remain independent build graphs
coordinated by root scripts, CTest, and one cross-platform CI workflow.

Cross-language contracts and their generators live with the repository, and a
change may update producers, consumers, tests, documentation, and packaging
atomically. Runtime process isolation and the VoiceEngine C ABI remain hard
runtime boundaries despite sharing source control.

Decision numbering is also repository-wide: ADR-001 owns the Tauri/Rust control
plane, ADR-002 owns the C++ Runtime/C ABI, and ADR-003 is reserved for the Phase
1 Audio Engine. Reserving a number is not accepting or creating that decision.

## Rationale

One reviewable change can keep canonical schemas, generated Rust/C++ code,
native fixtures, resource staging, and desktop consumers consistent. Shared CI
can exercise Debug and Release paths without publishing intermediate packages.
Visible directory boundaries discourage accidental engine loading in the
desktop process while avoiding premature repository/version coordination.

## Alternatives considered

### Separate repositories per component

Independent release histories would add package publication, compatibility
matrices, and coordinated pull requests before the interfaces are mature. They
would make atomic contract changes harder without improving process isolation.

### Nested `AI-Voice-Studio` repository

The current Git root already contains the architecture and plans. A nested
repository would split history, ignore rules, tooling, and CI ownership.

### One undifferentiated source tree and build

A single directory or one language-centric build would obscure ownership and
encourage direct Tauri-to-engine coupling. Separate modules and build graphs
make the intended boundaries testable.

## Consequences

- Contract, implementation, fixture, packaging, and documentation changes can
  be reviewed and validated together.
- Root automation must coordinate Node/pnpm, Rust/Cargo, and CMake/Ninja and
  keep their exact or minimum versions documented.
- CI must cover macOS arm64 and Windows x64 rather than assuming one platform's
  success transfers to the other. Both real runner jobs are accepted.
- Directory co-location does not permit ownership shortcuts: Rust does not load
  engine plugins, and native PCM does not cross RuntimeMessage or Tauri.
- Reserved modules such as `audio`, `backend`, and `core/model-package` may
  remain empty until an accepted decision and phase scope authorize behavior.
