# Capability-grouped function rules

This is the internal evaluator layer of [function naming](../README.md).
[mod.rs](mod.rs) declares the capability modules; [stages.rs](../stages.rs)
chooses when they run. Folder order does not determine precedence.

| Guide | Scope |
| --- | --- |
| [Browser](browser/README.md) | Persistence, query handling, frames and submit cleanup |
| [Encoding](encoding/README.md) | Hashing, code units, entropy and numeric writers |
| [Events](events/README.md) | Event records, visibility and interaction state |
| [Graphics](graphics/README.md) | Canvas, vectors and animation-frame sampling |
| [Network](network/README.md) | Request metadata, route patterns and ICE handling |
| [Observables](observables/README.md) | Stream factories, operators and serialization wiring |
| [Runtime](runtime/README.md) | Interception, collection carriers and worker environments |
| [Scheduling](scheduling/README.md) | Timers, queues and collection protocols |

The sibling capability files, such as [browser.rs](browser.rs), declare each
folder's leaves. Evaluators inspect a scoped `Model` and `FunctionSite`, then
return proposals; their visibility does not make them external APIs.
[shared.rs](../shared.rs) owns shared consequence proofs and reconciliation,
[resolve.rs](../resolve.rs) callable resolution, and
[support](../support/README.md) the narrower AST helpers.

Stage groups collect their claims before falling through when empty. Preserve
the explicit order and ambiguity handling in `stages.rs`; adding a rule to a
module declaration alone does not route it. Proving a captured data binding is
also distinct from proving the callable or native identity of its value.

See [thread naming](../../../docs/naming/threads.md) for report behavior and
[program rewriting](../program/README.md) for applying names to source.
