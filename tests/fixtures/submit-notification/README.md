# Submit notification and linked forwarders

[stateful-notification.js](stateful-notification.js) and
[direct-notification.js](direct-notification.js) are JavaScript parent harnesses
retaining form selection, submit listener removal, notification and callback
publication. They preserve the exact linked notifier and forward/reset bodies.

[The integration target](../../naming_submit_notification.rs) accepts originals
and renamed layouts under submit_notification_protocol. Changing a notifier's
defaultPrevented guard, form/event payload or published slot rejects its linked
family; changing one forwarder checks that body's own event/target/reset proof.
Direct-call parent controls retain their separate receiver-producing context.
[Module-local family tests](../../../src/naming/functions/events/submit_notification/family_tests.rs)
also consume both files alongside the larger parent captures.

Preserve field names, capture initialization, publication order and allowed null
resets. Edit replay and member-text checks establish that naming changes do not
rewrite protocol data. Captured receivers do not by themselves prove native
browser origin; do not execute these static recovered inputs.

Read next: [forwarding contexts](../submit-forwarding/README.md) and
[submit proof guide](../../../src/naming/functions/events/submit_notification/README.md).
