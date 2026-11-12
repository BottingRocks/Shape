# Split-null pixel vector history

A JavaScript source fixture preserves an owned vector comparison loop with a null decision split across assignments.

[`owned.js`](owned.js) covers `recordPixelVectorSample`: vector/history comparison, sample copying, count updates, and bounded history handling remain in the owning callback.

Consumed by:

- [naming_vector_split_null.rs](../../../naming_vector_split_null.rs)

The positive, alpha-renamed, and `var` variants must replay edits exactly. Negatives independently change null branches, equality, loop direction, history fields, slice receiver, extra effects, shadowing, clobbers, and nested donor assignments.

Keep comparison, history update, and split-null assignments together. Preserve source bytes and mutation anchors; equivalent-looking null operators or shifted bindings are deliberate refusal cases.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../README.md) and the [program fixture overview](../README.md).
