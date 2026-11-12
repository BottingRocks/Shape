# Report payloads and headers

This layer wraps caller-produced signal bytes and generic report values in the
wire format used by the [signals subsystem](../README.md). It does not collect
browser observations or interpret positional `extra` bytes.

| File or guide | Responsibility |
| --- | --- |
| [mod.rs](mod.rs) | Explicit profiles, `PayloadCodec`, `Generator`, raw/container payloads and validation |
| [report.rs](report.rs) | Four-channel reports and generic-value/container conversion |
| [superpack guide](superpack/README.md) | Generic value serialization and canonical decoding |
| [headers.rs](headers.rs) | Header chunks, checksum/token binding and sibling context |
| [extract guide](extract/README.md) | Structural pinning and the saved `SeedLayout` artifact |
| [typed_json.rs](typed_json.rs) | Depth preflight and exact typed JSON consumption |

The report path packs generic values, frames container sections, prefixes the
two payload seeds, applies the cipher and emits unpadded custom-base64 text.
Header APIs then format chunks and configured sibling fields. `Payload` can
instead hold raw plaintext or an opaque container. Keep per-signal seeds and
layouts out of this layer's raw byte sections.

`Report` carries seeds and ordered omitted keysets alongside its four channels;
`ChannelReport` owns only the channels, so encoding needs that metadata from
a caller or loaded layout. Keyed entries retain order, including repeated
numeric names. Header context preserves sibling presence and values separately
from encrypted chunks.

Profiles validate cipher geometry and input limits. These payloads have no
authentication: decoding and container framing checks establish neither the
key's correctness nor signal semantics. Consult the [generator guide](../../../docs/generator/README.md)
for supported interfaces and [generator_report.rs](../../../tests/generator_report.rs),
[generator_pipeline.rs](../../../tests/generator_pipeline.rs) for integration coverage.
