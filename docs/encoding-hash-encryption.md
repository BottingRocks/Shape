# Encoding, hashing, and encryption

Focused analysis of the encoding / hashing / encryption subsystem in
`program-invariant-storage-release.js` (17,888 lines, 673 `$THREAD_*`).
Everything below is inferred from the traced code; nothing was executed.

## Layering (from innermost out)

```
keystream PRNG  ($KEY_295 / $KEY_1)        one byte per call, xorshift/ARX
   |  used by every encoder below as the per-byte mask
   |
   +-- string encoder   ($KEY_251 / $KEY_10)   UTF-8-ish + cache + XOR keystream
   +-- number encoder   ($KEY_398 / $KEY_12)   tag/varint or IEEE-754 + XOR
   +-- varint writer    ($THREAD_1_27599)      type-tagged LEB128 for the Token format
   |
31-hash      ($KEY_290 = $THREAD_3_64369, $KEY_247)   Java-style h*31
SHA-256      ($THREAD_1_29477 -> $THREAD_3_71024/72508/103184)   crypto.subtle, 2-stage
   |
Token binary encoder ($THREAD_7_68085 class, $THREAD_1_30764 encodeValue, $THREAD_6_37901 encode)
   |
payload assembler ($THREAD_9_74675)  varint framing + random 8-byte prefix + base64url
   |
encryption:  payload cipher  ChaCha-style ARX  ($THREAD_1_137873 -> $THREAD_9_33778)
             persisted state AES-GCM 128       ($THREAD_1_105431 encrypt, $THREAD_1_108554 decrypt)
```

Two distinct “encryption” paths exist and should not be confused:

* the **payload cipher** (ChaCha-style ARX over a 16×u32 state) protects the
  collected-signal blob that leaves the page;
* the **state crypto** (AES-GCM) protects the JSON state kept in
  `localStorage` (`shj37!*290ge`, `uwi3863hme@34` and related).

## 1. Keystream PRNG

There are two instances of the same generator; only the initial state differs.

| symbol | thread | line | initial state |
|---|---|---|---|
| `$KEY_295` | `$THREAD_8_135449` | 1418 | `s2 = $KEY_67` (= `hashToken(transactionToken)`), `s5 = arg` |
| `$KEY_1` (invariant-storage worker) | `$THREAD_4_104680` | 17793 | `s2 = $KEY_3`, `s5 = arg` |

Algorithm (`$THREAD_5_127562` / `$THREAD_0_136734`): a 4-byte block is produced
lazily; every 4th call mixes the state:

```js
i = (i + 1) % 4;
if (i === 0) {
  out   = (s2 + s5);                 // 32-bit add
  a = s5; b = s2;
  s5n   = a ^ b;
  s2    = a ^ b ^ rotl(b, 26) ^ (s5n << 9);
  s5    = rotl(s5n, 13);
  return out & 0xff;                 // then >>8, >>16, >>24 on later calls
}
```

Output byte order is little-endian (`&0xff`, `>>8`, `>>16`, `>>24`). Callers seed
it as `prng(per_signal_seed ^ per_signal_static_xor)`; the four initial calls are
warm-up. This is *the* mask applied to every encoded byte, so no field is stored
in plaintext except the 4-byte seed prefix (and the AES-GCM key/IV container).

## 2. String encoder — `$KEY_251` (`$THREAD_1_105012`, 1213) / `$KEY_10` (`$THREAD_8_77503`, 17585)

Signature: `enc(value, bytes, outIndex) -> newIndex`.

* **Cache hit** (`stringHist[value]` exists): emit `129 ^ ks()` then
  `number-encode(offset)` — a back-reference into the string table.
* **Cache miss**: encode each UTF-16 code point as UTF-8-ish bytes, each XORed
  with `ks()`:
  * `0x00` → `192 ^ ks()`, followed by `128 ^ ks()`
  * `< 0x80` → 1 byte
  * `< 0x800` → 2 bytes
  * surrogate pair → 4 bytes
  * other `< 0x10000` → 3 bytes
  * terminate with one byte `0 ^ ks()` and record `stringHist[value] = n++`.

## 3. Number encoder — `$KEY_398` (`$THREAD_7_123609`, 1295) / `$KEY_12` (`$THREAD_1_109539`, 17663)

* **int32** (`v|0 === v`): sign bit `64`, magnitude `|v|`, length chosen by
  magnitude with continuation bit `128`:

  | magnitude | bytes | layout |
  |---|---|---|
  | `< 32` | 1 | `sign | v` |
  | `< 4096` | 2 | `sign|32|(v&31)`, `v>>5` |
  | `< 524288` | 3 | …, `127&(v>>5)|128`, `v>>12` |
  | `< 67,108,864` | 4 | …, `v>>19` |
  | else | 5 | …, `v>>26` |

* **non-int**: marker `128 ^ ks()`, then the IEEE-754 binary64 value expanded to
  sign/exponent/mantissa and written as 8 bytes, each XORed with `ks()`.
  Fast path uses a typed array (`$KEY_17`); manual bit extraction is
  `$THREAD_3_68646` (15262).

## 4. Varint writer (Token format) — `$THREAD_1_27599` (1907)

A separate, *type-tagged* integer encoder used only by the Token encoder:
chooses `UINT14/16/24/32/64` tags by magnitude and pushes 1–9 bytes
(`$THREAD_9_74675` has an inlined LEB128 variant for framing lengths).

## 5. Hashing

### 5a. 31-hash — `$KEY_290` (`$THREAD_3_64369`, 912), chunked `$KEY_247` (`$THREAD_5_145927`, 865)

```js
h = 0;
// strings: consume two UTF-16 units at a time
h = (h * 31 + ((s.charCodeAt(i) << 16) | s.charCodeAt(i + 1))) | 0;
// ArrayBuffer/typed view: consume Int32 words
h = (h * 31 + int32word) | 0;
```

Chunked variant processes 400 words per scheduler tick and reports through the
queue, so long buffers do not block. Dispatched by `$KEY_273`
(`$THREAD_2_30423`, 1707): string → `$KEY_416`, view → `$KEY_247`, else throws
`TypeError`.

### 5b. SHA-256 — `$THREAD_1_29477` (2072)

Two-stage digest over per-signal chunks:

1. for every `[data, sidx]` pair, `crypto.subtle.digest("SHA-256",
   Uint8Array(data))` → `Promise.all` (`$THREAD_3_71024`);
2. concatenate all digests and digest **again** (`$THREAD_3_72508`);
3. lowercase hex (`$THREAD_9_103184`, byte→hex `$THREAD_7_122084`) →
   `{ sidx, hash }`, `catch` → `null`.

So a signal’s hash is `SHA256(concat(SHA256(chunkᵢ)))`, not a plain SHA-256 of
the concatenation.

## 6. Encryption

### 6a. Payload cipher — ChaCha-style ARX (`$THREAD_1_137873` 4191, `$THREAD_9_33778` 4200)

16×u32 state: `[0..3]` IVs, `[4..11]` eight session keys, `[12..13]` 64-bit
block counter advanced per 64-byte block, `[14..15]` the two payload seeds.
Runs double rounds (8 quarter-rounds each), adds the initial state back, packs
32-bit words little-endian, XORs the keystream with the data. **Verified count in
this release:** 48 `rotl16` quarter-rounds inside the `slice()` → add-back region
of `$THREAD_9_33778` (lines 4200–4700) ⇒ **6 double rounds** (not the ChaCha20
nominal 20). The 4-byte seed prefix and base64url alphabet are outside this
function.

### 6b. State crypto — AES-GCM (`$THREAD_1_105431` 3691, `$THREAD_1_108554` 3745)

Encrypt:

```js
data = new TextEncoder().encode(json);
key  = crypto.subtle.generateKey({name:"AES-GCM", length:128}, true, ["encrypt","decrypt"]);
iv   = crypto.getRandomValues(new Uint8Array(12));
ct   = crypto.subtle.encrypt({name:"AES-GCM", iv}, key, data);
raw  = crypto.subtle.exportKey("raw", key);          // 16 bytes
out  = raw || iv || ct;                              // packed ArrayBuffer
```

Decrypt splits `out` at **16** (key) and **28** (IV), imports the raw key,
decrypts, `TextDecoder` + `JSON.parse`. The packed buffer is persisted as
`new Uint8Array(buf).toString()` (comma-separated decimal bytes).

## 7. Token binary encoder

Class factory `$THREAD_7_68085` (15347); constructor `$THREAD_0_61918` (depth
checks, `keysets[]`, `stringHist{}`, `stringPlaceholders`); `encode`
`$THREAD_6_37901` (15402); `encodeValue` `$THREAD_1_30764` (15517); statics
`encode` `$THREAD_4_133003` (15774) and `extend` `$THREAD_9_144325` (15126).

`encodeValue` dispatches on type and emits the type tag before the payload:

| value | encoding |
|---|---|
| extension match | `EXTENSION`, extension id, serialised body |
| `false` / `true` / `null` / `undefined` | single tag |
| finite number | integer → `$THREAD_1_27599`; else `FLOAT32` / `DOUBLE64` (`$THREAD_7_14757` 15237) |
| string | `STRLUT`-based: `STRREF` + index if in `stringHist`, else `STR5_BASE|len`, `CSTRING`, `STR8 len`, or `STR_` + varint len |
| array | `ARRAY_` + length + elements (`$THREAD_8_27919`) |
| typed array | `BARRAY8` + … |
| object / map | `MAP_` / `MAPL_` + keysets (`$THREAD_8_38219`) |

`encode` prepends a `STRLUT` section built from `stringHist` and dedups every
string through `stringPlaceholders` before writing keysets.

## 8. Payload assembly — `$THREAD_9_74675` (15876)

Framing (unsigned LEB128 lengths), after optional per-section Token encoding by
`$KEY_397`:

```
varint(len(errors)) || errors
for each positionalSignal:
    varint(len(encoded)) || encoded
    varint(len(extra))   || extra
varint(len(signals)) || signals
for each keyed signal: varint(name) || varint(len(encoded)) || encoded || varint(len(extra)) || extra
```

The assembled byte array is then given a random 8-byte prefix (the two seeds)
and base64url-encoded with the launcher alphabet. The tracker/collector side
(`$KEY_400`, `$KEY_303`, `$KEY_433` …) supplies `descriptor`, `signalFilter`,
`notifyLastChance`.

## 9. Thread inventory (encoding / hash / encryption / seed)

Highest-signal entries (★ = importance, L = source lines):

| thread | line | ★ | L | name | category |
|---|---|---|---|---|---|
| `$THREAD_9_33778` | 4200 | 5 | 501 | chacha20_stream_xor | encryption |
| `$THREAD_1_137873` | 4191 | 5 | 512 | create_chacha20_cipher | encryption |
| `$THREAD_7_68085` | 15347 | 5 | 486 | tokens_encoder_class_factory | encoding |
| `$THREAD_3_50484` | 15847 | 5 | 334 | token_encode_setup | encoding |
| `$THREAD_9_74675` | 15876 | 5 | 303 | encode_security_signals | encoding |
| `$THREAD_1_30764` | 15517 | 5 | 254 | encode_value | encoding |
| `$THREAD_6_37901` | 15402 | 5 | 68 | encoder_encode | encoding |
| `$THREAD_4_133003` | 15774 | 5 | 53 | encode_factory | encoding |
| `$THREAD_1_43889` | 13692 | 4 | 455 | media_features_serializer | encoding |
| `$THREAD_1_109539` | 17663 | 4 | 129 | obfuscated_number_encode | encoding |
| `$THREAD_8_72596` | 14779 | 4 | 104 | high_entropy_values_encoder | encoding |
| `$THREAD_8_77503` | 17585 | 4 | 77 | make_string_encoder | encoding |
| `$THREAD_5_39657` | 17592 | 4 | 68 | obfuscated_string_encode | encoding |
| `$THREAD_3_68646` | 15262 | 4 | 83 | encode_float_manual | encoding |
| `$THREAD_7_14757` | 15237 | 4 | 23 | encode_float_fast | encoding |
| `$THREAD_1_105431` | 3691 | 4 | 53 | encrypt_state_aes_gcm | encryption |
| `$THREAD_1_108554` | 3745 | 4 | 47 | decrypt_state_aes_gcm | encryption |
| `$THREAD_5_93557` | 3711 | 4 | 28 | aes_gcm_encrypt_pack | encryption |
| `$THREAD_8_128488` | 3722 | 4 | 15 | export_and_pack_ciphertext | encryption |
| `$THREAD_2_38317` | 3725 | 3 | 10 | concat_key_iv_cipher | encryption |
| `$THREAD_8_14882` | 3762 | 3 | 14 | aes_gcm_decrypt | encryption |
| `$THREAD_1_29477` | 2072 | 4 | 51 | signal_sha256_aggregator | hash |
| `$THREAD_2_14509` | 1720 | 4 | 42 | hash_string_async | hash |
| `$THREAD_4_137662` | 1731 | 4 | 28 | hash_string_chunk | hash |
| `$THREAD_3_71024` | 2078 | 3 | 11 | sha256_pair_digest | hash |
| `$THREAD_3_72508` | 2089 | 3 | 18 | sha256_join_digest | hash |
| `$THREAD_9_103184` | 2107 | 3 | 11 | hash_to_hex_result | hash |
| `$THREAD_3_64369` | 912 | 3 | 48 | hash_value (h*31) | hash |
| `$THREAD_5_145927` | 865 | 3 | 47 | string_hash_scheduled | hash |
| `$THREAD_2_30423` | 1707 | 4 | 12 | hash_dispatch | hash |
| `$THREAD_8_135449` | 1418 | 3 | 43 | init_prng (keystream) | seed |
| `$THREAD_5_127562` | 1424 | 2 | 28 | prng_next_byte | seed |
| `$THREAD_4_104680` | 17793 | 4 | 43 | make_prng_byte_stream | seed |
| `$THREAD_0_136734` | 17799 | 4 | 28 | prng_next_byte (invariant) | seed |
| `$THREAD_1_105012` | 1213 | 3 | 81 | init_string_encoder | encoding |
| `$THREAD_4_10376` | 1220 | 3 | 72 | encode_string_worker | encoding |
| `$THREAD_7_123609` | 1295 | 3 | 122 | encode_varint_value | encoding |
| `$THREAD_1_27599` | 1907 | 4 | 48 | write_varint | encoding |
| `$THREAD_4_128521` | 1955 | 3 | 8 | format_header_summary | encoding |
| `$THREAD_1_125227` | 1958 | 2 | 3 | format_header_token | encoding |
| `$THREAD_7_122084` | 2108 | 2 | 4 | byte_to_hex2 | encoding |

All 77 encoding/hash/encryption/seed entries (with purpose + notes) are in
`docs/thread-ledger.json`; filter on `category in {encoding,hash,encryption,crypto,seed,base64}`.

## 10. Notes and limits

* Field-level `XOR` masks are the keystream bytes, not constants; constants only
  appear as the *pre-mask* value (e.g. `130`, `131`, `192`, `128`, `64`, `32`).
* `$KEY_398` is the number encoder, `$KEY_251` the string encoder, `$KEY_295`
  the keystream — these three are the whole encoding core; everything else is
  framing or hashing.
* The Token encoder is a separate serialisation used for the outer payload
  sections; the `$KEY_398`/`$KEY_251` road is used by the per-signal byte
  payloads (`$BYTES`-style arrays).
* Round count of the ARX cipher, the transaction-token hash base (`$KEY_67`),
  and the base64url alphabet are build/seed-specific and must be taken from the
  matched seed, not from another capture.
