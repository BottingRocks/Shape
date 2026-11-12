# VM string evaluation and emitted folding

This reference covers proof-backed evaluation of `vmDecodeString` and its
post-emission integration. The [module guide](../../src/vm_string/README.md) gives
the API choices, ownership map, and focused tests.

Read [proof boundary](#proof-boundary) for admission, [evaluator semantics](#evaluator-semantics)
for bytes and caching, [three entry points](#three-entry-points) for source/report/plan
handoffs, and [integration](#integration) for emitted-call replacement.

## Why this exists

The string-decoding helper is deliberately unremarkable JavaScript: a table
lookup, two manual base64 decodes, an XOR mask, and a `String.fromCharCode`
loop. When the encoded input and key index are source literals, all of that work
is statically determined by the same bytes the helper would read at runtime.
Leaving the call in emitted output keeps a function call, two decodes, an XOR
loop, and a cache lookup for a value that can be written as one literal.

The alternative—executing the captured decoder—would require running untrusted
JavaScript and trusting its globals, prototypes, getters, and cache state. This
module instead proves a fixed helper/decoder pair against source, evaluates the
proven arithmetic in Rust, and preserves the result as lossless UTF-16. If any
part of that chain is uncertain, the call stays.

**Reduces:** calls, base64/XOR work, helper invocations, and string-building
loops in emitted output, replacing them with one string literal. It does not
remove the helper declaration itself.

This is not a general constant folder: one helper body, one decoder template,
one table shape, and one cache shape are recognized; only proven calls shrink.

## Proof boundary

`VmStringProof::prove` requires four things:

1. the exact source bytes, helper span, and decoder span;
2. an AST-structural match for the complete seven-parameter `vmDecodeString`
   helper, normalized through resolved binding identity;
3. `prove_runtime_decoder` success for the exact manual-loop decoder;
4. explicit, all-true `DependencyAssumptions::trusted_static()` facts.

The proof stores a SHA-256 of the **entire source bytes**, both spans, the
decoder proof, and the dependency facts. `verify_source` rejects a different
digest and re-proves both components at the stored spans. A proof is therefore
bound to one source identity and two specific occurrences, not to a function
name.

Names are not evidence. The helper matcher parses the source, resolves every
binding/reference to an Oxc `SymbolId`, renames resolved bindings to `r0`–`r15`
in first-seen order, allows only `undefined` as an unresolved reference, and
compares the normalized function with a normalized canonical function. Renamed
parameters, locals, and functions are accepted; changed arithmetic, extra
statements, captures, shadowed externals, a nested function in the target, or
more than 16 bindings are not.

The helper shape is exactly the seven-parameter, non-async, non-generator
`vmDecodeString` with the early cache return, two `decodeBase64` calls, the
`(keyBytes[0] + encodedBytes[0]) & 255` mask, the `fromCharCode` loop, and the
cache store. The decoder proof accepts the standard `+/` or URL-safe `-_` alphabet with
the fixed four-character loop and bounded tail stores, and the evaluator reads
operands only in the alphabet it proved. Padding, whitespace, and other decoder
templates are not admitted here.

### Why the decoder boundary is narrow

`prove_runtime_decoder` is a whole-function template match over two named
alphabets. It proves the `+/` or `-_` rule, a four-character input quantum with
three output bytes and two tail guards, destination parameter index 1, and
`accepts_padding = false`. The shared matcher also requires the complete body,
unshadowed built-ins, the exact allocator alias form, alias closure, and no
dynamic hazards. Exact immutable aliases may be expanded in a temporary proof
input; the source is never modified.

Padding is outside the manual-loop contract, so `accepts_padding` is always false.

### Table and cache

A table is a non-empty array literal of string literals whose units form valid
Rust strings; an entry with an unpaired surrogate makes the candidate ambiguous
and declines. A cache is a non-destructuring declarator of any declaration kind
whose initializer is exactly `Object.create(null)`, with an unresolved global
`Object` and one `null` argument. The report supplies a binding name, but the
name is only a search hint: the collector requires exactly one matching
declarator/symbol, otherwise the fold is ambiguous.

The evaluator owns a Rust `VmStringTable` and `VmStringCache`. A cache must be
created with `TrustedNullPrototypeDataOnly`: a normal object, accessor, proxy,
or unknown prototype is not silently treated as safe.

## Admitted helper

The canonical body is checked as a complete function:

```js
function vmDecodeString(encodedString, keyIndex, stringTable, cache,
                        decodeBase64, fromCharCode, computeOnly) {
  if (!computeOnly) {
    var cacheKey = encodedString + "," + keyIndex;
    var cachedString = cache[cacheKey];
    if (typeof cachedString !== "undefined") { return cachedString; }
  }
  var keyBytes = decodeBase64(stringTable[keyIndex]);
  var encodedBytes = decodeBase64(encodedString);
  var xorMask = keyBytes[0] + encodedBytes[0] & 255;
  var decodedString = "";
  for (var byteIndex = 1; byteIndex < keyBytes.length; ++byteIndex) {
    decodedString += fromCharCode(
      encodedBytes[byteIndex] ^ keyBytes[byteIndex] ^ xorMask);
  }
  if (!computeOnly) { cache[cacheKey] = decodedString; }
  return decodedString;
}
```

Spelling is not compared; the normalized structure is. The decoder is proven
separately, so a same-named function with different arithmetic cannot pass.

### Helper ABI

| Position | Parameter | Proven role |
| --- | --- | --- |
| 0 | `encodedString` | input to `decodeBase64`; cache-key half |
| 1 | `keyIndex` | selects the table entry; cache-key half |
| 2 | `stringTable` | data-only table |
| 3 | `cache` | null-prototype cache |
| 4 | `decodeBase64` | separately proven decoder |
| 5 | `fromCharCode` | unshadowed `String.fromCharCode` |
| 6 | `computeOnly` | optional; falsy enables cache lookup/store |

The declaration needs exactly seven parameters. Source calls may pass six
(`computeOnly` is then falsy) or seven with a literal `false`; a literal `true`
or non-literal seventh fails. The integration source check also requires at
least one helper call, so an emitted name alone never authorizes a fold.

<a id="exact-admitted-domain"></a>

## Evaluator semantics

- **Values are UTF-16 code units.** `Utf16String(Vec<u16>)` preserves every code
  unit, including unpaired surrogates. Conversion to Rust `String` is fallible
  and never replaces them.
- **Byte arithmetic.** Both decoded operands must be non-empty and equal length.
  The mask is `key[0].wrapping_add(encoded[0]) & 255`; output starts at byte 1
  and each unit is `encoded[i] ^ key[i] ^ mask`. Output length is
  `key.len() - 1`.
- **Base64.** Only `A-Z a-z 0-9 + /` is accepted. Empty input, a length with
  remainder 1, non-alphabet bytes, and oversized inputs/outputs are rejected.
  Remainders 2 and 3 use the same bounded tail stores as the manual loop; unused
  trailing bits are not re-validated for canonicality.
- **Cache.** The key is exactly `encoded + "," + keyIndex`. A lookup happens
  before reading `stringTable[keyIndex]`, matching the helper's early return. A
  hit is rechecked against current decoded-byte and output-unit limits.
  `computeOnly` skips both lookup and store, although the initial cache-size
  check still runs. Capacity is strict.
- **Limits.** Defaults are 64 KiB encoded input (bytes; the accepted alphabet is
  ASCII), 64 KiB decoded bytes, 64 KiB output units, 4096 cache entries, and 256
  cache-key units.
- **Typed errors.** Missing proof, unknown input/key, oversized input/cache/key/
  output, untrusted cache, out-of-range key index, invalid key/input base64,
  empty/unequal decoded operands, and unknown/empty raw bytes become typed
  decline results, never guessed strings.

`evaluate_raw_xor` is a lower-level utility with no proof authority. It is for
tests or callers that already have independent byte evidence.

The cache key intentionally does not include source, proof, dependency, or
table identity because that would change the admitted helper's JavaScript
semantics. Callers must therefore scope one cache to a compatible source,
dependency capture, and string table, retaining the proof digest and checking
those identities at the integration boundary. A cache reused across different
tables or proofs can legitimately produce the helper's early cache hit, but
must not be treated as evidence that the new table or proof produced the
cached value.

### Evaluation order

1. Missing proof (`MissingProof`) or untrusted dependencies/cache (`UntrustedCache`).
2. Cache length over `max_cache_entries` (`CacheTooLarge`).
3. Unknown input (`UnknownInput`), then unknown key (`UnknownKey`).
4. Input over `max_input_chars`; unless `compute_only`, cache key over `max_cache_key_units`.
5. Unless `compute_only`, lookup and return a rechecked `CacheHit`.
6. Key index, key decode, input decode (`KeyOutOfBounds`, `InvalidBase64`).
7. Raw XOR checks and arithmetic.
8. Unless `compute_only`, store under a strict capacity check; otherwise
   `cache_stored: false`.

The order is observable: cache size is checked before input validity, and a
`compute_only` request still enforces input, decoded-byte and output bounds;
it skips the cache-key bound because it neither looks up nor stores that key.

### Operand and cache edge cases

| Case | Result |
| --- | --- |
| empty input, remainder 1, or non-alphabet byte | `InvalidBase64` |
| remainder 2 or 3 | bounded tail store; trailing bits not revalidated |
| one-byte key | empty result string |
| mask above 255 | wrapping add; `255 + 255` becomes 254 |
| encoded length differs from key | evaluator rejects unequal; folder truncates longer, declines shorter |
| cache at capacity, miss | store refused (`CacheTooLarge`, `actual = len + 1`) |
| cache hit at capacity or bad index | hit wins; no table read |

## API and type map

`shape::vm_string` is public; the crate root re-exports only the report and plan
entry points, `VmStringFoldPlan`, and the version constant from this module.

| Item | Contract |
| --- | --- |
| `VmStringProof` | digest, spans, decoder proof, facts; `prove` and `verify_source` |
| `VmStringProofError` | span, parse, helper, decoder, dependency, source mismatch |
| `DependencyAssumptions` | four facts; `trusted_static()` is all-true |
| `CacheAssumption` | unproven or trusted null-prototype data-only |
| `VmStringLimits` | five budgets with the documented defaults |
| `Utf16String` | lossless units; fallible `to_string`, never replacement |
| `VmStringTable` / `VmStringCache` | Rust data-only table / explicit-prototype cache |
| `VmStringRequest` / `VmStringResult` / `VmStringEvaluator` / `VmStringError` | operands and `compute_only`; hit or computed; bounded evaluation; typed declines |
| `decode_manual_base64` / `evaluate_raw_xor` | standard decoder / raw arithmetic, no proof authority |
| `fold_emitted_vm_strings` (+ `_from_report`, `_from_plan`) | raw, saved-report, and plan entry points |
| `VmStringFoldPlan` / `VmStringFoldReason` | plan compile/validate; twelve labels via `as_str()` |
| `VmStringFoldReport` / `VmStringFoldResult` / `VmStringIntegrationError` | counts/reasons/diagnostics; artifact; hard errors |

## Limits and budgets

| Budget | Default | Checked by |
| --- | --- | --- |
| encoded input bytes | 65,536 | evaluator and decoder |
| decoded bytes per operand | 65,536 | decoder, XOR, cache hit |
| output units | 65,536 | XOR, cache hit |
| cache entries | 4,096 | before read and store |
| cache-key units | 256 | before lookup |
| plan entries / entry bytes | 65,536 each | plan validation |
| plan decoder index / decline detail | 32 / 4,096 | plan validation |

All bounds are checked before the protected allocation. The integration uses
`VmStringLimits::default()` with no CLI override; API callers may pass tighter
limits, and a hit is rechecked against them.

## Three entry points

| Entry point | Trust input | What is re-proven | Hard failure |
| --- | --- | --- | --- |
| `fold_emitted_vm_strings(capture_source, report, emitted)` | Raw capture must hash to `report.input_hash` | Helper/decoder/table/cache/scope proofs rebuilt against `report.output` | `ReportInputMismatch` |
| `fold_emitted_vm_strings_from_report(report, emitted)` | Saved report is validated; its input hash is a declared identity | Same inner proof against `report.output` | invalid saved report |
| `fold_emitted_vm_strings_from_plan(plan, emitted)` | Plan shape/version/provenance | No pipeline/source parse; only emitted JavaScript is parsed, then plan names/entries drive calls | invalid plan/emitted parse |

A source-backed fold re-proves against the standardized pipeline output rather
than reusing earlier spans, because final presentation may have moved them. The
raw capture is hashed but never executed.

### Entry-point contracts

- **Raw:** FNV-1a over the capture must equal the report's 16-hex hash (hard
  `ReportInputMismatch`), then the proof is rebuilt against `report.output`.
- **Saved report:** `validate_saved_report` checks output, hash, bounds, spans,
  and extraction lengths; the inner proof is still rebuilt.
- **Plan:** `validate()` runs first; a declined plan returns its reason without
  parsing emitted code, while a proven plan synthesizes a zero-digest proof.

All three end in the same proven tail: parse emitted code, link calls, evaluate,
replace spans, and reparse.

### Fold plan

`VmStringFoldPlan` is the compact artifact-backed handoff. It contains the plan
schema version, compiler provenance, optional helper/decoder/table/cache names,
string-table entries, four dependency facts, decoder destination-parameter
index, padding flag, and an optional typed decline. It carries no helper
JavaScript, AST, or source fragments.

`VmStringFoldPlan::compile(report)` is called only while a source-backed report
is available. It runs the same table/cache/decoder/native-dependency proofs and
produces either a complete plan or an explicit decline. `validate()` checks the
version, provenance, bounds, required fields, and, for a non-declined plan,
`accepts_padding == false`. A declined plan skips the required-field and padding
checks but must carry a detail of at most 4096 bytes.

A plan is **not itself a source proof**. `VmStringProof::from_compiled_plan`
synthesizes a proof with a zero digest and no spans. Artifact-backed trust comes
from the enclosing artifact: the plan is part of the wire payload covered by
`integrity_digest`, and loading recomputes that digest. A structurally valid
public plan can drive folding, but it cannot pass `verify_source` or establish
that the plan came from a particular capture. Treat it as a trust-boundary
handoff, not as a substitute for the source proof.

### Plan fields

| Field | Meaning |
| --- | --- |
| `schema_version`, `compiler_provenance` | exact version and compiler identity |
| `helper_name`, `decoder_name` | proven occurrence names for linkage |
| `table_name`, `cache_name` | report binding identities |
| `table_entries` | literal string-table data; no helper JavaScript |
| `decoder_destination_parameter_index`, `decoder_accepts_padding` | defaulted; index ≤32, padding false when proven |
| `decoder_url_safe` | defaulted false and omitted when false; true for the `-_` alphabet |
| `declined` | typed reason plus detail of at most 4096 bytes |

<a id="emitted-trace-integration"></a>

## Integration

- **Thread:** only after a completed trace: emit → fold → strict property
  writes → array constructions → undeclared-name check → write. The fold report
  is retained and printed as `vm strings: N folded, M declined`, reason counts,
  and up to three diagnostics.
- **Corpus `--cleanup`:** checks the unfolded emitted code first, then runs
  fold → property writes → arrays and checks the cleaned code. The traced code
  returned for later checks remains unfolded; a hard cleanup error becomes
  `cleanup failed: ...`.
- **Program:** each function is emitted, folded, property-cleaned, and
  array-cleaned before linking; stage dumps include `01-vm-strings.js`.
- **Worker emission:** artifact-backed emission folds if a plan exists;
  otherwise it returns unchanged code with a `HelperOrDecoderNotProven` decline.

Only the call-expression span is replaced. The literal emitter uses readable
characters where possible, `\"`/`\\` where needed, `\uXXXX` for controls and
lone surrogates, and real characters for valid surrogate pairs. Declined calls
and unrelated declarations remain. `EmittedJavaScript` statement and node maps
are preserved unchanged because no statement is removed or reordered. If any
edit is rejected, the operation is a hard `InvalidOutput`, not a partial fold.
Calls whose callee name is not the plan's helper are silently skipped rather
than counted as declines.

Folding precedes property/array cleanup because the property pass leaves runtime
`vmDecodeString` keys alone until string folding proves them.

### Emitted-call link

The collector parses only `emitted.code` and requires: callee identifier equal to
`helper_name` with no resolved symbol; six arguments, or seven with a boolean
literal seventh; string-literal encoded input; finite non-negative integer key;
plain identifier table/cache/decoder names with no symbols; and
`String.fromCharCode`/`String["fromCharCode"]` with unresolved `String`.

A same-name callee that resolves locally declines `EmittedCallNotProven`; a
different callee name is skipped without a decline. The folder then decodes the
table entry and the whole literal, declines if the literal is shorter than the
key, truncates to the key length, re-encodes unpadded, and evaluates with
`compute_only: true`, so folds are independent per call and no runtime cache hit
is simulated.

## Decline reasons

| Reason | Meaning |
| --- | --- |
| `ReportInputMismatch` | raw capture does not match the report's input hash |
| `MissingStringTable` / `MissingStringCache` | no extracted fact or binding identity |
| `AmbiguousStringTable` / `AmbiguousStringCache` | zero or multiple matching declarators, usually shadowing |
| `DynamicScope` | `with` or direct `eval` prevents stable binding resolution |
| `MutableDependency` | table/cache/decoder/helper alias, escape, write, member call, delete, update, or native `fromCharCode` mutation |
| `HelperOrDecoderNotProven` | no unique exact helper/decoder pair, or the output did not parse |
| `HelperCallDependencyMismatch` | a source helper call does not use the proven table/cache/decoder/fromCharCode bindings |
| `EmittedCallNotProven` | an emitted call does not resolve to the proven identities |
| `EvaluationRejected` | bounded evaluator rejected the concrete input/key/cache |
| `Utf16LiteralUnavailable` | result could not be emitted as a lossless literal |

Soft declines return the original artifact unchanged with a report. Hard parse,
edit, or output failures abort the caller.

### What each decline prevents

| Label | Raised at | Prevents |
| --- | --- | --- |
| `report-input-mismatch` | raw entry | using another capture's report |
| `missing-string-table-proof` | fact lookup | guessing a table |
| `missing-string-cache-proof` | fact lookup | guessing a cache |
| `ambiguous-string-table-binding` | collection | choosing among shadowed or malformed tables |
| `ambiguous-string-cache-binding` | collection | choosing among shadowed caches |
| `dynamic-scope` | whole source | reasoning through `with` or direct `eval` |
| `mutable-dependency` | whole source | observing or replacing a protected binding |
| `helper-or-decoder-not-proven` | pair search | folding a same-shaped lookalike |
| `helper-call-dependency-mismatch` | source ABI | ignoring a call outside the ABI |
| `emitted-call-not-proven` | emitted link | folding a same-name call with wrong dependencies |
| `evaluation-rejected` | evaluator | inventing a value for invalid bytes |
| `utf16-literal-unavailable` | literal encoder | substituting a lossy character |

Whole-source stages decline once and return the input unchanged; per-call stages
count each call. `thread` prints at most three diagnostics. `ReportInputMismatch` is a hard error at the raw-source entry point.

<a id="value-and-identity-boundary"></a>

## Assumptions and non-goals

The pass assumes table entries are data-only literals; the cache is a
null-prototype data-only object with no getters, proxies, or inherited
properties; the decoder is the exact manual standard-base64 template;
`String.fromCharCode` is the unshadowed intrinsic; all helper calls use the
proven ABI; and no `with`/direct `eval` destabilizes binding resolution.

It does not model BigInt, Symbol, prototypes, accessors, proxies, object/heap
identity, or shared closure cells. `VmStringTable` entries are Rust strings, not
JavaScript arrays, and `VmStringCache` is a Rust-owned map whose prototype
assumption is explicit. A result carries no heap identity, so it must not be
read as an observation of an object or closure cell.

The module deliberately does not add hooks to the symbolic tracer, HandlerIR,
or dispatch layers. It is a post-emission transform.

## Invariants

1. An unproven folding stage returns the input with a typed decline. Identity,
   report/plan validation, parse, edit, and output failures are hard errors.
2. Names never prove: spans, source digest, resolved symbols, and exact AST
   structure decide; names are hints or presentation links.
3. Only call spans change; statement/node maps and `object_parameters` survive.
4. No partial rewrite: a rejected edit is `InvalidOutput`.
5. UTF-16 is lossless; lone surrogates survive and `to_string` may fail.
6. Unknown, invalid, or oversized input declines. The standalone evaluator
   requires equal operand lengths; integration applies its proven truncation rule.
7. Limits are checked before allocation or cache growth.
8. Cache order and key mirror the helper; hits are rechecked against limits.
9. A plan cannot bypass validation or pass `verify_source`.
10. Evaluation, source proof, and emitted linkage are separate authorities.

<a id="worked-bytes-and-cache-behavior"></a>

## Worked examples

### Decoder and XOR bytes

```text
decode_manual_base64("AQID") -> [1, 2, 3]
decode_manual_base64("AQ")   -> [1]
decode_manual_base64("AQI=") -> error: outside the alphabet
decode_manual_base64("-_8")  -> error: outside the alphabet
decode_manual_base64("A")    -> error: impossible remainder

evaluate_raw_xor(encoded=[4, 0xE9], key=[1, 2]) -> mask 5
unit = 0xE9 ^ 2 ^ 5 = 0xEE -> [0xEE]
```

### One emitted fold

With table entry `"AAAA"` → `[0, 0, 0]` and literal `"AQID"` → `[1, 2, 3]`,
the mask is 1, so the units are `2 ^ 0 ^ 1 = 3` and `3 ^ 0 ^ 1 = 2`:

```js
// before
return vmDecodeString("AQID", 0, vmStringTable, vmStringCache, decode,
                      String["fromCharCode"]);
// after
return "\u0003\u0002";
```

### Cache and `computeOnly`

With key `"AQI"` → `[1, 2]` and input `"BOk"` → `[4, 0xE9]`, mask 5 gives unit
`0xEE`. The first normal request is `Computed { cache_stored: true }`, the second
is `CacheHit`, and `compute_only: true` is `Computed { cache_stored: false }`
without changing the cache. A hit still wins with an empty table;
`compute_only` then returns `KeyOutOfBounds`, matching the helper's order.

### Declines by stage

`function vmDecodeString() {}` with report facts but no table declaration stops
at `AmbiguousStringTable` before helper proof. `var tableAlias = vmStringTable;`
declines `MutableDependency`. A table entry `"AQI="` decodes the table but the
key decode fails, so the call declines `EvaluationRejected`. A local parameter
named `vmDecodeString` in emitted code declines `EmittedCallNotProven`.

## Troubleshooting

| Symptom | Likely cause | Check |
| --- | --- | --- |
| call remains, no decline | callee name differs from plan | `helper_name` vs emitted callee |
| one whole-source decline | table, cache, scope, pair, or dependency | the first reason and detail |
| ambiguous table with one visible table | shadowing, duplicate, or malformed initializer | declarator count; holes/spreads/lone surrogates |
| `mutable-dependency` | alias, escape, member call, write, delete, update | every protected-binding reference |
| `helper-call-dependency-mismatch` | no call, `true` seventh arg, wrong position | helper ABI and source calls |
| "shorter than the proven key" | key index and literal disagree | table lengths and key index |
| key/input decoder rejection | padding, URL-safe, whitespace | exact entry and literal alphabet |
| literal is mostly `\uXXXX` | controls or lone surrogates | intentional lossless escaping |
| `SourceMismatch` | source changed after proving | digest, spans, snapshot |
| plan invalid | version, provenance, padding, empty names | `validate()` and recompile |
| `ReportInputMismatch` | capture differs from report | exact capture or saved report |

## Testing and related docs

The evaluator tests cover the admitted helper/decoder, byte arithmetic, base64
tails, UTF-16 preservation, cache behavior, limits, and typed declines. The
integration tests cover source/report/plan entry points, binding ambiguity,
mutable dependencies, decline reporting, and exact output mappings.

### Test and fixture matrix

| Suite | Tests | Pins |
| --- | --- | --- |
| [`tests.rs`](../../src/vm_string/tests.rs) | 2 | padding/URL-safe; non-ASCII raw units |
| `tests/vm_string_evaluator.rs` | 9 | decoder, XOR, proof/cache/`computeOnly`, bounds, identity, variants |
| [`integration.rs`](../../src/vm_string/integration.rs) tests | 12 | folds and maps, plan/report parity, declines, literals, pair search |
| [`retained-decoder-alias.js`](../../tests/fixtures/retained-decoder-alias.js) | fixture | exact decoder aliases shared by three suites |

```sh
cargo test --lib vm_string
cargo test --test vm_string_evaluator
```

### Related docs

- [Evaluator contract](vm-string-evaluator.md)
- [Decoder proof](runtime-decoder.md#choose-the-proof-for-the-consumer)
- [Emitted cleanup pipeline](../transforms/emitted-program-cleanup.md#pipeline)
- [Emitted property writes](../transforms/emitted-property-writes.md#provenance-and-cli-integration)
- [Trace CLI](../tracing/trace-cli.md)
- [Source modules](../reference/source-modules.md)
- [Deobfuscation and remaining calls](deobfuscation.md)
