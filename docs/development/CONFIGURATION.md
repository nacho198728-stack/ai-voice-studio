# Configuration and capability policy

## Ownership and resource resolution

`config/config.json` is the schema-versioned development/default contract
shipped with the repository. `core/config` owns its Rust types, semantic bounds,
and read-only byte/path loaders. The file is not a user-writable settings store.

The packaged Desktop application will resolve the shipped file through its
resource APIs and pass explicit bytes or an explicit absolute path. Neither the
configuration crate nor RuntimeManager searches the current working directory,
home directory, environment, or platform settings folders. Runtime and plugin
resource paths are deliberately absent from JSON: the Desktop resource owner
passes both paths explicitly when constructing `RuntimeManagerConfig`.

A future user settings store must use a separate platform-owned writable
location, schema, migration policy, and atomic-write implementation. It must not
modify or shadow the shipped resource implicitly.

## Validation and failure policy

Loading rejects unknown, missing, duplicate, and wrong-type fields, unsupported
schema versions or enum values, out-of-range Runtime/Mock settings, and any
Phase 0.5 audio claim other than disabled/unconfigured. Errors are typed as I/O,
invalid document, unsupported schema version, or semantic validation failures,
and include actionable context.

Invalid configuration fails closed. Loading never repairs, defaults, creates,
rewrites, migrates, or falls back to a different file. The caller decides how
to surface an error; it must not silently continue with implicit defaults.

## Phase 0.5 capability truthfulness

`core/capability` normalizes the native Runtime payload against canonical
versions from `core/contracts` and only accepts the paired native claims
`mock/aivs-mock-v1` or `unavailable/unavailable`. A capability profile records
manager health/generation and distinguishes available, unavailable, unknown,
and not-evaluated values.

No capability path reads CPU, GPU, RAM, NPU, audio devices, drivers, benchmark
tiers, or machine identifiers. Unknown platform/architecture remains `unknown`;
an unqueried connected Runtime remains `not_evaluated`; a stopped Runtime or an
explicit native unavailable response remains `unavailable`.
