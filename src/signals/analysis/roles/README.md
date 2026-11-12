# Structural role finders

The [analysis layer](../README.md) uses these finders to anchor signal recognition
on proven program behavior. The naming plan also consumes their claims through
[semantic_names.rs](../../../naming/semantic_names.rs). Finders return evidence;
they do not edit or rename the input program.

[mod.rs](mod.rs) owns `FinderCtx`, `FinderClaim`, vocabulary lookup and
`run_all`. Claims identify function spans, declaring identifiers for MAIN
bindings, or class spans in the supplied parse. `run_all` retains a claim only
when its role occurs in that package's vocabulary.

| Start here | Responsibility |
| --- | --- |
| [entry_runtime.rs](entry_runtime.rs) | Entry/runtime roles and late entry claims |
| [VM execution guide](vm_exec/README.md) | Reactive operators, queues and supporting classes |
| [collector_wiring.rs](collector_wiring.rs) | Collector coordination roles |
| [encoding_framing.rs](encoding_framing.rs), [serializer.rs](serializer.rs) | Wire helpers and generic serialization roles |
| [purposes.rs](purposes.rs) | Reusable purpose proofs consumed by other role finders |

Each package declares `VOCABULARY` alongside `find`. Keep role identifiers and
role kinds aligned with the claim target; function aliases and data bindings are
not interchangeable. Return every structurally proven candidate and let the
naming consumer resolve ambiguity rather than selecting by generated spelling
or source position. Stable host/API members can be evidence when backed by the
behavior being proved.

The parser, scoping and AST nodes in `FinderCtx` belong to the same source;
claims carry coordinates from that parse. Read [analysis/mod.rs](../mod.rs) for
how function claims become slot-local role evidence. Catalog-wide structural
restrictions are checked in
[catalog_tests.rs](../catalog_tests.rs). The [naming guide](../../../naming/README.md)
explains the downstream naming plan and inventory.
