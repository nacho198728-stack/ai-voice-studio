# Phase 0.5 acceptance evidence

This document records local macOS arm64 evidence and successful real Windows
x64 GitHub-hosted runner evidence gathered on 2026-08-20. It is the
cross-platform Phase 0.5 sign-off.
The durable local package record is the generated
[acceptance evidence manifest](PHASE-0.5-ACCEPTANCE-EVIDENCE.json).

## Gate status

| Field | Value |
| --- | --- |
| Overall | PASS |
| Only blocker | None |
| Local host | macOS 26.5.2 build 25F84, arm64 |
| Local repository root | /Users/alex/Documents/ChatGPT/AI翻唱 |
| Evidence date | 2026-08-20 Asia/Shanghai |
| Plan state | Task 18 and Task 20 complete |

## Verification matrix

| ID | Requirement | macOS arm64 | Fresh local evidence | Windows x64 |
| --- | --- | --- | --- | --- |
| MONO | Repository is the planned monorepo and preserves the V1.0 documents | PASS | Tracked-tree audit, all prior task reports, and final diff scope review | PASS |
| CROSS | CMake, Cargo, frontend, and Tauri gates have clean cross-platform paths | PASS | Frozen install, both native presets, locked Rust, frontend build, workflow fixtures, and arm64 bundle build | PASS |
| UI | Desktop exposes the bounded Runtime states and five control operations | PASS | Frontend 8/8, lint, typecheck, Vite build, real rebuilt app launch and clean quit; screen lock prevented a fresh visual inspection | PASS |
| PROC | RuntimeManager starts, handshakes, queries, stops, recovers, and reaps | PASS | Debug and Release process CTests 25/25 plus RuntimeManager gate repeated three times per profile | PASS |
| IPC | Framing, bounds, correlation, command, and version errors are deterministic | PASS | JS contract 6/6, Rust protocol 17/17, native contract and hostile-process CTests | PASS |
| MOCK | Runtime dynamically loads the Mock through C ABI and returns a fixed checksum | PASS | Native CTest plus staged Tauri invoke integration reached handshake, Mock command, ordered shutdown, and reap | PASS |
| ABI | C and C++20 consume the versioned caller-owned C ABI contract | PASS | Debug and Release C/C++ ABI, layout, scalar-drift, export, and dynamic-loader gates | PASS |
| PCM | PCM remains native-only; IPC and UI receive only a bounded summary | PASS | Mock pipeline wire result remains the fixed 80-byte summary; contract and desktop tests reject payload drift | PASS |
| LOG | Structured Rust/C++ logs cannot corrupt stdout IPC or log per frame | PASS | Telemetry, native logging, stderr flood, stdout framing, and no-hot-path-log gates passed | PASS |
| CONFIG | Strict config and capability semantics fail closed with actionable errors | PASS | Config 6/6, capability 2/2, staging, bundle, and native startup evidence | PASS |
| TEST | Contract, Runtime, benchmark, frontend, Rust, and build gates pass | PASS | Full suites passed; benchmark repeated 10 times per native profile with 20-second test deadlines | PASS |
| ADR | ADR-000/001/002 match implementation and the next ADR number remains reserved | PASS | Documentation contract 8/8 and repository filename audit | PASS |
| EXCL | No Python, PyTorch, CUDA, ONNX Runtime, model payload, cloud/account/player, or real audio-device integration exists | PASS | Dependency closure, CMake link graph, exact package inventory, Mach-O dependency and symbol audits described below | PASS |

The macOS `PASS` entries are local results only. They do not infer a Windows
result. The authoritative protocol and ABI versions are in
[`version.json`](../../core/contracts/version.json); the architecture and
ownership boundaries are in [ARCHITECTURE.md](../architecture/ARCHITECTURE.md).

## Version evidence

| Component | Declared | Observed | Authority |
| --- | --- | --- | --- |
| Product | 0.0.0 | 0.0.0 | VERSION + apps/desktop/package.json + tauri.conf.json + bundle Info.plist |
| Runtime | 0.0.0 | 0.0.0 | core/contracts/version.json |
| IPC protocol | 1 | 1 | core/contracts/version.json |
| VoiceEngine ABI | 1 | 1 | core/contracts/version.json |
| Node.js | 24.16.0 | 24.16.0 | .node-version |
| pnpm | 11.19.0 | 11.19.0 | package.json#packageManager |
| Rust | 1.97.1 | 1.97.1 | rust-toolchain.toml |
| CMake | 4.4.2 | 4.4.2 | .github/workflows/build.yml |
| Ninja | 1.13.2 | 1.13.2 | .github/workflows/build.yml |

## Fresh local command evidence

| Gate | Result |
| --- | --- |
| `node tools/scripts/doctor.mjs --json` | PASS: Node 24.16.0, pnpm 11.19.0, rustc/Cargo 1.97.1, CMake 4.4.2, Ninja 1.13.2, Git 2.50.1, Xcode 26.6; MSVC correctly not evaluated on Darwin |
| `pnpm install --frozen-lockfile` | PASS: lockfile unchanged and workspace already current |
| Contracts, tooling fixtures, docs, workflow, acceptance, and frontend unit tests | PASS |
| Frontend lint, typecheck, and production Vite build | PASS: 2 files and 8 tests, no lint/type error |
| Rust fmt, locked check, clippy with warnings denied, and workspace tests | PASS |
| Native Debug configure, build, and CTest | PASS: 25/25 in 5.47 seconds |
| Native Release configure, build, and CTest | PASS: 25/25 in 5.77 seconds |
| Focused benchmark repeat | PASS: Debug 10/10 and Release 10/10, each invocation bounded by 20 seconds |
| Focused RuntimeManager repeat | PASS: Debug 3/3 and Release 3/3, each invocation bounded by 20 seconds |
| Release staging and ignored native desktop integration | PASS: 1/1, real Runtime and Mock path, clean shutdown/reap |
| Tauri Release app build and bundle layout verification | PASS: rebuilt from the Release staged sidecar, plugin, and config |
| Actual app launch and quit | PASS: PID 40001 remained live during startup guard and exited status 0 after application quit request, within 10 seconds |
| Post-run process and temporary-fixture scan | PASS: no app or `voice-runtime` process and no matching `/tmp` fixture directory remained |

All potentially long native repeats used CTest's `--timeout 20`. The app smoke
installed an exit trap, a bounded graceful-quit loop, and a final terminate/
kill cleanup path. At collection time `CGSSessionScreenIsLocked=Yes`, so this
run is launch/quit evidence and not a fresh visual claim. Earlier Task 13
evidence remains the visual UI record.

## Exclusion and ownership audit

The absence conclusion is based on build/package structure and executable
interfaces, not on a prose keyword search:

1. `cargo metadata --locked` was traversed from all six workspace members over
   normal and build edges. The reachable closure contained 432 packages and
   zero exact matches for the audited embedded-Python, PyTorch, CUDA, ONNX, or
   audio-device crate families. This includes dependencies linked through the
   Tauri application rather than only direct declarations.
2. `pnpm --recursive list --prod --depth Infinity --json` contained one
   production package, `@tauri-apps/api`; the audited forbidden package set was
   empty.
3. Ninja's Release commands show `voice-runtime` compiled only from the
   Runtime process/session/pipeline/loader/logging sources and linked to their
   static libraries plus pinned spdlog. The Mock dynamic library is built from
   `engines/mock/mock_voice_engine.cpp` and the C ABI headers only. There is no
   audio, model, Python, AI, cloud, account, or player target in this graph.
4. The rebuilt app inventory is exactly the six files below. A recursive
   file-type and extension audit rejects symlinks, archives, Python files,
   model formats, and audio assets rather than accepting unknown extras.
5. `lipo`, `otool -L`, and `nm` were run on all three Mach-O files. Each is
   exactly arm64. The desktop uses system UI frameworks and system libraries;
   the sidecar uses only libc++ and libSystem; the plugin uses its own rpath
   install name plus libc++ and libSystem. Audited Python/PyTorch/CUDA/ONNX and
   concrete audio-device dependencies/imports were absent from 3,294 total
   global observations (3,268 unique) and 457 total undefined observations
   (433 unique).
6. The tracked extension inventory contained no Python bytecode/source,
   static archive, model, or audio-media file. `audio/` and
   `core/model-package/` contain only ownership README files. The system
   `CoreVideo` framework brought by the Tauri/WebKit shell is a graphics/video
   dependency and is not evidence of a CoreAudio device path.
7. Functional integration produces PCM only inside the C++ Mock pipeline. The
   public Rust/UI result remains frame count, checksum, elapsed time, and
   metrics in the fixed 80-byte summary; no PCM array crosses RuntimeMessage.

The repeatable executable check is
`node tools/scripts/inspect-phase05-bundle.mjs "target/release/bundle/macos/AI Voice Studio.app" aarch64-apple-darwin`.
Its fixture suite also proves that an injected model/audio file, forbidden
dynamic library, direct AI/audio-device symbol, symlink, missing artifact, or
wrong architecture fails the gate.

Regenerate the manifest only after a fresh Release staging and Tauri app build,
or whenever bundle contents or the exact Node/pnpm/Rust/CMake/Ninja authorities
change:

```sh
node tools/scripts/inspect-phase05-bundle.mjs "target/release/bundle/macos/AI Voice Studio.app" aarch64-apple-darwin --write-manifest docs/development/PHASE-0.5-ACCEPTANCE-EVIDENCE.json
```

The generator runs doctor and reads the repository declarations, bundle
Info.plist, inventory, `file`, `lipo`, `otool`, and both `nm` modes before it
writes JSON. The manifest intentionally contains no checkout-absolute path or
generation timestamp, so identical evidence serializes identically.

## Mach-O inspection evidence

| Repository-relative path | Architecture | Dependency count | Undefined observations | Undefined unique | Global observations | Global unique | Inspection provenance |
| --- | --- | --- | --- | --- | --- | --- | --- |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | arm64 | 13 | 292 | 292 | 2624 | 2624 | lipo -archs; otool -L; nm -u; nm -g — completed |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/voice-runtime | arm64 | 2 | 161 | 161 | 665 | 665 | lipo -archs; otool -L; nm -u; nm -g — completed |
| target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib | arm64 | 3 | 4 | 4 | 5 | 5 | lipo -archs; otool -L; nm -u; nm -g — completed |

## Mach-O dynamic dependencies

| Repository-relative binary | Dependency |
| --- | --- |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /System/Library/Frameworks/WebKit.framework/Versions/A/WebKit |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /System/Library/Frameworks/ApplicationServices.framework/Versions/A/ApplicationServices |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /System/Library/Frameworks/CoreGraphics.framework/Versions/A/CoreGraphics |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /System/Library/Frameworks/Carbon.framework/Versions/A/Carbon |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /System/Library/Frameworks/CoreVideo.framework/Versions/A/CoreVideo |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /usr/lib/libSystem.B.dylib |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /System/Library/Frameworks/AppKit.framework/Versions/C/AppKit |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /System/Library/Frameworks/Foundation.framework/Versions/C/Foundation |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /usr/lib/libobjc.A.dylib |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /usr/lib/libiconv.2.dylib |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /System/Library/Frameworks/ColorSync.framework/Versions/A/ColorSync |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /System/Library/Frameworks/CoreServices.framework/Versions/A/CoreServices |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/voice-runtime | /usr/lib/libc++.1.dylib |
| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/voice-runtime | /usr/lib/libSystem.B.dylib |
| target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib | @rpath/libaivs_mock_voice_engine.dylib |
| target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib | /usr/lib/libc++.1.dylib |
| target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib | /usr/lib/libSystem.B.dylib |

## Symbol observation totals

| Scope | Undefined observations | Undefined unique | Global observations | Global unique |
| --- | --- | --- | --- | --- |
| All three Mach-O files (cross-file union for unique) | 457 | 433 | 3294 | 3268 |

## Release bundle artifacts

| Role | Repository-relative path | Absolute path | Bytes | SHA-256 | File type |
| --- | --- | --- | --- | --- | --- |
| Bundle metadata | target/release/bundle/macos/AI Voice Studio.app/Contents/Info.plist | /Users/alex/Documents/ChatGPT/AI翻唱/target/release/bundle/macos/AI Voice Studio.app/Contents/Info.plist | 1000 | cb73d729da55f14b514374da0c6c7f4f48b9af03651fe5a51c68420af85c3967 | XML 1.0 document text, ASCII text |
| Desktop executable | target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | /Users/alex/Documents/ChatGPT/AI翻唱/target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | 11126720 | c33e5ab4f89491e453745b8568cbaff22b243404368f7477c7dc071b712134bd | Mach-O 64-bit executable arm64 |
| Runtime sidecar | target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/voice-runtime | /Users/alex/Documents/ChatGPT/AI翻唱/target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/voice-runtime | 670464 | 3760d334380837366bcccd593e88ef85c1b2423a0c5898c2fd99eecff8696dd6 | Mach-O 64-bit executable arm64 |
| Application icon | target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/AI Voice Studio.icns | /Users/alex/Documents/ChatGPT/AI翻唱/target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/AI Voice Studio.icns | 2245 | 8a5d5dd9e5d4278030f536935e19b98d62053eba97c05a1821cbeeae34dc025f | Mac OS X icon, 2245 bytes, "icp6" type |
| Product config | target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/config/config.json | /Users/alex/Documents/ChatGPT/AI翻唱/target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/config/config.json | 668 | 6a951523d63686a0e4fb46e315180414b047aeb2667074db99a9d5f90e9e5584 | JSON data |
| Mock plugin | target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib | /Users/alex/Documents/ChatGPT/AI翻唱/target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib | 34736 | 88816039193e32b4f9affeed55a46f57b132c2a973272c8b4a0f2b3b5f2af76c | Mach-O 64-bit dynamically linked shared library arm64 |

The app directory occupies 11 MiB on this host. These ignored build products
are reproducible local evidence, not committed binaries.

## Windows CI evidence

| Job | Status | Evidence URL |
| --- | --- | --- |
| Windows x64 | PASS | https://github.com/nacho198728-stack/ai-voice-studio/actions/runs/32361978210/job/96403295911 |

The workflow's `windows-x64` job declares x64 MSVC, exact Node/pnpm/Rust/CMake/
Ninja checks, Debug and Release CTest, unique DLL export, real dynamic loading,
Runtime process shutdown/reap, native desktop integration, Tauri `--no-bundle`,
and portable layout verification. The linked job executed these gates
successfully on `windows-latest` x64 with MSVC and is the authoritative Task 18
Windows evidence.

## Known limitations

- The current macOS session was locked. The rebuilt app's launch and exit were
  freshly exercised, but its pixels and interactions were not visually
  rechecked in this run.
- This local app is linker ad-hoc signed only. It launches successfully, but
  `codesign --verify --deep --strict` reports that resources are not sealed.
  Distribution identity signing, notarization, and installer acceptance are
  outside the current infrastructure gate.
- The product version is development `0.0.0`; Runtime version is `0.0.0`, IPC
  protocol is v1, and VoiceEngine ABI is v1.
- The transport is single-child bounded stdio, the backend is Mock, and
  capability hardware fields remain `not_evaluated`. There is no Audio Engine,
  real device, model runtime, cloud service, account, player, or user workflow.

## Phase 1 inputs

- Preserve RuntimeMessage v1, ErrorCode, the process reap barrier, and the
  VoiceEngine C ABI v1 as the control/plugin baseline.
- Use the reserved next architecture decision for Audio Engine ownership,
  callback constraints, device enumeration, and failure isolation; no such
  decision file belongs to this phase.
- Keep PCM below the Runtime/plugin boundary and continue returning bounded
  summaries to the control plane.
- Decide release signing/notarization separately from Audio Engine work.
- Preserve the successful Windows x64 workflow as a required baseline gate.

## Minimal external action

Completed: the private GitHub repository is configured, this branch is pushed,
and the linked macOS arm64 and Windows x64 jobs are successful. No further
external action is required for Phase 0.5. ADR-003 remains reserved.
