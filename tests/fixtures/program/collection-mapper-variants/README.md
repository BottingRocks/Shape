# Collection mapper and header child variants

These JavaScript function excerpts isolate descriptor payload mapping and encoded-header copying from their surrounding collection factories.

[`immutable-alias-carrier-mapper.js`](immutable-alias-carrier-mapper.js), its alternate, and the grouped mapper are positive `buildBeaconDescriptorPayloads` witnesses. [`immutable-alias-carrier-collector.js`](immutable-alias-carrier-collector.js) and the grouped collector cover `collectEncodedHeaders`. [`encoded-capture-reuse-mapper.js`](encoded-capture-reuse-mapper.js) retains a separate layout for comparison; the consumer does not include it in its positive loop.

Consumed by:

- [naming_collection_api_variants.rs](../../../naming_collection_api_variants.rs)

Tests require the exact `payload: void 0` placeholder followed by the owned payload write. Header controls distinguish an uninitialized or `void 0` key from effectful or different initializers, and require the loop-assigned key to be copied.

Keep these controls independent. Nested donor functions cannot supply the mapper write or collector assignment; captures are provided by the static test wrapper.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../collection-carrier-layout/README.md) and the [program fixture overview](../README.md).
