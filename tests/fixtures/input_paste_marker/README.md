# Input paste marker captures

[captured-slot-factory.js](captured-slot-factory.js) and
[ordered-slot-factory.js](ordered-slot-factory.js) retain JavaScript factory
contexts for counting paste input and marking a target inactive. Capture records
are kept in [provenance.json](provenance.json), including the fixture identity
and full-parent location.

[The integration target](../../naming_input_paste_marker.rs) exercises the
countInputPasteAndMarkInactiveTarget role with direct and hoisted capture
proofs. The ordered source checks straight-line initialization of mutable count
and inactive slots; conditional or missing setup is a refusal control.

Mutations vary input type, target/active-element comparison, error receiver,
publication and capture writes. Older opaque-receiver rules can independently
apply to some negatives, so refusal is checked at the intended evaluator boundary.
No native DOM or event installation identity follows solely from this role.

Preserve source bytes, captured relationships, hash records and mutation anchors.
Analyze these files statically; do not execute their recovered JavaScript.
Read next: [closed emission parents](../closed-emission-protocols/README.md)
and [the fixture collection](../).
