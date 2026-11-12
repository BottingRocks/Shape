# Generic codec proof references

These source-only functions express reviewed algorithms. They are parsed
statically and never run. Identifier spelling is deliberately generic.

`stream.js` emits little-endian bytes of `a + b`, updates
`a = (a XOR b) XOR rotl(a,26) XOR ((a XOR b) << 9)` and
`b = rotl(a_old XOR b_old,13)`, and discards the first four bytes. Its captured
external input is the verified token hash; its parameter is captured seed XOR
salt. `hash.js` hashes paired UTF-16 units with wrapping `h*31 + pair` on its
string path; the character accessor dependency is checked separately.

`number.js` has the 5-bit first magnitude group, 7-bit continuations and sign
bit 6. Only its complete integer branch receives a certificate. Its float path
remains unproved. `string.js` and `string-main.js` express equivalent modified
UTF-8 writers with tag 129 intern references, stream-masked zero terminators,
and a factory-local table. Their number and character-accessor dependencies
and each emission's table lifetime require separate proofs.

The matcher retains operators, literals, property keys, parameter order,
resolved binding identity and control flow. It normalizes identifier spelling
and leading local declarations/initializations while retaining declaration
kinds (including mutable versus const bindings). A source role match or a byte
round trip alone cannot grant an algorithm identifier.
