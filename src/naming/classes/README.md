# Whole-class naming proofs

These recognizers support the [naming pipeline](../README.md) by proving class
bodies and their surrounding identities. Start at [mod.rs](mod.rs) for the
compiled module list; a file in this directory is not necessarily a module
entry point of its own.

| File | Responsibility |
| --- | --- |
| [batch_scheduler.rs](batch_scheduler.rs) | Ordered scheduler contract and elapsed-clock witness |
| [callback_notifier.rs](callback_notifier.rs) | Callback notification grammars, constructor uses and base-class checks |
| [lazy_registry.rs](lazy_registry.rs) | Registry class proof; includes [transport checks](lazy_registry_transport.rs) |
| [ring_buffer.rs](ring_buffer.rs) | Ring-buffer class recognition |
| [subscriptions.rs](subscriptions.rs) | Subscription classes and descriptor forms |
| [encoder.rs](encoder.rs) | Returned value-encoder identity tied to serializer state and wire operations |

Most entry points here are crate-internal. The public
`identify_value_encoder_classes` helper parses source and returns identity
witnesses before generated-name filtering and rename hygiene. Parsing/model
errors are returned as strings; an absent proof yields no class witness.

Keep a complete class proof separate from a helper witness: the encoder borrows
an array-comparison witness, while the scheduler consumes an elapsed-clock
witness. Neither supplies the rest of the class contract. The notifier's closed
projection retains extra statements and unknown receivers rather than erasing
them to obtain a match.

Read [batch_scheduler_tests.rs](batch_scheduler_tests.rs),
[callback_notifier_tests.rs](callback_notifier_tests.rs) and
[subscriptions_descriptor_tests.rs](subscriptions_descriptor_tests.rs) for local
controls. [Encoder integration tests](../../../tests/naming_thread_class_encoder.rs)
exercise the identity boundary through naming.
