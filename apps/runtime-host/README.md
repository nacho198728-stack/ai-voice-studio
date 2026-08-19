# RuntimeHost telemetry

RuntimeHost emits structured lifecycle and request-correlation events through
`ai-voice-telemetry`. It logs generation on process lifecycle events and adds
request ID when known, without retaining a state lock or changing actor order.
Messages never contain IPC payloads, PCM, environment values, plugin/runtime
paths, or secrets. Child stderr remains independently collected into the
existing bounded tail.

The application composition root must initialize telemetry once, retain its
guard until shutdown, resolve the configuration's development log directory
against a platform-owned writable base, then construct `RuntimeManagerConfig`
from the same validated product configuration. RuntimeManager stores the
validated logging policy rather than a raw level and explicitly passes both
its effective level and debug-authority bit to each native child.
