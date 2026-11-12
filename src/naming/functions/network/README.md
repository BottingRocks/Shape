# Request and network-facing rules

The sibling [network.rs](../network.rs) declares these internal
[function rules](../README.md). Their claims range from opaque request callback
protocols to closed URL/route matching bodies; those are different proof scopes.

| File | Responsibility |
| --- | --- |
| [request_open_metadata.rs](request_open_metadata.rs) | Ordered request-metadata callback protocol |
| [xhr_open_metadata.rs](xhr_open_metadata.rs) | XHR open metadata recognition |
| [route_pattern_canonicalizer.rs](route_pattern_canonicalizer.rs) | Complete request URL and route-pattern matcher |
| [url_query_rebuild.rs](url_query_rebuild.rs) | Query rebuild protocol |
| [ice_candidate.rs](ice_candidate.rs), [ice_emitter.rs](ice_emitter.rs) | Candidate processing and ICE emission |

Request metadata recognition does not infer a native transport identity.
It shares owned query predicates with the browser query module, while retaining
its own ordered callback proof and scoped participants.

The route matcher projects the whole body against its
[reference grammar](route_pattern_canonicalizer.reference.js): local bindings
are alpha-renamed, captures resolve by identity, and globals retain allowed
spellings. Operators, regex flags, argument order, throws and returns remain
part of the tree. Captures must be immutable and free of redeclarations;
unknown identifiers do not become invented host globals.

See [metadata tests](request_open_metadata_tests.rs) and
[route matcher tests](../../../../tests/naming_route_pattern_canonicalizer.rs).
For the shared query parser start with [browser rules](../browser/README.md).
