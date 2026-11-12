# Reverse snapshot notifier definitions

This family separates a complete notifier class definition from stronger installed-base or runtime guarantees.

[`operational-canonical.js`](operational-canonical.js) and [`operational-reordered-heldout.js`](operational-reordered-heldout.js) contain independent Node assertion scenarios for reverse snapshot traversal, carried results, sparse callbacks, errors, and mutation during notification. [`captured-notifier-parent.js`](captured-notifier-parent.js) preserves the full static parent; [`naming-outcome-ledger.json`](naming-outcome-ledger.json) records prior outcome identities.

Consumed by:

- [naming_reverse_snapshot_notifier.rs](../../../naming_reverse_snapshot_notifier.rs)

The claimed name is `ReverseSnapshotNotifier` with definition-only evidence. Client mutation does not promote that scope to callback purity, inherited runtime behavior, or installed-base proof. Tests replay exact edits and independently reparse declaration/reference identities; live ascending notification remains a separate control.

Keep the operational reference scenarios distinct from recovered source. The Rust consumer reads all three as text; do not run the captured parent. Preserve ledger hashes, class spans, and earlier outcomes unchanged.

The operational files are explicit Node assertion references; the captured parent remains static source input.
Next: [related fixture guide](../callback-notifier/README.md) and the [program fixture overview](../README.md).
