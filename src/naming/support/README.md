# Scoped AST support for naming rules

The sibling [support.rs](../support.rs) declares internal helper bags used by
[function rules](../functions/README.md) and [operation rules](../operations/README.md).
Return to the [naming guide](../README.md) for the public entry points.

| File | Responsibility |
| --- | --- |
| [seeded_flow.rs](seeded_flow.rs) | Plain capture origins, data/callable distinctions and global paths |
| [interaction.rs](interaction.rs), [reassembly.rs](reassembly.rs) | Scoped expressions, locals and call/update shapes |
| [callable_identity.rs](callable_identity.rs) | Candidate-local callable proof allowing exact self-stores |
| [ordered_numeric.rs](ordered_numeric.rs) | Small ordered numeric expression algebra |
| [vector_state.rs](vector_state.rs), [selector_graphics.rs](selector_graphics.rs) | Vector and selector/graphics predicates |

These helpers have different acceptance boundaries. For example,
`seeded_flow::plain_origin` admits ordinary `var`/`let`/`const`
declarations and refuses resource declarations when claiming an origin.
A data capture is not a callable-origin witness. Keep each consumer's ownership
and write checks even when another helper has a similarly named predicate.

`callable_identity` handles initialized lexical captures with exact
`slot = slot` stores; unrelated, compound and property writes cannot establish
that identity. `ordered_numeric` retains literal bit patterns, operators and
scoped operands without commutation or rewriting.

These are proof primitives, not standalone proposal producers. Read their
consumers and the [shared proof layer](../shared.rs) before widening a helper;
[interaction tests](../../../tests/naming_interaction_sample_roles.rs) and
[captured protocol tests](../../../tests/naming_captured_signal_protocols.rs)
exercise several of these predicates through naming.
