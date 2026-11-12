# Canvas text sampling setup

These static JavaScript inputs exercise the complete queue-driven canvas sampling
setup, including pixel collection, encoded output, completion and cancellation.

[The integration target](../../naming_canvas_text_sampling_setup.rs) accepts
[closed-queue-setup-baseline.js](closed-queue-setup-baseline.js),
[the alternate layout](closed-queue-setup-alternate.js) and
[the event bootstrap layout](closed-queue-setup-event-bootstrap.js) with their
whole enclosing captures. [late-queue-capture.js](late-queue-capture.js) is a
refusal control: the required queue capture is unavailable at exposure.

[invented-capture-harness.js](invented-capture-harness.js) wraps a saved own body
with invented helpers. It supports alpha-renaming and semantic mutation tests;
it is not a source-backed witness for those helpers' origins. Controls change
canvas dimensions, queue budget, receiver identity, pixel lanes and effect order.

Keep source bytes, scoped capture stores and mutation anchors intact. Do not run
these recovered programs or substitute sibling helper evidence. Read next:
[observer batch and reverse sampling controls](../observer_batch_lifecycle/README.md)
and the [fixture collection](../).
