# Submit forwarding parent contexts

Whole JavaScript parents and paired compact excerpts preserve submit target
selection, parent walks, notifier linkage and forward/reset protocols.
[stateful-submit.js](stateful-submit.js) with
[stateful-submit-compact.js](stateful-submit-compact.js), and
[forwarding-baseline.js](forwarding-baseline.js) with
[forwarding-baseline-compact.js](forwarding-baseline-compact.js), show both scales.
Alternate, captured-submit, checkbox-hook and initialized-range pairs retain
independent surrounding layouts.

[Module-local family tests](../../../src/naming/functions/events/submit_notification/family_tests.rs)
consume these controls to prove notifier, forwarder and trusted parent paths.
The stateful case includes mutually exclusive receiver stores that require a
parent proof rather than the older unchanged-capture guard alone.
[Checkbox unit tests](../../../src/naming/functions/events/remember_checkbox.rs)
additionally consume [checkbox-hook.js](checkbox-hook.js).

Keep compact and whole contexts separate, with exact publication, null-reset,
receiver and event relationships. Mutations belong in consuming tests; preserve
saved source bytes and span anchors. Do not execute these recovered programs.
Read next: [notification controls](../submit-notification/README.md) and
[submit proof guide](../../../src/naming/functions/events/submit_notification/README.md).
