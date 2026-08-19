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

The same host supplies an explicit absolute writable application-data base for
logging. `debug.development_log_directory` is validated as a bounded portable
relative path and joined to that base; it is never resolved against cwd, home,
an environment variable, or the executable location. Empty/dot/parent
components, absolute or drive-qualified paths, backslashes, NUL, and oversized
values fail validation. RuntimeHost passes the resolved absolute directory,
effective level, and generation to `voice-runtime` as native process arguments,
preserving Unicode through `OsStr` on Rust and `wmain` on Windows.

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

`debug.log_level` accepts `trace`, `debug`, `info`, `warn`, and `error`. When
`debug.enabled=false`, requested trace/debug is deterministically clamped to
info; warn/error remain stricter. RuntimeManager validates this gate again, so
the production launch path cannot enable verbose logging by mutating only the
level. The telemetry initializer, not the configuration loader, creates the
resolved development directory.

## Phase 0.5 capability truthfulness

`core/capability` normalizes the native Runtime payload against canonical
versions from `core/contracts` and only accepts the paired native claims
`mock/aivs-mock-v1` or `unavailable/unavailable`. It has no generation-bearing
profile constructor. RuntimeHost privately maps actor-minted observations into
the serialized capability profile, which records manager health/generation and
distinguishes available, unavailable, unknown, and not-evaluated values.
Capability queries are modeled as not evaluated, inconclusive, or observed;
every observation is bound to a manager generation, and stale observations are
rejected after restart. A connected Runtime is available independently of
whether the native payload reports the Mock engine available or unavailable.

No capability path reads CPU, GPU, RAM, NPU, audio devices, drivers, benchmark
tiers, or machine identifiers. Unknown platform/architecture remains `unknown`;
a connected Runtime is `available`, while its engine/backend observation remains
`not_evaluated` until queried; a stopped Runtime is `unavailable`, and an
explicit native unavailable response marks only the engine `unavailable`.
