# ADR-000: Use one monorepo

- Status: Accepted
- Date: 2026-08-19

## Context

AI Voice Studio spans a desktop shell, Rust control plane, C++ runtime, audio boundary, engine plugins, shared contracts, tests, and developer tooling. These components must evolve together while preserving explicit process and ABI boundaries. At repository initialization, the project needs one location for its architecture, plans, contracts, and cross-module verification without creating nested repositories.

## Decision

Use the current Git repository as the single AI Voice Studio monorepo. Its top-level modules are `apps`, `core`, `audio`, `runtime`, `engines`, `backend`, `tests`, `docs`, and `tools`.

ADR-001 is reserved for the Tauri + Rust Control Plane. ADR-002 is reserved for the C++ Runtime + C ABI. ADR-003 is reserved for the Phase 1 Audio Engine.

## Rationale

One repository keeps shared contracts, version compatibility, architecture documentation, and cross-language tests reviewable in the same change as their consumers. It also makes the intended module boundaries visible before build systems and implementations are introduced, while retaining the required runtime-process isolation.

## Alternatives Considered

### Separate repositories per component

This would provide independent histories but would add coordination and versioning overhead before stable interfaces or release processes exist.

### Nested `AI-Voice-Studio` repository

This was rejected because the current Git root already contains the authoritative V1.0 architecture document and plans; a nested repository would split the project baseline and complicate tooling.

### Single undifferentiated source directory

This was rejected because it would obscure ownership and encourage violations of the desktop, control-plane, runtime, and engine boundaries.

## Consequences

- Cross-module changes can be reviewed and validated atomically in one repository.
- Module ownership is established by top-level directories and will be refined through later ADRs and build configuration.
- A monorepo does not remove runtime isolation: the native runtime remains a separately owned process boundary.
- Repository-wide tooling and CI will eventually need to account for multiple languages and platforms.
