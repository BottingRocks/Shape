# Memory attribution formatting

Whole scopes retain attribution mapping from bytes, types, scope, and optional container fields into compact output records, including bounded URL formatting. They also retain registry method and constructor evidence.
The static JavaScript inputs include [memory-breakdown-baseline.js](memory-breakdown-baseline.js) and
[memory-breakdown-alternate.js](memory-breakdown-alternate.js).

[naming_memory_attribution_format.rs](../../../naming_memory_attribution_format.rs) is the integration consumer.
Both layouts establish formatter positives. Controls alter slice bounds, regex flags, field sources, container assignment, callback effects, or constructor guards; malformed methods cannot supply constructor evidence.

These are source-analysis fixtures, not runnable browser programs.
Keep complete lexical parents, declaration/reference identity, native spellings,
and statement order. Tests locate exact source bodies and, in some cases,
assert byte spans; even whitespace changes can invalidate the control.
Preserve each independent layout rather than deriving one from the other.
Read the linked assertions before editing, then continue with the
[naming fixtures overview](../README.md) and [naming implementation](../../../../src/naming/README.md).
