# Runtime base64 decoder proof

`prove_runtime_decoder(source, function_span)` returns a `DecoderProof` only
when the complete bounded standard or URL-safe decoder template is present at
the exact function span. It parses and semantically resolves the supplied
JavaScript with Oxc; it never executes JavaScript. Retained decoder aliases may be rewritten into a
temporary proof input, but the original source and its dependency closure are
never changed or treated as if cleanup had removed them.

## Choose the proof for the consumer

Both entry points are exported from `shape` and `shape::runtime_decoder`:

| API | Accepted source alphabet | Current consumer |
| --- | --- | --- |
| `prove_runtime_decoder` | Standard `+/` or URL-safe `-_`; the result records which one matched | Runtime configuration, payload extraction, and the `vmDecodeString` proof/evaluator |
| `prove_standard_decoder` | Standard `+/` only | Callers that admit only the standard alphabet |

The shared matcher checks the same loop geometry, bindings, and effects for
both alphabets. The proof's `rule` is `standard-base64-decoder-v1` or
`url-safe-base64-decoder-v1`; `alphabet` carries the complete accepted alphabet.
Consumers must decode with that alphabet rather than accepting both character
pairs indiscriminately. Neither proof establishes padded-input equivalence.

## Structural and dependency checks

The implementation normalizes local binding names through resolved Oxc
identities, then uses Oxc's structural AST comparison against a small decoder
template. Separate guards verify built-ins, allocator aliases, declaration
dominance, alias closure, and native-root stability; unresolved names cannot
stand in for normalized bindings. This replaces the former
handwritten matcher without weakening arithmetic or extra-effect checks.

The proof is alpha-renaming safe for local variables because local roles are
matched with Oxc `SymbolId` bindings. Required globals (`Uint8Array`, `Array`,
`Math`, and, for the uncurried lookup spelling, `String`) must remain
unshadowed. The external allocation alias must have the exact stable form:

```js
var Alias = typeof Uint8Array === "function" ? Uint8Array : Array;
```

The equivalent exact inline allocator form is also accepted:

```js
new (typeof Uint8Array === "function" ? Uint8Array : Array)(size)
```

The recognized function has two parameters: input and an optional caller
destination. Its body consists of an optional exact native fast path, the
literal alphabet, input-length and destination allocation, seven
uninitialized working locals, and this fixed loop:

```text
for (inputCursor = 0, outputCursor = 0;
     inputCursor < inputLength;
     inputCursor += 4, outputCursor += 3)
```

Each iteration must perform four alphabet lookups at indices `J`, `J + 1`,
`J + 2`, and `J + 3`; compute exactly

```text
Y << 2 | k >> 4
(k & 15) << 4 | H >> 2
(H & 3) << 6 | h
```

and write the first byte unconditionally, followed by the two exact tail
guards at `J + 2 < length` and `J + 3 < length`. The only accepted lookup
forms are direct string methods and the proven uncurried
`String.prototype.indexOf.call` / `String.prototype.charAt.call` forms.

Any changed shift, mask, read index, increment, return, extra effect, extra
early return, nested decoder body, shadowed builtin, or mutated external alias
causes `None`. The manual loop counts raw input length and does not prove the
standard padded-input contract, so `accepts_padding` is always `false`.

The public proof fields are `rule`, `alphabet`, `template`,
`destination_parameter_index`, `accepts_padding`, plus bounded loop and formula
evidence in `evidence`.

## Complete accepted shape

The following is the manual-loop form exercised by
`tests/runtime_decoder.rs`. It is shown as a
whole because the proof rejects extra statements as well as wrong arithmetic:

```js
var Alias = typeof Uint8Array === "function" ? Uint8Array : Array;
function decode(input, destination) {
  var alpha = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  var length = input.length;
  destination = destination || new Alias(Math.floor(length * 3 / 4));
  var a, b, c, d, e, f, g;
  for (var cursor = 0, write = 0; cursor < length; cursor += 4, write += 3) {
    a = alpha.indexOf(input.charAt(cursor));
    b = alpha.indexOf(input.charAt(cursor + 1));
    c = alpha.indexOf(input.charAt(cursor + 2));
    d = alpha.indexOf(input.charAt(cursor + 3));
    e = a << 2 | b >> 4;
    f = (b & 15) << 4 | c >> 2;
    g = (c & 3) << 6 | d;
    destination[write] = e;
    if (cursor + 2 < length) { destination[write + 1] = f; }
    if (cursor + 3 < length) { destination[write + 2] = g; }
  }
  return destination;
}
```

Call `prove_standard_decoder` with the **entire source** and the byte span of
the function, excluding the separate `Alias` declaration. The test constructs
that span from the length of its prefix; the returned proof reports
`standard-base64-decoder-v1`, destination parameter index 1, a four-character
input quantum, a three-byte output quantum, and `accepts_padding = false`.
Renaming local bindings does not affect the result. Changing only `+/` to `-_`
makes `prove_standard_decoder` decline, but `prove_runtime_decoder` can accept
that exact URL-safe variant and records its different alphabet/rule. Changing
a shift, mask, loop increment, or adding a second effect makes both entry
points decline. A declined proof is `None`, not a best-effort decoder guess.

## Payload validation and troubleshooting

Source proof and payload decoding are separate checks. Runtime configuration
first proves the decoder binding and its consumers, then validates each encoded
literal under the proven alphabet and decoded-byte limit. Standard source
rejects `-_` payload characters; URL-safe source rejects `+/`. Padding,
whitespace, malformed lengths, and unsupported characters cannot be rescued by
the repository's more permissive transport decoder. An invalid literal stays
unknown with its original encoded text and diagnostic.

If a decoder is declined, check the exact function span and whole source,
allocator/native binding stability, then the arithmetic and tail guards. A
decoder-looking name, alphabet string, or successful generic base64 decode is
insufficient. For a proved decoder with an unknown payload, inspect the literal
and consumer linkage before changing the structural matcher.

Run the focused proof, extraction, and string-evaluator checks from the
repository root:

```sh
cargo test --test runtime_decoder --test runtime_config --test vm_string_evaluator
```

`runtime_decoder.rs` pins the two API boundaries,
including URL-safe geometry mutations and shadowed built-ins. The separate
string evaluator intentionally remains standard-only.

This proof identifies a decoder implementation in source. It does not itself
decode a kernel payload, establish a string-table key, or authorize folding an
emitted `vmDecodeString` call. That latter hand-off also needs the helper,
table, cache, and call-site binding proofs described in
[the VM string evaluator](vm-string-evaluator.md).
