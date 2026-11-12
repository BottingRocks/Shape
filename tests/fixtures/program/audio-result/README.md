# Seeded audio result emission

Full cleaned JavaScript programs retain channel readers, captured seed factories, numeric writers, and observer registration around the result callback.

[`audio-result-baseline.js`](audio-result-baseline.js), [`audio-result-alternate.js`](audio-result-alternate.js), and [`audio-result-heldout.js`](audio-result-heldout.js) are independent positive inputs for `emitSeededChannelResult`.

Consumed by:

- [naming_audio_result_emitter.rs](../../../naming_audio_result_emitter.rs)

The consumer also derives negatives for the channel index, prefix, reverse traversal, cursor threading, output tuple, completion receiver, helper return, and early invocation. Alpha renaming must preserve the claim without relying on generated binding spelling.

Keep whole lexical scopes and original source bytes intact: the tests slice exact callback and writer ranges before applying mutations.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../audio-reverse-prefix/README.md) and the [program fixture overview](../README.md).
