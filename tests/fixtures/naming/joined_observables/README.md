# Joined observable ownership

The excerpts retain joined subscription state, cleanup, and neighboring native/proxy/codec-seed helpers. The two joined-state-parent JavaScript files retain fuller ownership contexts for teardown.
The static JavaScript inputs include [joined-state-excerpt-baseline.js](joined-state-excerpt-baseline.js) and
[joined-state-excerpt-alternate.js](joined-state-excerpt-alternate.js).

[naming_joined_observables.rs](../../../naming_joined_observables.rs) is the integration consumer.
Baseline and alternate scopes plus alpha variants establish role recovery. Mutations reject altered subscription protocols, unsafe capture writers, preinitialization exposure, and named-expression ownership confusion.

These are source-analysis fixtures, not runnable browser programs.
Keep complete lexical parents, declaration/reference identity, native spellings,
and statement order. Tests locate exact source bodies and, in some cases,
assert byte spans; even whitespace changes can invalidate the control.
Preserve each independent layout rather than deriving one from the other.
Read the linked assertions before editing, then continue with the
[naming fixtures overview](../README.md) and [naming implementation](../../../../src/naming/README.md).
