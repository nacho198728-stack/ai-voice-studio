# Configuration ownership

This Rust crate owns the strict schema and read-only loading APIs for product
configuration. It does not discover paths, create defaults, rewrite invalid
files, enumerate audio devices, or initialize logging.
