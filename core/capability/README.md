# Capability ownership

This Rust crate owns the stable Phase 0.5 capability types and normalization of
the native Runtime's minimal `GetCapabilities` JSON. The schema reports only
platform, architecture, canonical Runtime/protocol versions, backend and Mock
engine availability, plus manager health/generation. It does not inspect or
infer hardware, audio devices, drivers, benchmarks, or machine identity.

Query observations are explicitly `not_evaluated`, `inconclusive`, or a
validated native observation and carry the RuntimeManager generation they came
from. Profile construction rejects stale generations. Runtime reachability is
derived from manager health independently of Mock engine availability.
