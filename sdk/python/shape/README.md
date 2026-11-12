# Python SDK package

`shape` is a Python 3.9+ standard-library ctypes package. The main
[Python SDK guide](../README.md) contains complete runnable workflows and the
operation coverage table. [`py.typed`](py.typed) includes annotations for users.

The native boundary in [`_native.py`](_native.py) lazily resolves the library,
serializes strict JSON with exact Python integers, declares ownership/signatures,
and turns native envelopes into `ShapeError`. Named APIs require contract v2;
legacy `call` and `Generator` continue to use the older compatible interface.

[`artifact.py`](artifact.py) owns the `LoadedArtifact` interface and direct
`decode`/`encode`/`encode_signals`/`explain`/`schema` operations.
[`kernel.py`](kernel.py) provides `LoadedKernel` for compiled VM reuse.
Both reuse the coordinated handle lifetime in [`_generator.py`](_generator.py):
close rejects new calls, waits for active work and frees once. Serialized
exports are independent snapshots; mutating them cannot change native state.

The other modules group source, trace, naming, signal, acquisition, framing and
superpack operations by domain. [`cache.py`](cache.py) defines immutable
per-call Disabled/Directory/Environment policies. It never mutates process
environment. Memory reuse, disk kernel caching and caller-owned persisted
artifacts remain distinct choices.

See the [test guide](../tests/README.md) for uv commands and public fixtures.
