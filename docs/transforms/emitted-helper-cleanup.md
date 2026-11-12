# Emitted helper and tuple cleanup

Emitted functions often repeat wrappers and carry several results in temporary
arrays. These passes shorten those forms while preserving call order, receiver
binding, and result identity under the individual contracts below. The
[program cleanup reference](emitted-program-cleanup.md#pipeline) owns stage
order and dump filenames; the [module guide](../../src/transforms/emitted_js/README.md)
owns the file map and checks.

Helpers can become simpler in successive stages: body simplification exposes
a tuple, wrapper inlining exposes scalar copies, and later cleanup removes
those copies. The driver uses bounded repeats; one successful rewrite does not
authorize more general inlining. In particular, fresh tuple unpacking assumes
the standard Array iterator and keeps the allocation.

## VM subroutine parameter names (`vm_sub_parameters.rs`)

Matches function declarations whose name is exactly `vm_sub_<digits>_<digits>`
and whose parameters are simple identifiers beginning with `vm_sub_`. It
renames them positionally `$PARAMA`, `$PARAMB`, ... `$PARAMZ`, `$PARAMAA`, ... and
rewrites every resolved reference to those symbols. The function name and other
functions are untouched. A textual collision check adds `_2`, `_3`, ... to a
candidate name; a function containing `eval`, `with (`, or `with(` is skipped
for this pass. The guard is a substring test, not a syntax proof.

**Reduces:** long generated parameter names repeated at every read. No calls,
statements, or allocations change.

## VM helper sharing (`vm_sub_dedupe.rs`)

Finds two or more `function vm_sub_*` declarations that are direct statements
under the same program root and have identical canonical bodies. Local bindings
are renamed `$LOCAL0...` in source order for comparison. Every reference inside
a candidate must resolve to a declaration inside the helper; unresolved names
are treated as the same global from any nested scope under that root. Bodies
containing `eval`, `with`, `this`, `super`, `new.target`, `arguments`, comments,
backticks, nested functions, or shorthand properties decline. All references to
a helper must be direct call callees, with no captures.

One member is renamed to a fresh `$VMHELPER_<n>`, inserted after the root body's
opening brace, and every call callee is renamed. The original declarations are
deleted. The rewrite must strictly reduce output size and must reparse with
every `$VMHELPER_` reference still resolving.

**Reduces:** duplicate helper bodies and lines. Calls and allocations are
unchanged.

## VM body simplification (`vm_sub_simplify.rs`)

Rewrites only linear `vm_sub_*` bodies:

- a placeholder-array builder is delegated to the fresh-array pass;
- `let $ARRAY = [lits...]; let $SSA = callee(args..., $ARRAY); return [$SSA,
  $SSA];` can move the literal array into the call and delete the temporary;
- a single `let $X = INIT;` read only by a return or tuple return can substitute
  `INIT` at that one read.

The binding must have exactly one resolved reference and no write. Bodies must
have exactly the expected statement count and an empty directive prologue.
Comments in a rewritten declaration line decline. Substitution copies source
text and occurs at the first observable evaluation point, so an effect inside
`INIT` still happens once at the same time.

**Reduces:** helper temporaries and exposes tuple/call shapes for later
inlining.

## VM wrapper inlining (`inline_vm_sub.rs`)

Matches a small `vm_sub_*`/`$VMHELPER_*` function with one `return` and no
directives/rest. The return expression may use parameter reads, literals,
parentheses, binary/unary (not `delete`), member reads, arrays of those atoms,
`Number(x)` on an unresolved global, and calls with atom callees/arguments.
Every parameter must be read. Calls must use an identifier callee with a
resolved symbol and non-spread arguments.

Each parameter read is replaced by the call argument's source slice; a member
result is parenthesized or emitted as `(0, expr)` so a following call does not
gain a method receiver. Arguments must be literals or resolved generated
identifiers (`$SSA_`, `$KEY_`, `$CARRY_`, `$PARAM`, `$OBJECT_`). If reads are out
of order, an array return has member elements, or the callee read can have
effects, the pass requires stable `$SSA_` arguments with no writes. `eval`/`with`
anywhere disables it, as does a write to the helper or an extra reference.
A one-parameter wrapper can also accept a computed argument when the parameter
is read exactly once without an earlier observable operation.
Whole-helper inlining must shrink output; array-returning helpers may grow only
when the call is registered as scalarizable. The driver repeats up to four
times.

**Reduces:** wrapper functions, call frames, and call-site text.

## Tuple scalarization (`tuple_scalarize.rs`)

Matches `let $SSA_t = [first, second, ...];` with 2-4 dense elements, immediately
followed by exactly one `let $SSA_i = $SSA_t[i];` per element, in order, with at
least one statement after the run. Element 0 may also be a static/computed
member read; later elements must be identifiers or primitive literals. The
tuple's resolved references must be exactly those reads, write-free and equal to
the read objects, so the array cannot escape or alias.

The run is replaced by scalar `let` declarations at the same indentation, using
verbatim value text. This keeps `-0`, escapes, and lone surrogates intact. Only
element 0 may perform a property read, so no observable operation is separated
from the later copies. `eval`/`with` disables the pass.

**Reduces:** a temporary array, its indexed reads, and the statements carrying
them; it also exposes scalar SSA copies to the adjacent/alias passes.

## Fresh tuple unpacking (`tuple_unpack.rs`)

Matches a local immutable helper whose last statement returns a dense array
literal and whose earlier statements are only declarations or expression
statements. The caller must bind the result once, then read 2-7 elements in
order with immediate `$SSA_t[i]` reads. The tuple references must be exactly
those reads, write-free and equal to the read objects; every read name must be
`$SSA_`; comments inside the run decline.

The whole run becomes one destructuring declaration:

```js
let [$SSA_a, $SSA_b] = helper(args);
```

This assumes the standard Array iterator. It reduces N+1 statements to one but
does **not** remove the helper's array allocation or the call.

**Reduces:** the tuple binding and N indexed reads to one destructuring
statement; the helper's array and call remain.

## Short generated names (`short_generated_names.rs`)

The first whole-program stage (`12`) renames identifiers of the form
`PREFIX<function-identity>_<serial>` to `PREFIX<serial>_<number>` for
`$SSA_`, `$CARRY_`, `$MERGE_`, `$ARRAY_`, and `$OBJECT_`. Every binding and
reference with that exact name is replaced identically, so binding identity and
shadowing are preserved without symbol lookup. The number comes from the
linker's sorted function order, making it deterministic under parallel tracing.
If any replacement already occurs as an identifier in the program, the pass
fails instead of risking a collision. Strings, comments, and non-matching names
are untouched.

**Reduces:** the dominant per-function identity suffix at every read. It changes
bytes and readability, not statements, calls, or allocations.
