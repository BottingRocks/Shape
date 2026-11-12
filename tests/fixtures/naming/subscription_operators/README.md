# Subscription operator protocols

The baseline retains captured cleanup; the alternate retains separate cleanup. Both preserve scan, last-value, and callback adapter construction and invocation contexts. parent-outcomes.json records exact current parent cleanup outcomes.
The static JavaScript inputs include [subscription-parent-baseline.js](subscription-parent-baseline.js) and
[subscription-parent-alternate.js](subscription-parent-alternate.js).

[naming_subscription_operators.rs](../../../naming_subscription_operators.rs) is the integration consumer.
Tests accept original and alpha layouts, replay edits, and reject state/order, receiver, sentinel, capture-readiness, and private-self/external-binding changes. Outcome records preserve which declaration owns each role.

These are source-analysis fixtures, not runnable browser programs.
Keep complete lexical parents, declaration/reference identity, native spellings,
and statement order. Tests locate exact source bodies and, in some cases,
assert byte spans; even whitespace changes can invalidate the control.
Preserve each independent layout rather than deriving one from the other.
Read the linked assertions before editing, then continue with the
[naming fixtures overview](../README.md) and [naming implementation](../../../../src/naming/README.md).
