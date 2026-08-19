# Task 8 report — Mock VoiceEngine plugin, loader, and real mock pipeline

## Status and commits

Complete on macOS arm64, including review-fix round 1.

- `1be4308` — `feat(runtime): add dynamic mock voice pipeline`
- `8709064` — `fix(runtime): harden mock plugin contract closure`

The approved plan checkbox and frozen `runtime/api/voice_engine.h` / `docs/contracts/voice-engine-c-abi-v1.md` were not modified.

## Delivered behavior

- `aivs_mock_voice_engine` is a hidden-by-default shared library whose production export surface is only `aivs_voice_engine_get_api`. It implements all eight frozen VoiceEngine v1 operations through the negotiated table.
- Normal Mock configuration remains the exact UTF-8 form `{"work_iterations":N}` for `0..1000000`. A bounded, strict test-only form, `{"work_iterations":N,"initial_generation":G}`, seeds generation for exhaustion tests; Runtime CLI never emits it. The only model is `mock-v1` with empty data.
- Prepare accepts float32 interleaved PCM at 8–192 kHz, one or two channels, nonzero stream id, and 1–4096 frames. Process flips each IEEE-754 sign bit, supports disjoint out-of-place and exact in-place buffers, and permits zero frames.
- `process_audio` retains its hot-path constraints: no allocation, blocking lock, I/O, logging, clock, or environment access. Runtime allocates buffers and measures elapsed time outside the ABI call. Metrics/work state uses compile-time-verified lock-free 64-bit atomics.
- Engine-info rejects exact or partial overlap of the two actual output write ranges before capacity or lifecycle evaluation. Failure normalization respects each caller-visible struct prefix.
- The loader accepts only bounded absolute paths and rejects embedded NUL before native `c_str()` use. A raw native handle enters a no-throw guard immediately and transfers only after the shared module owner exists. Windows captures `DWORD` immediately after failing APIs.
- Windows CLI enters through `wmain`, parses wide arguments, preserves the plugin path as `std::filesystem::path`, and reaches `LoadLibraryW` without narrowing. POSIX retains `main`; the wide parser and Unicode-path process flow are portable tests.
- Module lifetime is pinned by instances and in-flight calls. Successful shutdown permits unload; retryable shutdown failure preserves the live instance. Destructor shutdown failure intentionally abandons the handle and retains the module so callable plugin code is not unloaded.
- `Session` accepts an injectable `PipelineService`. RunMockPipeline produces the fixed 80-byte little-endian summary; no PCM crosses RuntimeMessage. Its first-run checksum remains `0x3ecd5190f6f4f725`.
- Pipeline result construction may allocate and is not `noexcept`. Allocation exceptions are translated at the stdio boundary to exit `4` with bounded stderr and no partial new protocol frame; a previously completed Hello frame remains valid.
- CLI accepts only `--plugin <absolute-path>` plus optional `--mock-work-iterations <0..1000000>`. CLI/plugin setup failures exit `4`, emit bounded stderr, and write zero stdout bytes before Hello.

## Dynamic contract and failure coverage

The ABI matrix calls the function table obtained from the loaded shared library. It covers:

- factory nulls, malformed prefixes, version/reserved fields, invalid ranges/no overlap, too-small capacities with canaries, full-table clearing, and complete-table success;
- initialize outer/nested/result validation, strict configuration rejection, successful setup, and allocation/result normalization;
- model identifier and model-data validation, including wrong identifier with non-empty data;
- operation-specific request/result prefix, version, and reserved validation, plus argument-before-state/format precedence;
- prepare/reset invalid states and preservation, reset/prepare generation exhaustion, and retryable shutdown;
- info capacity and metrics failure normalization, info write-range wrap, and exact/partial info overlap;
- process pointer-range wrap, partial overlap rejection, stale generation/format errors, exact out-of-place, exact in-place, and zero-frame behavior;
- literal generation, latency, checksum, and cumulative metrics results.

Dedicated shared-library fixtures cover missing factory export, unsupported and incomplete factory negotiation, initialize failure, shutdown failure, and a blocked in-flight metrics call. Loader tests verify rejection/unload, instance ownership, in-flight ownership, successful-shutdown unload, and the documented shutdown-failure abandonment behavior. Real-process tests cover unsupported factory and initialize failure as setup failures.

## RED / GREEN evidence

In addition to the original feature RED/GREEN sequence, review fixes produced these direct failures before implementation:

1. Wide-option tests did not compile until the wide parser/native entry path was added.
2. Dynamic engine-info tests accepted overlapping output ranges and a wrapped required write range until validation was added.
3. Short result prefixes retained covered failure fields until prepare/reset/metrics normalization became per-field.
4. An allocating pipeline exception terminated the test process while `run` was `noexcept`; removing the incorrect contract allowed stdio to translate it to exit `4`.

The malicious fixture and lifetime tests then exercised the corrected loader through actual dynamically loaded modules.

## Verification evidence

- Debug configure/build/CTest: `13/13` passed.
- Release configure/build/CTest: `13/13` passed.
- Export inspection: the production Mock plugin exposed exactly `_aivs_voice_engine_get_api` on macOS.
- Real-process smoke: valid Unicode absolute plugin copy/path, Hello-first success, fixed 80-byte result/checksum, clean shutdown, bounded failure diagnostics, and zero stdout for setup failures.
- Warning build: all C/C++ targets passed `-Wall -Wextra -Wpedantic -Werror`.
- Rust: `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, and `cargo test --workspace` passed (11 tests).
- pnpm: `pnpm contracts:check` and `pnpm test` passed (13 tests total: 6 contract and 7 doctor).
- ABI drift/frozen-file audit and `git diff --check` passed.

## Remaining concern

Windows x64 compilation and native execution of `wmain` / `LoadLibraryW` still require the Task 18 Windows runner. The platform-neutral wide-option tests and real Unicode-path smoke pass locally, but this report makes no native Windows execution claim.
