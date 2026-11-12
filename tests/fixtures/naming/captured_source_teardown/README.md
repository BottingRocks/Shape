# Captured source teardown

These parents preserve captured subscription cleanup and sample expansion in complete enclosing scopes.
The static JavaScript inputs include [captured-source-parent.js](captured-source-parent.js) and
[sample-expansion-parent.js](sample-expansion-parent.js).

[naming_captured_source_teardown.rs](../../../naming_captured_source_teardown.rs) is the integration consumer.
Both scopes supply cleanup positives. In-memory controls change capture stores, whole bodies, sample protocols, and global bindings to require refusal at the same target.

These are source-analysis fixtures, not runnable browser programs.
Keep complete lexical parents, declaration/reference identity, native spellings,
and statement order. Tests locate exact source bodies and, in some cases,
assert byte spans; even whitespace changes can invalidate the control.
Preserve each independent layout rather than deriving one from the other.
Read the linked assertions before editing, then continue with the
[naming fixtures overview](../README.md) and [naming implementation](../../../../src/naming/README.md).
