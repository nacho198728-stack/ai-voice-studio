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
| Initialization macros | `voice_engine_abi_v1_initializers.h`, executed by both consumers in Debug and Release, pins all 13 published initializer macros. Test-owned literal helpers recursively check every field of nested byte views, both nested mutable byte buffers, and the nested mutable PCM output, in addition to every top-level default and reserve. |
| Eight operations and factory signatures | The factory declaration and public typedef are independently compared with a test-owned prototype returning literal `int32_t` over `const struct aivs_voice_engine_factory_request*`, `struct aivs_voice_engine_api*`, and `uint32_t`. Its Windows branch directly spells `__cdecl`; no public scalar/struct typedef or calling-convention macro is reused by the expected type. C `_Generic` and C++ `std::is_same` perform the comparisons without odr-using the export. The eight operation signatures remain independently constrained by explicit C stubs, strict diagnostics, and their table-slot type assertions. |
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
- Scalar-drift evidence: `tests/contract/voice_engine_adversarial_compile.cmake`
  and `tests/fixtures/voice_engine_unsigned_error_alias_test.{c,cpp}`.
- Portable test configuration: `tests/CMakeLists.txt`.
- This evidence: `.superpowers/sdd/2026-08-19-phase-0-5-infrastructure-v1/task-14-report.md`.

## Verification

- Debug configure/build/CTest — passed 23/23.
- Release configure/build/CTest — passed 23/23, with active contract assertions.
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
- `92f4ee5` — `test(runtime): make ABI expectations independent`
- `74cf631` — `test(runtime): reject coordinated ABI scalar drift`

## Limitations

- Execution evidence is macOS arm64 only. No Windows x64/MSVC build, DLL load,
  or real `dumpbin` result is claimed; Task 18 owns that acceptance evidence.
- The MSVC parser fixtures validate representative tool output and rejection
  logic, but they do not substitute for running the configured test against the
  produced Windows DLL.
- The contract gate does not introduce Audio Engine/device/JUCE/ONNX/AI/Python,
  UI, cloud, packaging, or model semantics.

## Review fix round 1

- Closed the self-referential factory-signature gap. Each language consumer now
  owns an expected function-pointer type with the literal
  `(const aivs_voice_engine_factory_request_t*, aivs_voice_engine_api_t*,
  uint32_t) -> aivs_error_code_t` prototype. Windows writes `__cdecl` directly;
  other platforms use their default convention. Both the exported declaration
  and `aivs_voice_engine_get_api_fn` must equal this independent type. The checks
  are unevaluated (`_Generic` / `decltype`), so the header-only contract
  executables do not acquire an unresolved factory reference.
- Closed the three composite-initializer gaps. Test-owned field helpers check
  all byte-view, mutable-byte-buffer, and mutable-PCM-buffer fields against
  literal zero/NULL/default expectations. They run against both standalone and
  nested expansions, so `AIVS_INITIALIZE_REQUEST_INIT.configuration_utf8`, both
  `AIVS_ENGINE_INFO_INIT` buffers, and
  `AIVS_PROCESS_AUDIO_RESULT_INIT.output` are recursively frozen, including
  nested versions, pointers, capacities/counts, PCM metadata, and reserves.
  Existing explicit checks retain every composite top-level reserve.
- These changes strengthen tests around the already-correct frozen ABI. The new
  assertions passed immediately; no production defect or false implementation
  RED is claimed. `runtime/api/voice_engine.h`, canonical JSON, ADR-002, and the
  main plan were unchanged, and ADR-003 remains absent.
- Fresh review-fix verification passed: focused Debug and Release ABI tests
  2/2; full Debug and Release CTest 21/21 each; canonical generator tests 6/6;
  root pnpm tests; and Cargo fmt, locked workspace check, clippy with warnings
  denied, and locked workspace tests. Windows execution remains deferred to
  Task 18 and is not claimed.

## Review fix round 2

- Closed the remaining coordinated-component drift gap. The test-owned factory
  type no longer uses `aivs_error_code_t`,
  `aivs_voice_engine_factory_request_t`, or `aivs_voice_engine_api_t`: it spells
  `int32_t`, both public structure tags, and `uint32_t` directly. Windows still
  spells `__cdecl` literally and other platforms use the platform default.
  Both the exported declaration and public factory typedef must equal this
  independent type in C and C++.
- The C consumer now uses `_Generic` to require `aivs_error_code_t` to be
  exactly `int32_t`; the C++ consumer uses `std::is_same_v`. The existing
  four-byte and canonical-value assertions remain complementary evidence and
  no longer stand in for exact signed type identity.
- Added persistent adversarial compile evidence without editing the public
  header. The fixtures preload the real generated contract with its error alias
  renamed, substitute same-width `uint32_t`, and then include the real C/C++
  contract translation units. The CTest script requires compilation to fail
  specifically on `canonical error code type must be exactly int32_t`, so an
  unrelated compile failure cannot produce a pass.
- RED: before changing the oracle, both C17 and C++20 adversarial fixtures
  compiled successfully and both CTests failed with `VoiceEngine contract
  accepted unsigned aivs_error_code_t drift`. GREEN: after the independent
  type and exact-alias assertions, the normal and adversarial C/C++ cases passed
  4/4 in Debug; the negative fixtures were rejected for the required reason.
- Fresh round-2 verification passed: full Debug and Release CTest 23/23 each,
  canonical drift and generator tests 6/6, root pnpm tests, Cargo fmt, locked
  workspace check, clippy with warnings denied, locked workspace tests, and
  `git diff --check`. The frozen ABI header, canonical JSON, ADR-002, main plan,
  and ADR-003 were not changed. Windows execution remains Task 18.
