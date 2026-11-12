# Tracked VM kernel references

These two raw JavaScript files are immutable, source-only references for the
JavaScript VM and WebAssembly VM variants. Their bytes are tracked here so
static analysis and regression tests do not depend on an acquisition store.

The files are never executed as JavaScript or WebAssembly. The identity and
provenance record is in [`manifest.json`](manifest.json).

- `javascript/kernel.raw.js` — JavaScript VM reference.
- `webassembly/kernel.raw.js` — WebAssembly VM reference.
