# Runtime

Owns the isolated native `voice-runtime` process, explicit-path dynamic loader,
stable plugin-facing boundary, and bounded control pipeline. It is separate from
the desktop control plane for failure isolation; stdout remains byte-for-byte
protocol-only.

RuntimeHost starts `voice-runtime` with explicit Unicode-safe arguments for the
plugin, Mock work bound, log directory, effective log level, debug-authority
bit, and host process generation. Trace/debug plus a false or missing
`--debug-enabled` argument is rejected before logger initialization. The
Runtime never reads logging configuration from environment
variables or the current working directory. Its non-global spdlog instance
writes unified JSONL synchronously to stderr and `voice-runtime.jsonl`; logger
destruction flushes the file. Initialization failure exits with an actionable
stderr diagnostic before any stdout frame is written. Logger initialization
rejects an invalid UTF-8 component, while invalid message byte sequences are
replaced with U+FFFD before the result is truncated at a UTF-8 boundary. Each
logger installs its own non-throwing spdlog error handler. A file sink failure
after initialization is contained silently so spdlog cannot print exception
details, absolute paths, or non-JSON text to stderr; the handler performs no
retry or fallback I/O and therefore cannot recurse into the failed sink.

No logging dependency enters the Mock engine target: configure-time target
guards pin its direct dependency surface, while a reviewed source hash pins the
complete `mock_process_audio` body. Existing multi-call behavior tests verify
zero stderr records, deterministic output, and the exact relaxed atomic
metrics. The hot path therefore contains no log call, allocation, or lock.
