# Tests

Owns C/C++ contract, runtime integration, fixture, and benchmark verification.
CTest is the native test harness and injects CMake-built sidecar/plugin paths
into Rust integration tests that are intentionally ignored under bare Cargo.
