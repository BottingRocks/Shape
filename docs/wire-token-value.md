# Token value certification

Native stream state requires a source proof that the hash reads the supplied
public token value. Matching the hash argument's text, or decoding and reencoding
the same bytes with the same state, does not establish that value.

The shared state-definition gate resolves the local context object, its immutable
aliases, the constructor formal and the public formal with AST symbol identities.
It requires a dominating token store or an own object-literal token initializer.
Computed literal stores participate in the same proof as static member stores.
Dynamic stores, member deletion/update, unresolved object escapes and mutators,
and changes to the consumed formal value refuse native authorization. Callback
exposure is checked independently of declaration text order. Existing algorithm
body, dependency, activation, source identity, pin and witness checks still apply.

The proof is shared by ordinary global certification and per-instance readiness;
loading saved readiness regenerates it. An irrelevant property store is allowed.
A mutation in a later statement of the hash's own activation does not affect the
value already read. A helper mutation is also irrelevant when the existing
activation proof establishes that every invocation follows the read. Textual
order of a closure declaration alone is insufficient.

The analysis deliberately refuses unresolved effects. It does not interpret
arbitrary mutator helpers, dynamic keys, mutable object aliases, recursive callback
exposure, or conditional stores as value-preserving operations. Such refusal is a
support boundary, not evidence that their runtime value differs.

`tests/signal_wire_initializer.rs` exercises actual analyzer JSON, both public
loaders, wrong-token blobs, named-value refusal, saved witness regeneration and
positive cases with aliases, computed stores and mutations after the final read.
