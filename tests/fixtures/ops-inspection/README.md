# Operation inspection sample

[handlers.raw.js](handlers.raw.js) is a small synthetic raw JavaScript kernel
shape with string data, bound built-ins, memory/context methods, dispatch data,
a byte image, and operation handlers. It provides a readable source input for
inspection rather than an immutable capture of the full VM.

The example in [operations/view](../../../src/operations/view/README.md)
uses it with `shape ops show ... --op 0 --format markdown`. Read
[the inspection contract](../../../docs/operations/ops-inspection.md) for the
catalogue/template/operand join and analyst-note authority boundaries.
No direct integration-test reference to this filename was found; implementation
unit checks are beside the operation view code.

A successful catalogue entry is not automatic proof that every joined analysis
or execution path is supported. Keep source/table ownership, handler order,
operand reads, and byte offsets intact. Treat this as static analysis input;
do not execute the JavaScript. Continue with [operation evidence](../operations/README.md)
and the [fixture overview](../README.md).
