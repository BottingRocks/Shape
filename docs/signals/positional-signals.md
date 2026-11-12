# Positional signals

`src/signals` is the addressable registry of the signals one recovered
`program.js` collects. It answers, for one exact capture:

- which wave collector owns a signal, and at which `positionalSignals` position;
- which descriptor that signal reports;
- the ordered wire of every emission (`PREFIX` / `NUMBER` / `TEXT` / `BYTE`,
  with `SEQUENCE` / `CHOICE` / `REPEAT` containers);
- the per-emission static XOR literal `C` of `PRNG(seed ^ C)`.

It is derived from the typed facts of
[`extract_program_properties`](../naming/README.md), never from a generated
spelling: a collector is identified by its wave index, a signal by its position
and descriptor, an emission by its source span.

## Address chain

```text
collector (wave index)  ->  positional signal (position)  ->  emission  ->  wire
```

`waveCollectorN` is the positional name contract: the collector registered at
wave index `i` is `waveCollector{i + 1}`. The same capture also has *direct*
signals, built from a registry slot with no wave and therefore no collector
position.

## Honesty rules

- A signal is `partial` only where the extractor reconstructed its writes, and
  `unresolved` with the extractor's own reason otherwise. There is no third,
  promoted state.
- `declared_not_emitted` is not a reconstruction failure. The direct
  constructor installed its task, but the task's whole lexical subtree never
  mentions an observer `next` and only completes: the value does not exist in
  this capture. It is counted apart from `unresolved`.
- A wire node exists only for a write the extractor typed. `Opaque` (the buffer
  is handed to an unmodelled call) and `Unresolved` stay inside the wire so a
  gap is visible at the exact position it occurs.
- An `xor` is the static literal of that emission's mask. It stays absent when
  the mask was not resolved; it is never inferred from neighbouring signals.
- An empty `sequence` is a real branch arm that writes nothing, not a gap.

## API

```rust
use shape::signals::SignalMap;

let map = SignalMap::ssx();                 // the checked-in capture
let signal = map.positional(0, 1).unwrap(); // wave 0, position 1
println!("{} position {} -> descriptor {}", signal.signal, signal.position, signal.descriptor);
println!("{}", signal.shape());             // e.g. "P N *{T}"

let coverage = map.coverage();              // partial / unresolved / emissions
```

| Call | Answer |
| --- | --- |
| `from_properties(&properties)` | the map of any recovered program |
| `collector(wave)` / `collector_named("waveCollector3")` | one collector |
| `positional(wave, position)` | one signal |
| `for_descriptor(n)` | the signal that reports a descriptor |
| `signals()` | every signal, wave-routed first, direct last |
| `coverage()` | status and emission counts |
| `Wire::mnemonic()` | `P N T B`, `*{…}`, `{a \| b}`, `?`, `??` |

## Regenerating the checked-in capture

The capture table under `src/signals/capture/` is data, not source of truth: it
is exactly what the current extractor builds from that program. Regenerate it
after a deliberate change and let the test suite prove the two agree.

```sh
shape trace program ~/Desktop/ssx.mod.js --out program.js --thread-cleanup
shape decode --schema-only --seed program.js --input-kind program --schema-out schema.json
```

`tests/program_signals.rs` re-extracts the checked-in fixture program and
compares every collector, position, emission and wire with the table. A change
in writer recognition, the naming contract or the wire model fails that test
instead of silently drifting.

The provenance block records the program's SHA-256 and the two commands, so a
table can always be traced back to the program that produced it.

## Writer copies

A capture that traces several threads carries one copy of each encoder helper
per thread. The extractor lists every copy for a role and types a write by the
callee's own lexical binding, so each copy types its own writes. This is why
`ssx.mod.js` resolves 70 of its 77 signals even though each of its three writer
roles appears twice.

## Field provenance

A collector that derives one signal from another emits a rebuilt buffer whose
fields are indexed reads of the value it was handed:

```js
function $THREAD_4_76417($AC) {           // $AC is the collected value
  let $AD = …;                             // fresh seed
  let $AE = [$AD >>> 24, …];               // its own prefix
  $AD = $KEY_17($AD ^ 2103546112);         // xor literal
  let $AF = $KEY_274($AD, $AC[0], $AE, 3); // write order:
      $AF = $KEY_274($AD, $AC[3], $AE, $AF);   // 0, 3, 4, 1, 2
      $AF = $KEY_274($AD, $AC[4], $AE, $AF);
      $AF = $KEY_274($AD, $AC[1], $AE, $AF);
           $KEY_274($AD, $AC[2], $AE, $AF);
  $KEY_1.next([[], $AE]);
}
```

Every wire leaf records the read it came from. Only a plain identifier indexed
by a decimal literal is a provenance claim; a computed field, a method call or
an arithmetic index is not.

```json
{ "kind": "number", "value": "$AC[3]",
  "source": { "binding": "$AC", "element": 3 } }
```

An emission renders `order` when every write to its buffer reads the same
collected binding and at least two fields are written. The tuple is the elements
in write order:

```json
{ "origin": [ … ], "xor": 2103546112, "order": [0, 3, 4, 1, 2],
  "wire": { "op": "sequence", "items": [ … ] } }
```

The prefix seed is the collector's own fresh bytes, not a field of the collected
value, so it never participates in the proof. A `choice` between mutually
exclusive arms has no single linear order and is never flattened into one. When
any write is computed, unknown or reads a different binding, `order` is absent;
it is never inferred from shape or from neighbouring signals.

On the checked-in capture two derived emissions render an order, each verified
from the writes in its own source:

| collector | signal | writes | order |
| --- | --- | --- | --- |
| `waveCollector7[0]` | 55 | `$BJ[0] $BJ[4] $BJ[3] $BJ[1] $BJ[2]` | `[0,4,3,1,2]` |
| `waveCollector7[3]` | 50 | `$BL[3] $BL[0] $BL[1] $BL[4] $BL[2]` | `[3,0,1,4,2]` |

The example above is not hypothetical: it is puma capture `175d3a` signal 32
(`$THREAD_4_76417`, xor `2103546112`), which now renders `[0,3,4,1,2]`.

This is what makes derived signals *assignable* rather than merely reconstructed:
the order is the reference generator's `order`/`isReverse`, recovered from source
instead of hand-written.

Export the current public schema from a caller-owned traced program (this
is a separate contract from the historical capture table):

```sh
shape decode --schema-only --seed program.js \
  --input-kind program --schema-out schema.json
```
