# Generic public wire round trip

`program.js` is a synthetic static-analysis input, composed by the test with
`src/signals/wire/reference/hash.js` and `stream.js`. The placeholders insert
those reviewed generic algorithms with only their function/captured-state names
changed. No captured program, session or signal data is used. JavaScript is
parsed, never executed; no browser, Node, network or external fixture is needed.

The session constructor assigns its argument to `context.transactionToken`,
hashes that field before constructing any collector, and captures that same
state in the stream factory. The character accessor is a concrete single-origin
native wrapper. The signal seed has a four-byte big-endian prefix, while its
source-defined XOR salt is 17. The outer cipher/header scaffolding supplies the
structural facts required by `pin`; it is not a JavaScript implementation or
certification of the outer payload cipher. The public pinned native payload and
header APIs perform those layers.

Independent arithmetic for the golden signal bytes (all arithmetic wraps at
32 bits): token `ab` hashes to `(97 << 16) | 98 = 0x00610062`. Initialize stream
words `a = 0x01020304 ^ 17 = 0x01020315`, `b = 0x00610062`. Each word step emits
`sum = a + b` in little-endian order, then sets `x = a ^ b`,
`b = x ^ rotl(b, 26) ^ (x << 9)`, `a = rotl(x, 13)`. Discard the first word
`0x01630377`; the next word is `0xafd349a2`. Thus the first usable mask byte is
`0xa2`. Expected input vectors are constants, never produced by the Rust signal
encoder under test.

The mock document owns `hasStorageAccess` and lacks `requestStorageAccess`, so
its independently expected flag byte is 1, encoded as `a3`. The second descriptor
writes literal 9, encoded as `ab`, and supplies the literal-mutation oracle.
Both golden blobs are `01 02 03 04` followed by that one masked byte. Descriptor
indices only route these values: semantic flag names must come from the source
probes and public analyzer's bindings.

The integration test also round trips null positional entries, generic positional
values, errors, generic signals and null/non-null keyed entries. It supplies normal
trusted session inputs; it never constructs an AlgorithmReport, WireSchema,
contract, certificate or pin route. Number, string and float programs are outside
this deliberately small byte-only regression.
