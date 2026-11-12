# C SDK headers

## Purpose

This directory defines the public C ABI used by native callers and the Python,
Node.js, and Go SDKs. The Rust SDK calls the engine directly.

## Files and responsibilities

- [`shape.h`](shape.h) declares named v2 JSON calls, immutable `ShapeArtifact`
  and `ShapeKernel` handles, direct signal operations, and the compatibility
  `ShapeGenerator` API. It includes
  C++ linkage guards and documents ownership and integer precision.
- The matching implementation is [`../src/lib.rs`](../src/lib.rs).

Command and generator-call results are JSON envelopes. Release returned strings
with `shape_free()` and generator handles with `shape_generator_free()`.
Named handles use `shape_artifact_free()` and `shape_kernel_free()` after all
concurrent calls finish. See [`operations.json`](../../operations.json) for
the complete v2 request contract and explicit per-call cache policy.
`shape_version()` returns static storage and must not be freed. Preserve JSON
integers above 2^53 without converting them through floating point.

## Build and check

From the repository root, build the release native library and binding copies
on macOS or Linux:

```sh
scripts/build-sdk.sh
```

Check that the header is self-contained without linking a program:

```sh
cc -std=c11 -Wall -Wextra -fsyntax-only -x c sdk/c/include/shape.h
c++ -Wall -Wextra -fsyntax-only -x c++ sdk/c/include/shape.h
```

Use `-I sdk/c/include` when compiling a consumer. See the parent guide for
platform-specific linking commands and the complete command contract.

## Navigation

- [C SDK guide](../README.md)
- [Shared SDK guide](../../README.md)
