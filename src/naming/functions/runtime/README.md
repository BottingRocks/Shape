# Runtime and interception rules

The sibling [runtime.rs](../runtime.rs) declares these internal
[function evaluators](../README.md). They cover owned runtime operations,
interception, collection carriers and worker-side environment serialization.

| File or guide | Responsibility |
| --- | --- |
| [owned_runtime.rs](owned_runtime.rs) | Bounded runtime bodies and scoped host API paths |
| [accessor_hooks](accessor_hooks/README.md) | Descriptor interception and captured lookup/cache proofs |
| [function_hooks.rs](function_hooks.rs) | Function interception; includes [transport](function_hooks_transport.rs) |
| [collection_carrier_layout.rs](collection_carrier_layout.rs) | Collection carrier structure |
| [worker_environment.rs](worker_environment.rs) | Seeded environment bundle and its owned emitter |
| [whole_body_rules.rs](whole_body_rules.rs) | Additional complete runtime body protocols |

`owned_runtime` resolves only plain, single-origin aliases initialized before
use; browser host roots must be unshadowed globals. A captured spelling does
not establish the host path.

The worker environment proof joins header, keystream, string and numeric writers
to the posted payload. It accounts for ordered local phases where a binding
changes from a nonce or callable into a cursor. Reusing a spelling does not make
its earlier and later values interchangeable; each protocol use must consume
the proven origin for that phase.

Read [function transport tests](function_hooks_transport_tests.rs),
[whole body tests](../../../../tests/naming_whole_body_rules.rs) and
[accessor tests](../../../../tests/naming_accessor_protocols.rs). Routing of
worker-specific stages remains in [stages.rs](../../stages.rs).
