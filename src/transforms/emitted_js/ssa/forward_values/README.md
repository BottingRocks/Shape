# Forwarded values

The late forwarding engine: removes a generated `let|const $SSA_n = init;`
(or an internal `$KEY_` slot) by sending `init` to its reads, in one parse per
round. Per-function stages only: `forward_values` at `11h0-resugared-helpers.js`
(`helper_expansion`), `11i-forwarded-values.js` and
`11k-forwarded-branch-values.js`; `forward_values_once` inside the
`11k3-forwarded-again.js` loop; `forward_values_and_property_reads` (rule 12
on) at `11m`, `11o` and `11p1` (`forwarding`). `--thread-cleanup` does not
use it.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | The rules (module docs), entry points, the round loop (up to 64 more rounds while anything moves), `Candidate`/`Forward`/`Plan` types, and the property-read and isolated-slot classifiers |
| [`scan.rs`](scan.rs) | One walk per owner: candidates and their `InitKind`, slot stores, effects, code-running nodes, property stores, branch tests, decoys, tuple aliases and pass-through slots |
| [`decide.rs`](decide.rs) | Which candidates move and how: `consider` tries the rules for one candidate (rules 1, 2, 10, 11, 12 here; 5–7 via `stability.rs`/`slots.rs`), in source order, repeated with memoized lookups until settled; `broken_assumptions` lists moves to block |
| [`stability.rs`](stability.rs) | Whether a value still holds at a read: stable symbols, closures settled, internal keys, snapshot and property-unchanged checks, rule 6 handoff |
| [`slots.rs`](slots.rs) | Slot snapshots and stores: rule 7 early slots, `slot_unchanged`, skipped stores, loop bodies, `try`/finalizer and `await`/`yield` checks |
| [`edits.rs`](edits.rs) | Turns decisions into edits: composed text, placement and parentheses, rule 8 store-to-load and rule 9 branch booleans; returns the symbols whose names would resolve differently |
| [`holders.rs`](holders.rs) | Fresh-tuple helpers and the bindings holding their results; `FixedArguments` for functions whose `arguments[n]` cannot change |
| [`tests.rs`](tests.rs) | One test per rule and its declines |

## Rules

1. **Stable value** — literal, unshadowed `undefined`/`NaN`/`Infinity`,
   never-written non-slot local, or an in-range slot of a read-only fresh
   tuple: replaces every read.
2. **Next door** — any `init` read once, in the next statement that does
   anything, evaluated exactly once there after a stable prefix.
3. **Duplicate slots** — `let [a, b] = H(…)` with `H` returning one identifier
   everywhere: `b` becomes a hole, its reads read `a`.
4. **Pass-through slots** — a slot `H` fills with an unwritten parameter reads
   the stable argument; a pure shuffle call goes when every slot does.
5. **Slot snapshots** — `let $SSA_n = $KEY_k;` reads the slot directly while
   nothing writes it up to the last read and no read is in another loop.
6. **Slot handoff** — a multi-read value whose first read is the next
   statement's slot store moves there; later reads read the slot.
7. **Early slot** — a value a later sibling stores into a `var` slot (or a
   `let` carry declared earlier) is stored where it is computed.
8. **Store to load** — after `$KEY_k = v;` with stable `v`, a later read of
   the slot in the same list reads `v`; the store stays for dead-key cleanup.
9. **Branch-known booleans** — inside the arms of `if (r)` / `r ? a : b`, a
   never-written `r` reads as `true` or `false`.
10. **Fresh allocations** — never copied; read once, moves to that read
    unless the read is in a loop the declaration is not in.
11. **Computed snapshot** — a pure computation whose names keep their values
    up to its last read (code only; not in the module docs).
12. **Property snapshot** — a property read off a name with keys of known
    primitive type, where nothing that can store a property runs up to its
    last read (code only; enabled by `forward_values_and_property_reads`).

## Contracts

- Assumptions: no getters on the property reads rule 12 copies; `.call`/
  `.apply` off a plain name and named properties of standard built-ins read
  fixed values; operators over typed primitives (`support/primitives.rs`) run
  no code; a parameterless function reading `arguments` only at literal
  indexes (`dead_ssa::readonly_arguments`) has fixed `arguments[n]`.
- A binding only its own function writes is settled inside closures only when
  the file has no `await`/`yield` and a `let` is that function's first statement.
- A stable value never carries a moved (rule 2) value inside it; a snapshot
  read never rides inside a moved value.
- Decisions may assume a later declaration goes; `edits` checks them against
  the final decisions and blocks the ones that fail, then decides again.
- Composed text is parenthesized; a member callee becomes `(0, value)`.
  Comments in the declaration, `with` and `eval` decline. Overlapping edits
  are an error; output is reparsed.
- Rule 12 runs after resugaring (`11i2`, the `11k3` loop), whose folds need
  the plain names, and before `merge_derived`, which would change objects
  under copied reads.

[Emitted JavaScript cleanup](../../README.md) ·
[Pipeline table](../../../../../docs/transforms/emitted-program-cleanup.md#pipeline) ·
[SSA folder](../README.md)
