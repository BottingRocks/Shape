# Closed emission protocols

Two whole JavaScript parents retain the helper implementations and capture stores
needed for bounded emission and callback-list proofs.

[tuple-reverse-and-storage.js](tuple-reverse-and-storage.js) supplies seeded
five-field tuples, reverse-array encoding, channel results and storage-removal
signals. [paste-callback-and-network.js](paste-callback-and-network.js) supplies
paste markers, reverse callback folding, callback forwarding/nulling/replacement,
distinct candidate-address collection and another storage signal.

[The integration target](../../naming_closed_emission_protocols.rs) checks exact
applied roles and uses in-memory mutations of field order, array indexing,
callback arguments and writer identity as refusals. The first parent also feeds
[canvas scene frame tests](../../naming_canvas_scene_anchor_frame.rs).

Keep complete enclosing scopes: a matching sibling cannot donate writer origin.
Preserve source bytes, declaration locations and mutation anchors. These are
static recovered inputs; do not execute or reformat them.

Read next: [paste marker controls](../input_paste_marker/README.md),
[canvas setup controls](../canvas_text_sampling_setup/README.md) and
[the fixture collection](../).
