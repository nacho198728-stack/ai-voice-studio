# ADR-001: Tauri and Rust control plane

- Status: Accepted
- Date: 2026-08-19

## Context

AI Voice Studio needs a local desktop composition root for a web-based product
surface, validated configuration, lifecycle control, telemetry, and the isolated
C++ Runtime established by ADR-002. Native engine code must not share the desktop
process. The required Phase 0.5 path is:

`Tauri webview -> typed command -> Rust RuntimeManager -> IPC -> C++ Runtime -> VoiceEngine C ABI -> Engine`

The desktop boundary must remain small, auditable, offline, and portable across
the macOS and Windows targets planned for the product. Phase 0.5 exposes only
runtime lifecycle, capability, and deterministic Mock summary data; it does not
define audio, model, cloud, account, or consumer workflows.

## Decision

Use Tauri 2 as the desktop shell and Rust as the control plane. The Tauri crate
is a thin adapter: it resolves Tauri-owned artifacts and application data,
initializes validated configuration and telemetry once, owns one
`RuntimeManager`, registers five typed input-free commands, and performs bounded
shutdown. Runtime process semantics stay in `apps/runtime-host`.

The webview receives only bounded camel-case DTOs and bounded public error codes.
It cannot receive raw IPC payloads, stderr, host paths, process identifiers, or
PCM/sample arrays. The command allowlist is `start_runtime`,
`get_runtime_status`, `get_runtime_capabilities`, `run_mock_pipeline`, and
`stop_runtime`; there is no generic shell, process, filesystem, dialog, network,
clipboard, updater, or plugin bridge.

## Rationale

Tauri preserves a compact web UI while moving native authority into memory-safe
Rust. Rust can reuse the existing typed contracts, configuration, telemetry, and
`RuntimeManager` without another lifecycle implementation. The separate C++
Runtime continues to contain plugin and engine failure modes, so choosing Tauri
does not weaken the process and C ABI boundaries recorded by ADR-002.

The result is a narrower trusted surface than a general browser runtime plus
Node integration, while still supporting a productive, testable frontend. It
also keeps operating-system packaging concerns at the adapter edge instead of
leaking them into RuntimeHost or the engine ABI.

## Security and process boundaries

- The only webview-to-native authority is the explicit five-command allowlist;
  every Phase 0.5 command is input-free.
- The main window is local-only. A restrictive content security policy blocks
  network connections, media, embedded frames, objects, and form targets; release
  devtools and remote navigation are disabled.
- The Rust process never loads the VoiceEngine dynamic library. It starts and
  supervises the C++ sidecar, which alone loads and invokes the C ABI plugin.
- Errors crossing into the webview are stable, bounded, and redacted. Internal
  paths, stderr, process IDs, and native payloads remain below the adapter.
- PCM is created and consumed within the native Mock pipeline and is never
  serialized through IPC or Tauri.

## Lifecycle ownership

The Tauri application owns one retained telemetry guard and one managed command
service backed by one `RuntimeManager`. Initialization is executed on Tauri's
async runtime because RuntimeHost owns asynchronous process supervision. Command
calls do not hold a global mutex across awaits. Window/application exit requests
a bounded, ordered Runtime shutdown and reap; RuntimeHost's kill-on-drop behavior
is the final orphan-prevention fallback.

The frontend owns presentation state only. It disables conflicting actions while
a command is pending, renders the six manager states, and recovers controls after
bounded command failure. It does not infer or duplicate native lifecycle state.

## Packaging implications

Native builds are staged deterministically by profile and Rust target triple into
ignored Tauri build directories. The C++ Runtime is packaged as a sidecar; the
Mock library and versioned `config/config.json` are packaged resources. Release
resolution uses only the installed executable and Tauri resource directory.
Development resolution uses only compile-time repository-relative locations and
returns an actionable build instruction when staging is absent. Runtime startup
does not search the current working directory, `PATH`, Homebrew, or environment
variables.

Artifacts carry target-qualified names where Tauri or dynamic-library collision
avoidance requires them. Platform-specific `.exe`, `.dll`, `.dylib`, and `.so`
rules are centralized in the staging and resolution seam. Windows execution and
installer validation remain later CI/release work; signing, notarization,
auto-update, and installers are outside this decision.

## Alternatives considered

### Electron

Electron offers mature web tooling, but bundling Chromium and Node would enlarge
the distribution and trusted runtime surface. Preventing Node, shell, filesystem,
and remote-content authority would require more policy around capabilities that
this product does not need.

### Qt

Qt is a strong native cross-platform toolkit, but it would add another C++ UI
ownership domain beside the isolated Runtime and make direct native coupling more
tempting. It would also forgo reuse of the project's typed web presentation seam.

### Flutter

Flutter provides a consistent renderer, but it would introduce Dart and a second
application control-plane ecosystem without improving the established Rust
RuntimeManager boundary. Native process, configuration, and telemetry contracts
would still need a bridge.

### All-JUCE shell

An all-JUCE application could share C++ code, but that is precisely the wrong
ownership direction: engine/plugin faults and ABI concerns would be drawn toward
the product UI process. JUCE may be evaluated for later audio needs, but it does
not replace the isolated Runtime boundary or justify making the desktop shell a
native-engine host.

## Consequences

- The desktop shell remains a small adapter over existing Rust services rather
  than a second process manager.
- Frontend and command behavior can be tested independently through a typed port,
  while a staged integration test proves the real C++/C ABI Mock chain.
- Tauri, Rust, JavaScript, and native artifact versions and layouts must be kept
  compatible and verified for every supported target.
- Platform packaging needs explicit resource staging and bundle-layout tests;
  macOS validation alone does not establish Windows acceptance.
- The deliberately narrow Phase 0.5 UI cannot grow new native authority without
  a reviewed command, capability, DTO, CSP, lifecycle, and test change.
- ADR-003 remains reserved for the Phase 1 Audio Engine decision and is not made
  by this ADR.
