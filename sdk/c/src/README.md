# C SDK implementation

## Purpose

This directory implements the C boundary around the Rust engine. The
`shape-c` package produces shared and static libraries named `shape`; the
language bindings use this ABI rather than calling Rust internals.

## Files and responsibilities

- [`lib.rs`](lib.rs) exports the functions declared in
  [`shape.h`](../include/shape.h), validates UTF-8 arguments, forwards JSON
  requests to `engine::sdk`, and catches Rust panics as error envelopes.
- It owns returned C strings, boxes immutable decode and kernel handles, and implements
  their release functions. `shape_version()` uses static storage instead.
- [`../Cargo.toml`](../Cargo.toml) selects `cdylib` and `staticlib` outputs,
  depends on the root `shape` engine, and configures the mimalloc dependency.

Keep exported signatures and ownership rules aligned with the public header.
The command implementations live in the engine's SDK module, outside this
directory.
Named `shape_call_v2` forwards to the portable operation provider; legacy
`shape_call` retains its historical defaults. `shape_artifact_*` aliases the
existing validated decode handle allocation. Direct decode/encode/explain/schema
functions reuse it without reparsing. `shape_kernel_*` holds one parsed kernel
and preserves source-identity checks for program tracing.

## Build and check

From the repository root, check this package without building binding copies:

```sh
cargo check -p shape-c
cargo test -p shape-c --lib
```

For usable release libraries and refreshed Python, Node.js, and Go copies on
macOS or Linux, run:

```sh
scripts/build-sdk.sh
```

The unit tests exercise the public ABI, validation, error envelopes and
ownership. The script applies the macOS install-name and stripping settings
needed for linking, and rebuilds the Node addon. The parent guide describes direct builds
and the ABI's error and threading contracts.

## Navigation

- [C SDK guide](../README.md)
- [Shared SDK guide](../../README.md)
