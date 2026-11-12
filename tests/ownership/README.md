# Nested ownership regressions

[joined_observables_teardown.rs](joined_observables_teardown.rs) is included
with `#[path]` by [naming_joined_observables.rs](../naming_joined_observables.rs).
It imports the parent's naming and named-expression helpers; it is not an
independent Cargo integration target. Run it through
`cargo test --test naming_joined_observables` from the repository root.

The module consumes both full parent layouts in
[joined_observables](../fixtures/naming/joined_observables/README.md).
It pins cleanup declaration spans and reference edits, verifies the owning
join factory, and tests alpha variants. Negative controls require the actual
target to remain indexed and unapplied, so a missing declaration or an unrelated
label cannot masquerade as a successful refusal.

Preserve parent/child lexical ownership, exact input spans, and private
function-expression versus external-binding identity. These controls analyze
source without executing it. Read the parent target and its nested tests before
changing a fixture, then continue with the [tests overview](../README.md).
