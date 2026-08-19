# Capability ownership

This Rust crate owns generation-free Phase 0.5 capability vocabulary and
normalization of the native Runtime's minimal `GetCapabilities` JSON. It exposes
validated platform, architecture, backend, Mock identity, and availability
values. It does not expose a profile builder or any type that can attach those
values to a RuntimeManager generation.

RuntimeHost owns the opaque query observation, the private truth-table mapping,
and the generation-bearing serialized profile. It attaches the authoritative
generation inside its actor and rejects stale outcomes before profile creation.
No layer inspects or infers hardware, audio devices, drivers, benchmarks, or
machine identity.
