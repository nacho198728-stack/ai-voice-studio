# Capability ownership

This Rust crate owns the stable Phase 0.5 capability types and normalization of
the native Runtime's minimal `GetCapabilities` JSON. The schema reports only
platform, architecture, canonical Runtime/protocol versions, backend and Mock
engine availability, plus manager health/generation. It does not inspect or
infer hardware, audio devices, drivers, benchmarks, or machine identity.
