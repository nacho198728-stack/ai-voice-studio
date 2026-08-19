# Runtime

Owns the isolated native `voice-runtime` process, explicit-path dynamic loader,
stable plugin-facing boundary, and bounded control pipeline. It is separate from
the desktop control plane for failure isolation; stdout remains byte-for-byte
protocol-only.

RuntimeHost starts `voice-runtime` with explicit Unicode-safe arguments for the
plugin, Mock work bound, log directory, effective log level, and host process
generation. The Runtime never reads logging configuration from environment
variables or the current working directory. Its non-global spdlog instance
writes unified JSONL synchronously to stderr and `voice-runtime.jsonl`; logger
destruction flushes the file. Initialization failure exits with an actionable
stderr diagnostic before any stdout frame is written.

No logging dependency enters the Mock engine target. In particular,
`mock_process_audio` contains no log call, allocation, or lock and continues to
update only its existing relaxed atomic metrics on the hot path.
