# Engines

Owns VoiceEngine implementations and adapters loaded by the Runtime through the
stable C ABI. `mock/` is the dependency-free deterministic Phase 0.5 plugin; it
does not access audio devices, files, networks, AI runtimes, or model formats.
