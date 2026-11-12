# Inline task queue means and temporal aliases

[mean-before-guard.js](mean-before-guard.js) and
[mean-committed-in-guard.js](mean-committed-in-guard.js) retain whole JavaScript
inputs for drainTaskQueue recognition. Their running-duration mean is computed
at different points around the queue budget guard.

[The integration target](../../naming_task_queue_inline.rs) parses each input,
builds the naming model and checks function_finder::task_queue_drain. Original
and renamed forms are positive. Mutations of worker arguments, queue receivers,
mean arithmetic, budget comparison and list escape are refusals.

[worker-forward-var-alias.UNEXECUTED.js](worker-forward-var-alias.UNEXECUTED.js)
and [budget-forward-var-alias.UNEXECUTED.js](budget-forward-var-alias.UNEXECUTED.js)
are static temporal-alias negative controls, actively read by Rust tests.
Ordered multi-hop aliases constructed from the baseline remain positive; a
forward alias before its value is ready must not borrow that evidence.

Preserve complete bootstrap/capture context, callback order and mutation anchors.
The UNEXECUTED suffix does not mean an ignored Rust test. Do not execute any of
these recovered JavaScript inputs or normalize their alias declarations.
Read next: [canvas queue consumer](../canvas_text_sampling_setup/README.md)
and [the fixture collection](../).
