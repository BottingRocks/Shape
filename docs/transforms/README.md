# Emitted JavaScript transforms

This folder explains how emitted JavaScript is simplified after tracing and
emission. The passes stay separate and fail closed when their proof does not
hold. The [documentation index](../README.md) covers the other areas.

Start with [emitted program cleanup](emitted-program-cleanup.md) for the
pipeline and [thread cleanup](thread-cleanup.md) for per-function rewriting. The
[`emitted_js` module guide](../../src/transforms/emitted_js/README.md) maps the
implementation.

## Pages

- [Individual thread cleanup](thread-cleanup.md) — per-emitted-function
  expansion, call restoration, construction cleanup, and loop recovery.
- [Emitted JavaScript cleanup](emitted-js-cleanup.md) — proven-dead pure
  declarations and regenerated provenance maps.
- [Emitted program cleanup](emitted-program-cleanup.md) — linked-program stage
  order, scalar bindings, carries, branches, and reporting.
- [Helper and tuple cleanup](emitted-helper-cleanup.md) — helper sharing,
  inlining, tuple handling, and generated names.
- [Emitted property-write cleanup](emitted-property-writes.md) — the
  fail-closed `Object.defineProperty` to assignment pass.
- [Remaining-loop audit](remaining-loop-audit.md) — finding and checking
  labeled infinite loops that structured-loop recovery did not rewrite.
