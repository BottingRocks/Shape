# Physical cursor proofs

The saved signal schema describes writes in wire order. A source position proof
checks those writes against the JavaScript's resolved index bindings. It does
not infer an index from a decoded value or from a signal's name.

The four-byte prefix occupies indices 0 through 3. A byte must address the next
index. A certified number or string writer takes the current last index and
returns the last index it wrote. Its encoded length can vary; the position proof
keeps that return symbolic rather than assigning a guessed size.

For example, the existing writer and indexed-store ingredients prove this path:

```javascript
let cursor = 3;
let next = numberWriter(mask, value, buffer, cursor) + 1;
buffer[next] = first ^ mask();
buffer[next + 1] = second ^ mask();
```

The assignment to `next` completes after the writer returns. Both later stores
use that same value version. The proof retains fixed offsets relative to the
physical cursor as bytes append. An intervening variable-length writer makes an
old relative position unknown unless a returned cursor or alias establishes the
new position.

Prefix and postfix increments have different effects. `buffer[++cursor]` starts
with the current last index, while `buffer[cursor++]` starts with the next index.
Constant additions and `cursor += constant` retain their source arithmetic.
Gaps, overwrites, unknown arithmetic, and incompatible branch results refuse the
program.

For a represented conditional, each arm is checked separately. Only cursor
assignments in that arm's own function and unguarded tail are replayed. A
conditional expression's return value is saved before its enclosing assignment
completes. The join keeps a cursor value only when every arm gives it the same
relation to that arm's physical endpoint. Packed flush arms may therefore append
a byte while their sibling arms append none. Existing source count and selector
certificates remain required for dynamic loops and flush decisions.

The final mutation checks cover the live ranges of the cursor value versions
actually read. Reusing a holder for unrelated data after its last cursor read
does not extend that old value's lifetime. Writes inside a live range,
conditional clobbers, and writes through captured cursor aliases still require
proof. Resource lifetime and draw accounting remain separate checks: accepting
a cursor does not authorize an unmatched stream draw or a guessed skip.

`tests/signal_wire_position.rs` parses generic JavaScript, obtains the analyzer's
actual schema JSON, reloads it with the public consumer, and checks independent
golden bytes with the same seed. It covers writer-return aliases, conditional
results, fixed offsets, increments, holder reuse, nested reverse loops, packed
flushes, saved-evidence tampering, and negative gap/overwrite/escape/draw cases.
No JavaScript execution is required.
