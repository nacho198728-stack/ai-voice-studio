# Runtime

Owns the isolated native `voice-runtime` process, explicit-path dynamic loader,
stable plugin-facing boundary, and bounded control pipeline. It is separate from
the desktop control plane for failure isolation; stdout remains protocol-only.
