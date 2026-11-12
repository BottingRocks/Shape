# Verified signal layouts

Each profile groups independently recorded layouts by its baseline's opening
signal sequence. Each `slot-map.json` maps layout labels to slot-address/canonical-identity
objects; matching `schemas/<label>.json` files retain the exact seed and a selected
grammar subset. Names alone do not establish coverage or binder support.

| Profile | What its baseline contributes |
| --- | --- |
| [nested-frame-anchor-property-names-hash-incognito-storage-probe-layouts](nested-frame-anchor-property-names-hash-incognito-storage-probe-layouts/README.md) | 76 mapped slots; 9 retained grammar records |
| [canvas-readback-probe-builtin-probe-checksum-layouts](canvas-readback-probe-builtin-probe-checksum-layouts/README.md) | 76 mapped slots; 9 retained grammar records |
| [clean-console-stack-access-flags-proxy-tamper-flags-layouts](clean-console-stack-access-flags-proxy-tamper-flags-layouts/README.md) | 37 mapped slots; 6 retained grammar records |
| [console-stack-access-flags-match-media-layouts](console-stack-access-flags-match-media-layouts/README.md) | 82 mapped slots; 9 retained grammar records |
| [emoji-glyph-rgba-samples-match-media-layouts](emoji-glyph-rgba-samples-match-media-layouts/README.md) | 76 mapped slots; 9 retained grammar records |
| [emoji-channel-samples-feature-policy-layouts](emoji-channel-samples-feature-policy-layouts/README.md) | 82 mapped slots; 9 retained grammar records |
| [feature-policy-builtin-probe-checksum-layouts](feature-policy-builtin-probe-checksum-layouts/README.md) | 76 mapped slots; 9 retained grammar records |
| [match-media-lifecycle-phase-timings-layouts](match-media-lifecycle-phase-timings-layouts/README.md) | 77 mapped slots; 9 retained grammar records |
| [console-member-nativeness-hash-pair-host-environment-layouts](console-member-nativeness-hash-pair-host-environment-layouts/README.md) | 77 mapped slots; 9 retained grammar records |
| [emoji-channel-sample-halves-match-media-layouts](emoji-channel-sample-halves-match-media-layouts/README.md) | 83 mapped slots; 9 retained grammar records |
| [web-gl-render-hash-vm-source-integrity-layouts](web-gl-render-hash-vm-source-integrity-layouts/README.md) | 65 mapped slots; 7 retained grammar records |
| [web-gl-render-hash-navigator-plugins-layouts](web-gl-render-hash-navigator-plugins-layouts/README.md) | 64 mapped slots; 6 retained grammar records |

[Catalog tests](../../../../src/signals/analysis/catalog_tests.rs) validate canonical
identities against [catalog registry](../../../../src/signals/catalog/mod.rs), verify
claims and card fields, and compare [field-order evidence](../field-orders.json).
Baseline/alternate maps are independent controls; close-pair tests ensure that
related signals do not claim each other's slots. Missed or refused bindings
remain distinct from wrong claims, so passing these checks is not completeness.

Corpus checks are ignored by default. With the external corpus available:

```sh
cargo test --profile iter --lib catalog_tests -- --ignored --nocapture
```

`SHAPE_FROZEN_ROOT` selects the corpus root. `SHAPE_VENDORS` selects profile names,
`SHAPE_SEEDS=baseline` is the default; `all` or comma-separated layout labels
select alternatives. `SHAPE_SIGNAL` selects canonical signal names. Source is
located by the schema's exact seed across corpus subdirectories; missing input
can skip a check. No recovered program is executed.

The [emoji-channel sample-halves profile](emoji-channel-sample-halves-match-media-layouts/README.md) also
supplies proofs for the ignored [projection](../../../program_signal_projection.rs)
and [coverage](../../../program_signal_coverage.rs) targets. Preserve golden JSON,
source spans, canonical identities and decompressed evidence; never fill gaps by copying a
neighboring profile's evidence. Continue at the [signal fixtures guide](../README.md).
