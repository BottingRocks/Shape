# Adapter protocols

Whole scopes retain href property accessors, JSON parsing, and event/target forwarding beside their captured helpers.
The static JavaScript inputs include [adapter-whole-scope.js](adapter-whole-scope.js) and
[adapter-whole-scope-alternate.js](adapter-whole-scope-alternate.js).

[naming_adapter_protocols.rs](../../../naming_adapter_protocols.rs) is the integration consumer.
The tests accept alpha-renamed layouts and reject changed property ownership, throw/return behavior, receiver identity, argument order, and executed-store epochs.

These are source-analysis fixtures, not runnable browser programs.
Keep complete lexical parents, declaration/reference identity, native spellings,
and statement order. Tests locate exact source bodies and, in some cases,
assert byte spans; even whitespace changes can invalidate the control.
Preserve each independent layout rather than deriving one from the other.
Read the linked assertions before editing, then continue with the
[naming fixtures overview](../README.md) and [naming implementation](../../../../src/naming/README.md).
