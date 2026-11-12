# Route pattern canonicalization

The complete matcher contains URL/punycode handling and regex-based matching. The late-store file predeclares captures but initializes them before exposure.
The static JavaScript inputs include [url-pattern-matcher.js](url-pattern-matcher.js) and
[url-pattern-matcher-late-store.js](url-pattern-matcher-late-store.js).

[naming_route_pattern_canonicalizer.rs](../../../naming_route_pattern_canonicalizer.rs) is the integration consumer.
Both linked layouts are positive. url-pattern-matcher-store-after-exposure.js is a negative: capture setup occurs after invocation. Further controls change constants, regex flags, order, native globals, and deferred writers.

These are source-analysis fixtures, not runnable browser programs.
Keep complete lexical parents, declaration/reference identity, native spellings,
and statement order. Tests locate exact source bodies and, in some cases,
assert byte spans; even whitespace changes can invalidate the control.
Preserve each independent layout rather than deriving one from the other.
Read the linked assertions before editing, then continue with the
[naming fixtures overview](../README.md) and [naming implementation](../../../../src/naming/README.md).
