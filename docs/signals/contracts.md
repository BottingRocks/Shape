# Signal contracts

`shape signals contracts` turns one recovered `program.js` into
generator-consumable signal contracts. For every descriptor in the capture's
registry, the report gives the producer that serves it, the ordered typed
grammar of each `observer.next` emission, the `error`/`complete` sites, the
provenance of every value written, and an accounting of every write to an
emitted buffer. Each descriptor gets a `complete`, `partial` or `unsupported`
status, and every non-complete status carries concrete reasons.

```sh
shape signals contracts program.js                      # text summary
shape signals contracts program.js --json > contracts.json
shape signals contracts program.js --signal 7           # one descriptor
shape signals contracts research/seed-1/program.js \
    --pair target/seed-1/program.js                      # second trace of the same raw build
```

The Rust API is `shape::signals::extract::contracts::{extract_signal_contracts,
signal_contracts_from_inputs, pair_trace_folded_literals, render_summary}`.
The schema version is `SIGNAL_CONTRACT_SCHEMA_VERSION` (1).

No command executes the recovered JavaScript. Every span is a UTF-8 **byte**
offset into the input file exactly as given. Renamed output is a reading and
analysis artifact, and the contract makes no claim of runtime equivalence.

## What is derived, and from where

| part | source | notes |
| --- | --- | --- |
| producer inventory | wiring pass (`wiring.rs`): descriptor registry, group rows, routing flags, direct constructor routes | counts come from this capture; `unrouted` and `multiply_routed` must both be empty |
| producer arguments and phases | the group or direct invocation and the body's registrar calls | `phase` is the innermost registered callback containing the site |
| grammar | ordered replay in `signals.rs` (`CollectorScanner`) | JavaScript evaluation order, path-sensitive per emission |
| lifecycle | wiring local emitters (`error`, `complete`) | kept apart from the emitted body |
| provenance | backward walk in `contracts.rs` (`ProvenanceWalker`) over definitions, reaching definitions (`reaching.rs`), context fields and callbacks | bounded; see limits |
| helpers | encoding pass (NUMBER writer, STRING writer factory, stream factory, hash) | identified by structure, never by name |

Production code never recognizes anything by identifier spelling, `$KEY_…`
names, salts, markers, descriptor or row numbers, offsets, capture hashes or
atlas IDs. The descriptor index is reported because the capture's own registry
defines it. It is a capture-local join key.

## Grammar

`GrammarNode` is tagged by `op`:

| op | meaning |
| --- | --- |
| `raw_prefix` | the 4 raw big-endian bytes of a per-message random word. `spelling` is `array_literal`, `index_stores` (four stores, or `[w >>> 24]` plus three stores) or `inherited` (set outside the replayed path); the rendering is always `RAW4` |
| `number`, `string` | a writer call. `value` and `value_span` hold the logical value; `mask` is `stream` with the stream argument in `mask_source`; `position` is the writer position argument |
| `byte` | one logical byte. For `v ^ m()`, `m() ^ v`, `v ^ m.call(void 0)` or `v ^ factory(seed ^ C)()`, the value is `v` and `mask` is `stream`; `v ^ 7` is `static_xor`; a plain store is `unmasked` (rendered `BYTE!`) |
| `sequence` | evaluation order |
| `choice` | `kind` is `if`, `conditional`, `logical`, `try_catch` or `switch`, with `condition` and labelled `arms`. An empty arm means the writes may not run. A `finally` is appended to both arms |
| `repeat` | `loop_kind`, `direction` (`ascending`, `descending`, `iteration` for `for…in`/`for…of`, or `unknown`), `header` and `count` (only when provable) |
| `opaque` | the emitted buffer is passed to a call that is neither a writer nor a verified store helper; the call may write it. The status is then `partial` |
| `unresolved` | a write path that could not be typed, a depth limit, or a bounded control/data-flow boundary (`try`/`catch`, `switch`, a buffer rebinding). It is never dropped |

Write order is post-order: `N(s, X, B, N(s, Y, B, p))` writes Y before X, and
`B[S(str, B, p) + 1] = v ^ m()` writes the STRING before the BYTE.

The following representation differences between trace forms are normalized:
- named formals versus `arguments[N]`
- `.p` versus `["p"]`
- hoisted named callbacks
- `observer.next(p)`, `observer.next.call(o, p)`, `observer.next.apply(o, [p])` and the extracted `m = observer.next; m.call(o, p)`
- callbacks passed to constructors (`new MutationObserver(() => …)`)
- a writer held in a reused local (`let w = writer; w = w(…)`)
- tuple stores (`t = [B, k]; t[0][t[1]] = v`)
- store helpers (`T[1] = store(T, 0, meta, B)`)
- the three prefix spellings
- the two endless-loop spellings `for (;;) { if (!t) { exit } body }` and `for (;;) { if (t) { body } else { exit } }`. Both become `REPEAT(test t){body}`, then the exit path.

Real per-build differences are kept: field order, bit order inside a BYTE,
marker constants, loop direction and branch structure.

An emission inside a loop body sees the whole body as a `REPEAT` before the
writes of its own iteration, because earlier iterations wrote too.

## Provenance

Each write carries `provenance` with these fields:
- `classes`: the sorted set of the classes below.
- `transforms`: the operators and calls applied, such as `^`, `|`, `.length`, `.sort()`, `hash`, `count`, `guard` and `callback`.
- `sources`: each with a class, span, text, API path and the definition hops followed.
- `elided` and `truncated`.

| class | meaning |
| --- | --- |
| `literal` | a literal in this capture. `literal_origin` is `unproven` unless pairing says otherwise; a literal alone is never treated as generator configuration |
| `source_table` | array or object literal tables and shapes (probe lists, glyph rows, option bags) |
| `message_randomness` | `Math.random()` / `crypto.getRandomValues` evaluated inside the producer |
| `session_randomness` | randomness drawn outside the producer and reused |
| `shared_state` | bindings owned outside the producer, producer arguments (labelled with their wiring role) and context fields with too many stores |
| `host_observation` | unbound globals and member paths on them, host APIs reached through the producer context object (`ctx.global` resolves to the program's own `global`/`self`/`window` probe), and constructors on host roots |
| `host_clock` | `Date.now()`, `performance.now()`, `new Date()`, including through local aliases |
| `loop_iteration` | loop counters (also update-only counters), iterated elements, and parameters of callbacks handed to iteration methods or to helpers |
| `caught_error` | catch parameters |
| `program_call` | the result of a program function not modelled further |
| `opaque` | a value the walk cannot follow |

An accumulating write (`++n`, `x += v`) also contributes the conditions that
guard it and the data its enclosing callback is handed. A value converted to
text and then hashed or reduced to a code unit stays a numeric computation,
with the conversion recorded in `transforms`.

## Trace-folded observations

The older research tracer folded some reads of the VM kernel's own globals
into literals (atlas A08). A single capture cannot prove this, so a literal's
origin is `unproven`. When a second trace of the same raw build is supplied
with `--pair`, alignment is attempted only if:
- the two producer inventories are equal, and
- for each descriptor, the emissions carry the same ordered write kinds.

For each aligned field, if the paired field reads a **non-platform** global
that this field does not, its literals are marked as follows:
- `trace_folded_observation` when the field is literal-only;
- `possibly_trace_folded` otherwise.

Such a global is an unbound identifier outside the ECMAScript/Web vocabulary;
in this corpus it is `vmEntry`. The paired field's classes and APIs, plus
`missing_host_reads`, are attached as evidence. The literal is never replaced,
and its exact derivation is not asserted.

Any other class differences between aligned fields are listed under
`pairing.provenance_mismatches`. They show where one trace representation
defeats the bounded walk. They are never fold claims.

## Status and accounting

A contract is `complete` only when every emission has a typed grammar with no
`opaque` or `unresolved` node, every stream mask is resolved, and every write
to its buffers is accounted for. `partial` lists reasons such as:
- `buffer_passed_to_unmodelled_call`
- `generic_encoder_rule_unapplied` (a named object sent through the generic token encoder)
- `mask_stream_unresolved`
- `write_in_other_function_scope`
- `emitter_next_site_not_reconstructed`
- `grammar_depth_limit`
- `buffer_alias_flow_unresolved` (a binding is reassigned from another known buffer)
- `try_catch_control_flow_unresolved` (both `try` and `catch` can write the same emitted buffer)
- `switch_fallthrough_control_flow_unresolved` (a case body can write the emitted buffer)

`unsupported` means no emission grammar was reconstructed.

`accounting` lists, per emitted buffer, every write the replay saw (writes,
prefix stores and opaque hand-offs, in all scopes and branches). Each one is
either in some emission's grammar or listed as excluded, with
`not_on_emission_path` or `other_function_scope`.

### Conservative control and alias boundaries

The bounded rules below are reported instead of being silently treated as
exclusive paths. They never drop an emission or a write: the known grammar and
accounting stay in the report, and the contract becomes `partial` with a
concrete reason. A boundary is scoped to the buffer it can actually affect; an
unrelated `try` or `switch` does not make a contract partial.

- **Stable buffer alias.** A plain `holder = buffer` is coalesced to the buffer
  binding when `buffer` is already known to be an emitted buffer, so a write
  through `holder` is accounted to `buffer`. This is deliberately narrow, not a
  general alias analysis: `known_buffers` is filled from recognized raw-prefix
  arrays and from the buffer argument of a writer call, so an alias of a buffer
  whose origin is not recognized is not tracked. Nothing here claims that every
  mutable alias flow is safe.
- **Buffer rebinding.** An assignment between two bindings already known to be
  emitted buffers (including inside a branch) puts
  `buffer_alias_flow_unresolved` on the target buffer's following grammars. It
  is path-insensitive on purpose: it says only that a later writer may mutate a
  different backing array than the symbol name suggests. A stable alias is not
  marked, and an assignment from a non-buffer is not marked.
- **`try`/`catch`.** When both the `try` body and the `catch` body can write the
  same emitted buffer, a throw after a write can leave that write applied while
  the catch body also runs. The existing `try_catch` choice then only
  approximates the reachable writes, so that buffer gets
  `try_catch_control_flow_unresolved` on following emissions. A write in only
  one arm is already represented by the choice and is not marked. Full
  exception completion/throw paths are not modelled.
- **`switch`.** When a case body can write an emitted buffer, fall-through into
  a later case and the unmatched empty route of a `switch` without `default`
  are not modelled, so that buffer gets
  `switch_fallthrough_control_flow_unresolved`. The rule is conservative: it
  marks even a single writing case. No `switch` statement occurs in the six
  pinned captures, so this boundary has no corpus effect today.

On the six pinned captures no `try` and `catch` pair writes the same buffer and
no known-buffer rebinding occurs, so these boundaries add no node there; they
are exercised by the synthetic Rust regressions only. `complete` remains a
grammar and symbol-accounting status, not a proof of runtime behaviour,
provenance completeness or generator configuration.

## Limits

- Only static facts. Whether a branch runs, how often a loop iterates and when
  a callback fires are kept as `CHOICE`, `REPEAT` and `phase`, never observed.
- Provenance is a bounded approximation:
  - all definitions of a binding are a flow-insensitive union unless a single
    reaching definition is proven;
  - context fields are resolved by field name across the program;
  - promise results (`.then(cb)` parameters) and values built by unmodelled
    program helpers stay `program_call`/`opaque`.

  The walk depth is 48. Literal and table sources beyond 12 per class are
  counted in `elided`.
- The generic named-object encoder is outside this grammar.
- The alias boundary only covers bindings already known to be emitted buffers;
  a buffer whose origin is not recognized is not tracked, and branch-sensitive
  pointer joins are not implemented.
- Writes after an unconditional `break`/`continue` are still recorded as
  reachable, and a write in a closure the replay never reaches is not in the
  accounting. Both are pre-existing bounded-replay limits, unchanged here.
- A `try`/`catch` in which only one arm writes the buffer stays a choice: a
  write prefix before a throw is approximated by the arm's full write set.
  Full exception and `switch` control flow is not modelled.
