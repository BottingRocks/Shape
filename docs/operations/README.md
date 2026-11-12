# Handler operations

This folder describes the recovered operation inventory: catalogues and
tables, abstract classification, handler graphs, admitted templates, bounded
execution, and operand separation. The [documentation index](../README.md)
covers the other areas.

Start with the [operation catalogue](operations.md), then use
[ops inspection](ops-inspection.md) to connect a handler to its evidence. For
implementation ownership, see the
[`operations` module guide](../../src/operations/README.md).

## Pages

- [Static operation catalogue](operations.md) — handler inventory and versioned
  JSON schema.
- [Operation-table presentation](operation-table.md) — source-linked identity,
  sparse keys, and handler entries.
- [Cross-capture operation classification](operation-classification.md) —
  abstract stack behavior and grouping across captures.
- [Ops inspection contract](ops-inspection.md) — the `ops show` join, separate
  statuses, and occurrence/decode projection.
- [Handler IR reference](handler-ir.md) — graph types, occurrences, effects,
  validation, and projection.
- [Handler templates](handler-templates.md) — recipe extraction and admission.
- [Bounded handler executor](handler-executor.md) — concrete execution of one
  admitted handler.
- [JavaScript inline operand separation](js-operand-separation.md) — inline
  operand reads and program-counter effects.
