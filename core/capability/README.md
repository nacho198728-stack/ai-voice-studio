# Capability ownership

This Rust crate owns the stable Phase 0.5 capability types and normalization of
the native Runtime's minimal `GetCapabilities` JSON. The schema reports only
platform, architecture, canonical Runtime/protocol versions, backend and Mock
engine availability, plus manager health/generation. It does not inspect or
infer hardware, audio devices, drivers, benchmarks, or machine identity.

The pure truth-table input is explicitly `not_evaluated`, `inconclusive`, or a
validated native evaluation. RuntimeHost owns the opaque query observation and
attaches the authoritative RuntimeManager generation inside its actor; callers
cannot mint or relabel one. RuntimeHost rejects stale generations before using
this crate's semantic mapping. Runtime reachability is derived from manager
health independently of Mock engine availability.
