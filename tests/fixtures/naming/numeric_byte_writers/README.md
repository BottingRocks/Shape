# Numeric byte writer protocols

Whole scopes retain tagged unsigned integer writers, typed-array float writers, extension registry factories, and array-equality wrappers with their captured callees.
The static JavaScript inputs include [numeric-writers-whole-scope.js](numeric-writers-whole-scope.js) and
[numeric-writers-whole-scope-alternate.js](numeric-writers-whole-scope-alternate.js).

[naming_numeric_byte_writers.rs](../../../naming_numeric_byte_writers.rs) is the integration consumer.
Both scopes and alpha variants are positive controls. Mutations test numeric protocol details, executed initialization stores, factory ownership, and equality callee readiness; a refused equality role must not be confused with another structural forwarder role.

These are source-analysis fixtures, not runnable browser programs.
Keep complete lexical parents, declaration/reference identity, native spellings,
and statement order. Tests locate exact source bodies and, in some cases,
assert byte spans; even whitespace changes can invalidate the control.
Preserve each independent layout rather than deriving one from the other.
Read the linked assertions before editing, then continue with the
[naming fixtures overview](../README.md) and [naming implementation](../../../../src/naming/README.md).
