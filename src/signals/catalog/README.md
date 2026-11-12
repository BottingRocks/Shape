# Canonical signal catalog

Each signal file owns its recognizer, binder, private helpers and approved
`CARD`. The [parent guide](../README.md) describes cards; the
[analysis guide](../analysis/README.md) describes how competing recognizers and
binding refusals reach the schema report.

| File | Responsibility |
| --- | --- |
| [mod.rs](mod.rs) | Private signal modules, internal `SIGNALS` registry, `cards()` and `card(name)` access |
| [card.rs](card.rs) | Card, message, field, group, repetition and flag-name types |
| [device_memory.rs](device_memory.rs) | Small example of recognition, positional binding and card data together |

A recognizer receives the structural `SlotView` and analysis context. A binder
receives the projected layout too and returns `FieldBinding` or `BindRefusal`.
For example, the memory recognizer restricts API evidence to numeric leaves to
avoid reused-local evidence on a string leaf, then binds the approved field name.
Preserve that distinction between identifying a slot and naming its fields.

Use card field names in bindings and card flag lists for bit names. Card order
does not establish bit positions: the [flags layer](../flags/README.md) derives
those from writes. Helpers needed by multiple signals belong in analysis;
helpers unique to one signal stay beside its recognizer.

When changing a card, inspect [docs.rs](../docs.rs), which renders the
[signal reference](../../../docs/signals/README.md), and its `pages_match_cards`
test. [catalog_tests.rs](../analysis/catalog_tests.rs) covers registry consistency
and restrictions on structural recognition sources. Avoid expanding a match by
using salts, descriptor positions or generated identifier spellings.

Each `CARD` also owns one `sample_payload: Option<&'static str>` beside its
summary and why: a captured decoded JSON value copied exactly from an observed
payload. Leave it `None` until captured, or when the card emits no payload.
These documentary samples never determine runtime schema or codec rules.
`None` is metadata absence, not an emitted JSON null or empty object.
`shape show signals [SignalName] [--json]` displays these cards without
reading a source file; `shape signals docs docs/signals` renders the same sample
into the generated reference pages.
