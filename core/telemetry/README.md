# Unified telemetry

`ai-voice-telemetry` owns Rust telemetry initialization and the shared Phase
0.5 JSONL record contract. Each UTF-8 line is one JSON object containing:

- `timestamp`: RFC 3339 UTC with fractional seconds;
- `component`: non-empty UTF-8, at most 64 bytes;
- `level`: `trace`, `debug`, `info`, `warn`, or `error`;
- `message`: UTF-8, at most 512 bytes;
- optional unsigned `request_id` and `generation`.

The crate installs one process-global `tracing` subscriber with a schema-
enforcing event formatter. Direct `tracing` events cannot bypass the contract:
the formatter derives lowercase level from event metadata, normalizes missing
component/message values, bounds the four string fields, accepts only unsigned
correlation fields, and discards unknown fields. Initialization accepts a
validated `LoggingPolicy`, which clamps trace/debug to info unless debug was
explicitly enabled; it never accepts a bare level as logging authority.

Initialization creates the explicit absolute directory, opens
`runtime-host.jsonl`, and routes the same records to stderr. A second
initialization returns a typed error; a directory or file failure occurs before
global registration. `TelemetryGuard` owns tracing-appender's bounded
background writer, and must live for the host process lifetime; dropping it
drains and flushes accepted records.

The Phase 0.5 file is append-only and has no rotation, retention, upload, or
secret-redaction service. Call sites must log fixed diagnostics rather than PCM,
payload bodies, secrets, model contents, or resource paths. Phase 1 must define
a realtime-safe telemetry transport before any audio callback may emit logs;
the Mock `process_audio` hot path only updates relaxed atomic metrics.
