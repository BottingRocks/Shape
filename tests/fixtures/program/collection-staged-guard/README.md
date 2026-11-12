# Staged collection request guards

JavaScript excerpts cover a request-admission guard lowered into ordered boolean assignments before header collection.

[`collector.js`](collector.js) isolates the null/request-policy decision and copy loop. [`factory.js`](factory.js) retains that collector inside collection API creation and initialization.

Consumed by:

- [naming_collection_staged_guard.rs](../../../naming_collection_staged_guard.rs)

Both are positive controls: the factory establishes child, initializer, and factory roles together. Negatives independently alter null checks, request fields, policy identity, branch ownership, effects, shadowing, and mutable captures. Breaking the child must block the lifecycle claims too.

Preserve staged assignment order and the exact guarded policy call. The test adds opaque lexical captures; these files are source input, not runnable API examples.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../collection-carrier-layout/README.md) and the [program fixture overview](../README.md).
