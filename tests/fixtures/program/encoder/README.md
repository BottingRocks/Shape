# Owned value encoder controls

JavaScript factory/class source preserves tuple headers, keyset assembly, payload encoding, and the callback that consumes the encoded output.

[`guarded.js`](guarded.js) is the positive `ValueEncoder` witness. [`reversed-assembly.js`](reversed-assembly.js), [`unreachable-header.js`](unreachable-header.js), and [`foreign-consumer-length-writer.js`](foreign-consumer-length-writer.js) are distinct negative controls. Sibling `.provenance.json` files record source spans, identities, and mutations.

Consumed by:

- [naming_thread_class_encoder.rs](../../../naming_thread_class_encoder.rs)

Reversing keyset/payload concatenation, throwing before the header, or substituting a foreign length writer must each fail. Positive tuple-call and alpha variants still require exact receivers, writer identity, fresh arrays, reachable owned effects, and closed output use.

Keep the positive and all three negatives byte-for-byte. Do not regenerate them or treat their provenance status text as a substitute for current consumer assertions.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../README.md) and the [program fixture overview](../README.md).
