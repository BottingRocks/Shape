# Captured key removal and one callback

The JavaScript bodies here concatenate two captured key lists, delete those keys
from a captured target, then conditionally unregister one callback from an
onBeforeInvoke member.

[The integration target](../../naming_network_captured_key_callback_removal.rs)
wraps [hook-unregister-guarded.js](hook-unregister-guarded.js),
[hook-unregister-compact.js](hook-unregister-compact.js) and their
[alternate guarded](hook-unregister-alternate-guarded.js) and
[alternate compact](hook-unregister-alternate-compact.js) counterparts with
explicit lexical captures. All four establish the same bounded own-body role.

In-memory negatives change list relationships, loop order, delete receiver,
callback identity, guards or capture declarations. The role deliberately leaves
activation and external/native behavior unproved; it does not establish successful
network teardown.

Preserve each independent lowering and its exact mutation anchors. Do not execute
these source-only bodies or merge their capture identities. Compare the separate
[two-hook operation family](../operation-captured-key-list-unregister/README.md),
then read [network rules](../../../src/naming/functions/network/README.md).
