# Signal recognition and field binding

This layer turns source-derived contracts into the named schema described in the
[parent guide](../README.md). Start at [mod.rs](mod.rs):
`analyze_program_signals` obtains inputs and contracts, optionally gathers role
claims, builds slot views, runs every registered recognizer, then binds and renders
slots with exactly one candidate.

| File or guide | Maintainer entry point |
| --- | --- |
| [slot_view.rs](slot_view.rs) | Slot-local emission, host API and callback evidence exposed to recognizers |
| [write_sites.rs](write_sites.rs) | Source slices and def-use evidence for written values |
| [roles](roles/README.md) | Proven function, class and binding roles used as anchors |
| [field_solver.rs](field_solver.rs) | Field assignment through uniqueness constraints |
| [value_facts.rs](value_facts.rs), [described_fields.rs](described_fields.rs) | Flow-aware value descriptions and field binding |
| [buffer_writes.rs](buffer_writes.rs), [layout_nodes.rs](layout_nodes.rs) | Emitted-buffer evidence and projected layout operations |

Recognition and naming are separate decisions. No candidate produces
`unidentified`; multiple candidates produce `ambiguous`; a binding or rendering
failure produces `refused`. An empty emission layout takes the `no_emission`
path first, except a declared-not-emitted slot (the producer completes without
`next`) that one no-data signal claims: it is listed under `signals` with
`type: "no_data"`, `fields: {}` and its catalog `effect`. `layouts_only` attaches positional layouts to eligible diagnostics;
it does not make an unresolved slot a named signal.

Role scope matters: `slot_roles` uses the innermost proven function around a
slot's anchors, while `roles_inside` includes the producer and callbacks and
`roles_enclosing` describes surrounding functions. A shared group body's roles
must not be mistaken for evidence specific to every slot. The analysis memo is
owned by one analysis and shared by recognizers and binders through `FinderCtx`.

Put signal-specific rules in the [catalog](../catalog/README.md); shared helpers
here are grouped by technique. Generated names, routing positions and salts are
not recognition inputs. [catalog_tests.rs](catalog_tests.rs) checks the registry
and structural source rules; [program_signal_projection.rs](../../../tests/program_signal_projection.rs)
checks projection. See [schema.rs](../schema.rs) for rendering and binding types
and the [flags guide](../flags/README.md) for byte-to-bit fallback behavior.
