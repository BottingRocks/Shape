# Callback notifier class dependencies

JavaScript class and prototype-installation excerpts establish complete callback list protocols and the constructor origins used by dependent class recognizers.

[`native-reverse.js`](native-reverse.js) and [`native-forward.js`](native-forward.js) retain distinct snapshot/reverse and live/forward protocols. The ordinary base files cover native class and lowered prototype forms; [`provenance.json`](provenance.json) records excerpt identities.

Consumed by:

- [callback_notifier_tests.rs](../../../../src/naming/classes/callback_notifier_tests.rs)

[`counterexample-static-base-mutation.js`](counterexample-static-base-mutation.js) is a refusal control. Tests independently change receivers, callback arguments, sparse entries, catch behavior, descriptor installation, constructor aliases, and returned subclass effects.

Keep base installation, factory transport, and consumer context together. A matching class definition alone must not authorize the stronger installed-base proof.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../lazy-registry/README.md) and the [program fixture overview](../README.md).
