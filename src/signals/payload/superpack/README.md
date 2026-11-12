# Generic value packing

The sibling [superpack.rs](../superpack.rs) owns the public value/options types
and `pack`, `unpack`, `unpack_canonical` entrypoints. This folder contains
their private implementations. The [payload parent](../README.md) uses them for
generic report channels, independently of per-signal layouts and encryption.

| File | Responsibility |
| --- | --- |
| [encode.rs](encode.rs) | Value tokens, keyset metadata, string lookup selection and byte emission |
| [decode.rs](decode.rs) | Bounded structural inverse, lookup/keyset reconstruction and exact document consumption |

`Value` distinguishes undefined, null, binary, dates and wire extensions from
ordinary JSON values. `Number` carries IEEE-754 bits in JSON; encoding still
normalizes NaNs as the source serializer does. Object input retains entry order,
but packing sorts keys by UTF-16 order and rejects duplicate own keys.

`SerializerOptions` carries ordered omitted keysets and explicit extension
policy. Decoding needs the same omitted keysets because they are absent from
the bytes. Semantic `depth_bound` truncation is distinct from resource limits:
byte, entry and nesting ceilings constrain work regardless of that option.
No JavaScript prototypes or callbacks are evaluated by this serializer.

`unpack` consumes one complete document and retains `original_bytes` even
when a legal alternative encoding is not canonical. Its `canonical` flag comes
from repacking and comparing bytes. `unpack_canonical` rejects such alternatives;
callers needing byte preservation must retain the original bytes rather than
assuming a value round trip reproduces them.

The [format reference](../../../../docs/generator/superpack.md) describes the
supported subset. [generator_superpack.rs](../../../../tests/generator_superpack.rs)
provides focused coverage; [report.rs](../report.rs) shows channel-specific options.
