# Execution framework roles

The sibling [vm_exec.rs](../vm_exec.rs) declares the two internal modules in this
folder. They supply execution-framework claims to the [role registry](../README.md),
not an interpreter or a public naming API.

| File | Proof family |
| --- | --- |
| [operators.rs](operators.rs) | Reactive operator factories, executors, observers, teardowns and queue internals |
| [classes.rs](classes.rs) | Elapsed clocks, queue helpers, subscription/observable classes, batch scheduling and callback notification |

Operator recognition anchors on a MAIN binding holding the result of a
zero-argument factory that returns a class with `subscribe` and `pipe`.
The factory/operator/executor relationship and each observer's use of its slots
distinguish roles. Follow that chain before changing an individual callback
rule: a method name alone is not its proof.

Class recognition collects methods from class syntax or constructor/prototype
writes and checks their behavior together. Queue helpers consume the purpose
proofs in [purposes.rs](../purposes.rs). Both modules use
[ParameterSlots](../../../extract/bindings.rs) to relate named formals and
`arguments[N]`; callback resolution also handles supported hoisted functions.
Keep these structural adapters separate from the role-specific tests.

Each module owns its `VOCABULARY` and `find`. For consumer-level
coverage, read [naming_subscription_operators.rs](../../../../../tests/naming_subscription_operators.rs)
and [naming_task_queue_roles.rs](../../../../../tests/naming_task_queue_roles.rs).
