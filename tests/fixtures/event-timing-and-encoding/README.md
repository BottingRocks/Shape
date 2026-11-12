# Event timing and encoding

These static JavaScript bodies cover orientation moments, interval extrema,
touch samples, script digest dispatch and seeded wheel counter frames.

[The timing target](../../naming_event_timing_protocols.rs) reads
[orientation_moments.js](orientation_moments.js),
[interval_extrema.js](interval_extrema.js),
[script_digest_dispatch.js](script_digest_dispatch.js) and
[wheel_counter_frame.js](wheel_counter_frame.js). Wheel setup includes the actual
[elapsed timer](createElapsedTimer.js), [keystream factory](createKeystreamByteSource.js)
and [numeric writer](writeNumber.js); an invented writer is a refusal control.
[touch.js](touch.js) feeds [touch payload tests](../../naming_touch_payload.rs),
which check coordinates relative to bounds, record storage and error forwarding.

Positive originals and renamed layouts are paired with changes to timestamps,
thresholds, moment arithmetic, promise fallbacks, byte cursors and listener cleanup.
[Captured-signal tests](../../naming_captured_signal_protocols.rs) reuse the wheel
family to test integer nonce and writer-call ordering forms.

Preserve captured helper relationships, field/effect order and mutation anchors.
Do not execute recovered JavaScript or change its bytes to simplify examples.
Read next: [captured signal controls](../captured-signal-protocols/README.md)
and [the fixture collection](../).
