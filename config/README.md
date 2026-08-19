# Product configuration

`config.json` is the checked-in, schema-versioned development/default contract.
It is validated by `core/config`; it is not a user settings file and is not
resolved relative to the process current working directory.

The future packaged Desktop shell owns resource resolution and must pass
explicit bytes or an explicit absolute path to the read-only loader. A future
user-writable settings store must use a separate platform settings location and
an explicit migration policy; it must never overwrite this shipped resource.

See `docs/development/CONFIGURATION.md` for validation and failure policy.
