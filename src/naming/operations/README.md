# Bounded operation naming

The sibling [operations.rs](../operations.rs) declares operation-level rules
for the [naming pipeline](../README.md). These claims describe own syntax and
effects over scoped, often opaque values rather than a whole native class.

| File | Responsibility |
| --- | --- |
| [callback_list_dispatch.rs](callback_list_dispatch.rs) | Whole callback-list argument, iteration, guard and receiver-write protocols |
| [captured_key_list_unregister.rs](captured_key_list_unregister.rs) | Captured-key list removal syntax with opaque capture ports |
| [throw_type_error.rs](throw_type_error.rs) | Zero-formal throw of a new unshadowed TypeError with a literal message |

Callback-list operations do not assume an opaque call is `slice`, `push`,
`indexOf` or `concat`. The unregister proof likewise does not promote capture
positions, getters or external writes into stable external identities.

The throw rule requires exactly one throw statement and one string-literal
argument to the unshadowed `TypeError` constructor. It retains the source
message and claims no surrounding failure cause or accessor purpose. Extra
statements, computed callees and effectful arguments do not satisfy that body.

Read the inline tests in the dispatch and throw files,
[captured-key tests](captured_key_list_unregister_tests.rs) and
[operation integration tests](../../../tests/naming_operation_captured_key_list_unregister.rs).
Routing still belongs to [stages.rs](../stages.rs).
