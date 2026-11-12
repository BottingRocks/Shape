# Captured signal protocols

Small JavaScript function bodies isolate callability/source traits, vector history
and digest dispatch while retaining their captured receiver spellings.

[The integration target](../../naming_captured_signal_protocols.rs) wraps
[function-traits.js](function-traits.js) with explicit source-text and property
helpers, [vector-history.js](vector-history.js) with a lexical owner, and
[script-digest.js](script-digest.js) with dispatch captures. Accepted originals
and alpha variants are paired with mutations of prototype deletion, vector
comparison/copying, promise fallbacks and call receivers.

[wheel-integer.js](wheel-integer.js) and
[wheel-cursor-alias.js](wheel-cursor-alias.js) retain additional wheel body forms.
The target's executable Rust test constructs its wheel control from the
[event timing fixtures](../event-timing-and-encoding/README.md), preserving that
family's proved factory and numeric writer; these saved bodies alone do not
prove capture origins.

Treat all JavaScript as static analysis input. Preserve whole-body order, capture
identities and mutation anchors; do not execute or normalize it. Continue with
[observable rules](../../../src/naming/functions/observables/README.md)
and the [fixture collection](../).
