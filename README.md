# AI Voice Studio

AI Voice Studio is a native desktop voice-conversion product. This repository is its monorepo root.

## Repository boundaries

| Directory | Ownership boundary |
| --- | --- |
| `apps/` | Desktop applications and host integrations. |
| `core/` | Shared contracts, capabilities, configuration, and telemetry. |
| `audio/` | Audio-domain interfaces and future audio-engine integration. |
| `runtime/` | Native runtime process and its stable plugin boundary. |
| `engines/` | Voice-engine implementations and adapters. |
| `backend/` | Non-UI backend services used by the runtime and engines. |
| `tests/` | Cross-module contract, integration, and benchmark tests. |
| `docs/` | Architecture, development, and decision records. |
| `tools/` | Reproducible developer and repository tooling. |

The V1.0 architecture document and implementation plans remain at the repository root and in `plans/` as the authoritative project baseline. Phase 0.5 implements the offline Tauri/Rust control plane, isolated C++ Runtime, stable C ABI, and deterministic Mock engine. It does not implement audio-device access, an AI/model runtime, Python, or a user voice-conversion workflow. See the [implemented architecture](docs/architecture/ARCHITECTURE.md).

## Native build entry points

The checked-in CMake presets provide the native build baseline. Run `cmake --preset native-debug`, `cmake --build --preset native-debug`, and `ctest --preset native-debug` from the repository root. Release equivalents use `native-release`; build trees remain under `build/` and install/package output is reserved under `dist/`.

## Project notes

- Version: see [VERSION](VERSION).
- License status: see [LICENSE.md](LICENSE.md).
- Third-party notices: see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
- Documentation and decision index: see [docs](docs/README.md).
