# Program naming fixtures

These JavaScript sources and JSON outcome/origin records exercise static structural
identification, scoped naming, capture readiness, and exact source rewriting.
Full cleaned parents retain real lexical dependencies; smaller excerpts rely on
bootstrap/capture wrappers supplied by their consuming Rust tests. They are not
a JavaScript application or a collection of scripts to run.

- Audio and frames: [audio-result](audio-result/README.md), [audio-reverse-prefix](audio-reverse-prefix/README.md), [channel-and-frame-protocols](channel-and-frame-protocols/README.md), [seeded-signal-frame](seeded-signal-frame/README.md).
- Collection APIs: [collection-carrier-layout](collection-carrier-layout/README.md), [collection-mapper-variants](collection-mapper-variants/README.md), [collection-staged-guard](collection-staged-guard/README.md).
- Classes and capture lifetimes: [callback-notifier](callback-notifier/README.md), [class-descriptors](class-descriptors/README.md), [lazy-registry](lazy-registry/README.md), [observable-mixed-factories](observable-mixed-factories/README.md), [observable-setup-order](observable-setup-order/README.md), [reverse-snapshot-notifier-definition](reverse-snapshot-notifier-definition/README.md), [ring-buffer](ring-buffer/README.md), [ring-buffer-constructor-epochs](ring-buffer-constructor-epochs/README.md).
- Storage and browser protocols: [encoder](encoder/README.md), [keyed-deletion](keyed-deletion/README.md), [migration-actual](migration-actual/README.md), [reflection-protocols](reflection-protocols/README.md), [storage-removal-layouts](storage-removal-layouts/README.md), [submit-cleanup](submit-cleanup/README.md), [url-query-reader](url-query-reader/README.md), [vector-split-null](vector-split-null/README.md).
- Wave origin and naming controls: [wave-multi-consumer](wave-multi-consumer/README.md), [wave-named-transport](wave-named-transport/README.md), [wave-position](wave-position/README.md), [wave-preserve](wave-preserve/README.md).

Files directly in this directory cover focused threads, shared hash dispatch and
interned writing, observable mapping, font metrics, checkbox state, persistent-ID
migration, subscription lifecycles, and batch schedulers. Start with
[focused thread tests](../../naming_focused_threads.rs),
[shared-thread tests](../../naming_thread_shared.rs),
[observable map tests](../../naming_observable_map_operator.rs),
[font sampler tests](../../naming_font_metric_sampler.rs), and
[batch scheduler unit tests](../../../src/naming/classes/batch_scheduler_tests.rs).

The child guides identify exact consumers and distinguish positive, negative,
baseline, alternate, and heldout controls. A familiar method name or a matching
fragment does not establish a whole-parent protocol, initialized capture, live
registry, or installed class dependency. Definition-only claims deliberately have
a narrower scope than operational dependency proofs.

Keep all fixture source bytes and golden JSON immutable during documentation work.
Tests use exact UTF-8 slices, declaration spans, function/class identities,
source hashes, mutation anchors, and edit replay. Do not reformat programs,
normalize captures, regenerate reports, or execute recovered JavaScript/WASM.
The operational notifier reference scenarios are distinct from captured source.

For the rewrite contract, read [program rewrites](../../../src/naming/program/README.md).
For broader context, continue to the [fixture directory](../) and [test directory](../../).
