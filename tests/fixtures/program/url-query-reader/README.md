# Owned URL search parameter readers

JavaScript source wrappers retain the complete URL-to-query-pair pipeline and requested-key filtering.

[`captured-query-baseline.js`](captured-query-baseline.js) uses a direct initial null/blank guard; [`captured-query-alternate.js`](captured-query-alternate.js) stages that guard. Both create an anchor, read `search.slice(1)`, decode pairs, and return values for the requested key.

Consumed by:

- [naming_url_query_reader.rs](../../../naming_url_query_reader.rs)

Both establish `readUrlSearchParameterValues`. Tuple/direct split-call and alpha variants remain positive. Negatives break the URL/key origins, tuple receiver, separators, decoding, pair order, result projection, globals, or owned effects; nested readers cannot lend the role to a wrapper.

Preserve exact mutation anchors, captures, and branch/dataflow relationships. The wrapper is static analysis input and should not be run against a browser document.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../reflection-protocols/README.md) and the [program fixture overview](../README.md).
