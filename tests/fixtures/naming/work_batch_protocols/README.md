# Work batch protocols

[work-batch-whole-scope.js](work-batch-whole-scope.js) retains a whole JavaScript
scope containing referenced empty callbacks, indexed maps, and batch methods
with timer/clock dependencies.

[naming_work_batch_protocols.rs](../../../naming_work_batch_protocols.rs)
checks exact no-op evidence and reference edits, indexed-map initializer proofs,
and batch-loop methods. Original and alpha scopes are positives. Mutations
exercise parameters/directives/effects in no-ops, reassigned targets or donors,
map callback capture and split initialization, numeric loop order, branches,
and timer receiver/delay effects.

The suite distinguishes naming a referenced empty function from the policy for
unused generated stubs. Preserve those independent cases and the difference
between ordinary and immediate split-output initialization proofs.
Keep fixture bytes, native names, lexical parents, and effect order intact;
do not execute the recovered scope. Read the test's exact protocol replacements
and the [naming fixtures guide](../README.md) next.
