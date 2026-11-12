# Whole-body naming controls

[basic.js](basic.js) is a compact enclosing JavaScript scope containing captured
callee wrappers, callback error forwarding, subscription state reset, a specific
TypeError throw, native-function source checking, handle completion, visibility
sampling, extension registration, and a batch-clock constructor.

[naming_whole_body_rules.rs](../../../naming_whole_body_rules.rs) consumes it
through static naming, verifies role evidence, and replays returned source edits.
Original and alpha layouts are positive controls. Near misses change literals,
guards, wrapper effects, or receiver/capture relationships and must lose the
specific role. A nearby recognizable fragment is insufficient proof for a body.

Keep this fixture small enough that exact unique replacements remain meaningful.
Preserve native spellings, lexical ownership, error text, and effect order;
add distinct controls in tests rather than weakening an existing mutation.
This is parser input, not a script to execute. Continue with the
[naming overview](../README.md) and linked target assertions.
