# Task 8 report — Mock VoiceEngine plugin, loader, and real mock pipeline

## Status and commit

Complete on macOS arm64. Implementation commit: `1be4308` (`feat(runtime): add dynamic mock voice pipeline`). The approved plan checkbox and frozen `runtime/api/voice_engine.h` / `docs/contracts/voice-engine-c-abi-v1.md` were not modified.

## Decisions and delivered behavior

- `aivs_mock_voice_engine` is a hidden-by-default shared library with exactly one exported symbol, `aivs_voice_engine_get_api`. It has no Rust, Tauri, audio-device, filesystem, network, Python, AI, or ONNX dependency.
- The plugin implements all eight VoiceEngine v1 operations through the frozen table. Configuration is exactly `{"work_iterations":N}` for `0..1000000`; the simulated model is `mock-v1` with an empty data view.
- Prepare accepts float32 interleaved, 8–192 kHz, one/two channels, nonzero stream id, and 1–4096 frames; algorithmic latency is exactly zero. Process flips the IEEE-754 sign bit, supports exact in-place, performs optional bounded deterministic CPU work, and uses compile-time-verified lock-free 64-bit atomics for metrics/work sink.
- `process_audio` contains no allocation, blocking lock, I/O, logging, clock, or environment access. Runtime allocates deterministic 128-frame/two-channel buffers and measures elapsed time outside the ABI call.
- The loader accepts only bounded absolute paths, uses `LoadLibraryW`/`GetProcAddress` or `dlopen`/`dlsym`, negotiates and validates the complete v1 table, tracks in-flight calls, and unloads only after successful shutdown. A destructor shutdown failure deliberately retains the live handle/module rather than unloading callable code.
- `Session` takes an injectable `PipelineService`. No plugin preserves `EngineUnavailable`; a loaded Mock reports `mock` / `aivs-mock-v1` and executes load-once, prepare, process, metrics, reset.
- RunMockPipeline request is exactly empty. Success is a fixed 80-byte little-endian control summary containing schema/size, frames/channels, FNV-1a checksum, Runtime-measured microseconds, latency, generation, and cumulative metrics. No PCM crosses RuntimeMessage. The literal first-run checksum is `0x3ecd5190f6f4f725`.
- CLI accepts only `--plugin <absolute-path>` and optional `--mock-work-iterations <0..1000000>`. Unknown, duplicate, missing, relative, or malformed values and setup failures exit `4`, emit bounded stderr, and write zero stdout bytes before Hello.

## Main files

- Plugin: `engines/mock/mock_voice_engine.cpp`, `engines/CMakeLists.txt`
- Loader: `runtime/loader/include/ai_voice_runtime/voice_engine_loader.hpp`, `runtime/loader/voice_engine_loader.cpp`
- Pipeline/control: `runtime/pipeline/include/ai_voice_runtime/mock_pipeline.hpp`, `runtime/pipeline/mock_pipeline.cpp`, `runtime/process/runtime_options.cpp`
- Runtime integration: `runtime/session/*`, `runtime/process/stdio_runtime.cpp`, `runtime/main.cpp`
- Contract documentation: `docs/contracts/voice-runtime-stdio-v1.md`
- Dynamic ABI, loader, pipeline, CLI, export, Session, stdio, and real-process tests: `tests/runtime/*`, `tests/CMakeLists.txt`

## RED / GREEN evidence

1. Dynamic plugin lifecycle: configure failed because `engines/mock/mock_voice_engine.cpp` did not exist; after the minimal plugin implementation, the real dynamic-table lifecycle test passed.
2. Explicit loader: configure failed because `ai_voice::runtime_loader` did not exist; after implementing the platform loader and RAII handle/module ownership, missing/wrong/valid library tests passed.
3. Pipeline injection and fixed result: configure failed because `ai_voice::mock_pipeline` did not exist; after implementation, deterministic checksum/generation/metrics and Session capability tests passed.
4. CLI: configure failed because `ai_voice::runtime_options` did not exist; the bounded parser passed after implementation.
5. Real process: smoke failed because RunMockPipeline still returned `EngineUnavailable`; after startup setup/injection, plugin request plus shutdown passed, while invalid CLI/missing-plugin cases proved zero stdout.
6. ABI error precedence: a malformed second load returned `InvalidState` instead of `InvalidArgument`; moving argument validation before lifecycle made the dynamic test pass.
7. Failure output atomicity: malformed nested engine-info output left a fully covered sibling count unchanged; prefix-aware independent normalization made the test pass.

## Verification evidence

- Debug: configure/build plus CTest, `12/12` passed.
- Release: configure/build plus CTest, `12/12` passed.
- Dynamic ABI tests cover factory negotiation, all eight function pointers, lifecycle, malformed state/argument precedence, all-or-nothing info, buffer-too-small output atomicity, out-of-place/exact-in-place/zero-frame processing, overlap/stale-generation rejection, reset/generation, cumulative metrics, bounded CPU work, retryable shutdown failure, and unload.
- Export inspection: macOS `nm` test found exactly `_aivs_voice_engine_get_api`.
- Real process smoke: checksum `0x3ecd5190f6f4f725`, fixed 80-byte summary, Hello-first, shutdown, no orphan timeout, empty stderr on success, and zero stdout on setup/CLI failure.
- Warning build: all C/C++ targets built with `-Wall -Wextra -Wpedantic -Werror`.
- Rust: `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, and `cargo test --workspace` passed (11 tests total).
- pnpm: `pnpm contracts:check` and `pnpm test` passed (13 Node tests total); generated contract mappings remained current.
- `git diff --check` passed.

## Self-review and concerns

- Reviewed the frozen ABI rules against each operation and checked the hot function body for allocation/lock/I/O/log/time/environment calls; only initialization allocates, and 64-bit atomics are required lock-free at compile time.
- RuntimeMessage schema/policies were not loosened; existing payload ceiling and stdout-only framing remain intact.
- Local macOS arm64 evidence is complete. Windows x64 compilation/loading/export/process behavior still requires the real Task 18 CI runner; the implementation has dedicated `LoadLibraryW`/`GetProcAddress` code and portable CTest paths but no local Windows claim is made.
