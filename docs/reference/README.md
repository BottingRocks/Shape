# Reference

These pages hold cross-module reference material: the source map, the
end-to-end execution walkthrough, the primitive operation taxonomy, and the
WASM boundary evidence. The [documentation index](../README.md) links the
workflow and contract guides.

Start with [source modules](source-modules.md) to locate code or
[from capture to executed handler](vm-flow.md) for the conceptual path.

## Pages

- [Source modules](source-modules.md) — cross-module ownership, flow, focused
  checks, and the glossary.
- [From capture to executed handler](vm-flow.md) — extraction, `HandlerIR`,
  templates, operands, dispatch, and tracing end to end.
- [VM operation reference](operation-reference.md) — primitive operation
  taxonomy and compositional vocabulary.
- [VM control-flow operations](vm-control-flow-ops.md) — source-shaped jump,
  call, branch, loop, and exception cases.
- [JS/WASM operand cases](js-wasm-operand-cases.md) — static paired
  operand-decoder evidence for the two tracked references.
- [Static WASM/JavaScript boundary](wasm-boundary.md) — module, import, table,
  and adapter inventory.
- [WebAssembly host exception boundary](wasm-exception-boundary.md) — source
  evidence for catch, rethrow, and host-flag behavior.
