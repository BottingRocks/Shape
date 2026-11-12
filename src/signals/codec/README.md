# Signal wire primitives

This is the byte-writing layer beneath the [signals subsystem](../README.md).
[mod.rs](mod.rs) implements token hashing, the per-signal keystream, number and
string writers/readers, container framing, custom base64 and the outer cipher.
The [payload layer](../payload/README.md) composes those primitives with explicit
profiles, reports and header policy.

Use `encode_signal` / `try_encode_signal` and `decode_signal` for a signal
body: a big-endian seed prefix precedes the masked writes. The keystream burns
an initial block, so changing reader initialization independently of the writer
breaks decoding. String interning belongs to the writer/reader state; field order
therefore affects subsequent bytes.

`string_units` reads/writes exact UTF-16 units, including lone surrogates.
The existing `string` API keeps its Rust-string convenience behavior. The
[schema consumer](../wire/mod.rs) checks source algorithms and cursor/table
scope before using these primitives; primitive round trips alone do not
certify a recovered program. See the [wire schema](../../../docs/signals/schema.md#versioned-wire-metadata-and-library-consumer).

[layout.rs](layout.rs) provides `Layout`, `Field` and `SignalValue`, plus
`encode_layout` and `decode_layout`. Its data-driven fields cover numbers,
strings, bytes, packed booleans, optional values and arrays (including reverse
iteration). This is a codec write program supplied by the caller; the
[source extraction grammar](../extract/README.md) is a separate representation.
Check the field-specific validation when extending either direction.

`Session` carries token/alphabet/keys; `Build` carries cipher geometry. The
module includes a fixed default build, so inspect configuration when adapting
these low-level helpers to another input. The payload profile API validates
explicit geometry rather than inferring it from successful decryption.

Known-answer and round-trip tests are embedded in [mod.rs](mod.rs) and
[layout.rs](layout.rs). For framing and generic report composition, continue at
[report.rs](../payload/report.rs) and the [generator API](../../../docs/generator/api.md).
