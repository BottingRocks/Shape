# Captured handle termination

The stateful parent retains multiple cleanup declarations with guarded handle completion; the canonical parent protects the existing cleanup rule priority.
The static JavaScript inputs include [stateful-parent.js](stateful-parent.js) and
[canonical-parent.js](canonical-parent.js).

[naming_captured_handle_termination.rs](../../../naming_captured_handle_termination.rs) is the integration consumer.
The integration assertions require all selected declarations to be reached and keep canonical cleanup from being relabeled by the guarded-handle evaluator. Additional refusal controls live in the evaluator unit tests.

These are source-analysis fixtures, not runnable browser programs.
Keep complete lexical parents, declaration/reference identity, native spellings,
and statement order. Tests locate exact source bodies and, in some cases,
assert byte spans; even whitespace changes can invalidate the control.
Preserve each independent layout rather than deriving one from the other.
Read the linked assertions before editing, then continue with the
[naming fixtures overview](../README.md) and [naming implementation](../../../../src/naming/README.md).
