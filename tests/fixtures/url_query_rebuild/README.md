# URL query reconstruction

[primary.js](primary.js) retains the exact JavaScript builder plus default-port,
anchor href accessor and cache setup in a minimal carrier. Its identity and
original function location are recorded in [provenance.json](provenance.json).

[The integration target](../../naming_url_query_rebuild.rs) checks
rebuildUrlWithEncodedQuery through thread_url_query_rebuild. It covers cache-hit
and anchor-normalization paths, HTTP(S) guarding, own query decode/encode work,
space-to-plus conversion and ordered URL component assembly.
[Function/proxy protocol tests](../../naming_function_and_proxy_protocols.rs)
also reuse this source.

Original, renamed and equivalent split forms are positive. Changed components,
borrowed setup, early calls/exposures and unproved alias activation are refusals;
all required capture stores must precede every permitted exposure.

Keep source bytes, accessor/default/cache relationships and mutation anchors
unchanged. The provenance record is immutable; link it rather than copying its
hash or source identifiers. Do not execute the recovered builder.
Read next: [evaluator](../../../src/naming/functions/network/url_query_rebuild.rs)
and [network rule guide](../../../src/naming/functions/network/README.md).
