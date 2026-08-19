# Task 14 report — VoiceEngine C ABI contract gate

## Status

Complete on macOS arm64. The independent contract gate now keeps the public
VoiceEngine ABI consumable as strict C17 and C++20, pins the frozen v1 layout
and values, exercises the real Mock dynamic table and failure matrix, and
checks the single permitted plugin export on Unix and in the MSVC CTest
configuration. `runtime/api/voice_engine.h`, canonical JSON, ADR-002, and the
main plan checkbox were not changed. ADR-003 was not created.

Windows x64/MSVC was not executed. Its compile options and `dumpbin /EXPORTS`
CTest path are configured, and the parser is covered by representative positive
and negative fixtures; real Windows execution remains Task 18.

## Audit and requirement mapping

| Required outcome | Concrete evidence |
| --- | --- |
| Self-contained public header in strict C17 and C++20 | `voice_engine_c_abi_c_test.c` and `voice_engine_c_abi_cpp_test.cpp` include `voice_engine.h` first, assert the active language standard, and build with extensions disabled. The two external-consumer targets use `-Wall -Wextra -Wpedantic -Werror` on Clang/GCC or `/W4 /WX` on MSVC; C++ also uses `/permissive- /Zc:__cplusplus`. |
| ABI/current/minimum versions and fixed-width values | The C contract pins v1/current/minimum to 1, all ABI boolean/PCM/reset values and widths, and all 12 generated canonical error values. `aivs_contract_mapping_test` independently checks generated C++ mappings. |
| Every public v1 structure | `voice_engine_abi_v1_layout.h`, compiled by both languages, pins 19 structure sizes, 8-byte alignments, every field offset, every two-entry reserved array, and the API table's four-entry reserve on a 64-bit pointer ABI. |
| Initialization macros | `voice_engine_abi_v1_initializers.h`, executed by both consumers in Debug and Release, pins all 13 published initializer macros, including nested prefix/version state, defaults, NULL pointers, zero counts, and reserves. |
| Eight operations and factory signatures | C `_Generic` assertions and C++ `std::is_same` assertions pin the factory and all eight function-pointer typedef/member signatures, including `AIVS_VOICE_ENGINE_CALL`. The table layout manifest pins all eight slots. |
| Table completeness and negotiation | Header helper tests cover invalid/no-overlap/highest-overlap selection, exact/short/large capacity, bounded clearing, full-table completeness, wrong version, reserve, and missing-slot rejection. The dynamic Mock matrix additionally proves a future host range `[1,2]` selects and returns a complete v1 table. |
| Failure atomicity and caller-owned buffers | The C helper test pins output clearing canaries, pointer/count rules, checked PCM byte/range arithmetic, exact/disjoint/overlap rules, capacity, generation exhaustion, and bounded process-result normalization. The real Mock dynamic and matrix tests cover info/process/prepare/reset/metrics failure normalization without modifying caller PCM or metadata. |
| Exactly one Mock plugin export | macOS/Linux inspect the actual shared library with `nm`. MSVC now registers the same CTest name using `dumpbin /NOLOGO /EXPORTS`; fixture tests prove the parser accepts only `aivs_voice_engine_get_api` and rejects an additional export. |
| Canonical drift fails | `voice_engine.h` consumes generated `generated_contracts_c.h`; `pnpm contracts:check` compares checked-in outputs with `version.json` and `error-codes.json` without rewriting. Generator tests prove stale output rejection and immutable v1 behavior after an active-range advance. |
| Debug and Release are meaningful | Both C/C++ contract translation units explicitly keep `assert` enabled when the build defines `NDEBUG`. This prevents Release CTest from silently compiling away the runtime contract assertions. |

Existing Task 6 dynamic lifecycle, loader, failure-matrix, Unix export, layout,
canonical generator, and documentation evidence was retained. No duplicate API,
new audio semantics, or speculative ABI surface was added.

## RED/GREEN evidence

- Baseline Debug and Release targeted tests passed 7/7, but their compile
  commands had no warning gate, the header was not the first include, and no
  Windows export test was registered.
- MSVC export RED: after adding the positive and negative `dumpbin` fixtures,
  the positive CTest failed in the old script with `Mock VoiceEngine export
  inspection arguments are required`. GREEN added the minimal MSVC invocation
  and export-table parser; the single factory fixture passes and the extra
  symbol fixture is rejected.
- Strict Release RED: after enabling warnings as errors, both Release contract
  targets failed with 16 unused-variable diagnostics. This proved `NDEBUG` had
  compiled every runtime `assert` out of the original Release gate. GREEN keeps
  assertions active inside the two test translation units; the same strict
  Release build and all contract tests then pass.
- The added reserved-shape, initializer, canonical-value, helper-edge, and
  dynamic future-range assertions passed immediately. They protect behavior
  already present in the frozen ABI; no false implementation RED is claimed.

## Files

- Contract consumers and manifests:
  `tests/contract/voice_engine_c_abi_c_test.c`,
  `tests/contract/voice_engine_c_abi_cpp_test.cpp`,
  `tests/contract/voice_engine_abi_v1_layout.h`, and
  `tests/contract/voice_engine_abi_v1_initializers.h`.
- Dynamic boundary: `tests/runtime/mock_voice_engine_contract_matrix_test.cpp`
  and `tests/runtime/mock_voice_engine_exports.cmake`.
- MSVC parser evidence: `tests/fixtures/msvc-exports-factory-only.txt` and
  `tests/fixtures/msvc-exports-extra-symbol.txt`.
- Portable test configuration: `tests/CMakeLists.txt`.
- This evidence: `.superpowers/sdd/2026-08-19-phase-0-5-infrastructure-v1/task-14-report.md`.

## Verification

- Debug configure/build/CTest — passed 21/21.
- Release configure/build/CTest — passed 21/21, with active contract assertions.
- Target compile-command inspection — confirmed `-std=c17` / `-std=c++20`,
  extensions disabled by CMake, and `-Wall -Wextra -Wpedantic -Werror` on the
  two macOS consumer translation units.
- `pnpm contracts:check` — passed; generated mappings are current.
- `node --test tools/scripts/contracts.test.mjs` — passed 6/6.
- Root `pnpm test` — passed: contracts 6/6, doctor 7/7, native staging 3/3,
  bundle verification 2/2, and desktop Vitest 8/8.
- `cargo fmt --all -- --check` — passed.
- `cargo check --workspace --locked` — passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` — passed.
- `cargo test --workspace --locked` — passed; CTest-owned native fixture cases
  remain intentionally ignored by the generic Cargo invocation and passed in
  both CTest configurations above.
- `git diff --check` and staged diff check — passed before the implementation
  commit.

## Commit

- `dd12752` — `test(runtime): close VoiceEngine ABI contract gate`

## Limitations

- Execution evidence is macOS arm64 only. No Windows x64/MSVC build, DLL load,
  or real `dumpbin` result is claimed; Task 18 owns that acceptance evidence.
- The MSVC parser fixtures validate representative tool output and rejection
  logic, but they do not substitute for running the configured test against the
  produced Windows DLL.
- The contract gate does not introduce Audio Engine/device/JUCE/ONNX/AI/Python,
  UI, cloud, packaging, or model semantics.
