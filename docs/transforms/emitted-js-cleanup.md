# Emitted JavaScript cleanup

The declaration cleanup API in `emitted_cleanup` is an **opt-in, standalone**
pass over JavaScript produced by `emit_javascript`. It parses and analyses the
source with Oxc, resolves every declaration through the semantic `SymbolId`
graph, and removes only dead declarations whose initializer is provably pure
and unable to throw. The same module also supplies the ordered cleanup passes
used by the linked `shape trace program` output.

It is not a minifier and it is not wired into the deobfuscation pipeline. The
symbolic emitter is not modified, emitted code is never executed, and no
captured JavaScript or WASM is run. This page defines the standalone
declaration cleanup and the shared strict array/property contracts. The
linked-program stages are documented in [program cleanup](emitted-program-cleanup.md)
and [helper and tuple cleanup](emitted-helper-cleanup.md). The
[module guide](../../src/transforms/emitted_js/README.md) maps implementation ownership.

For tracing and linking checks, `shape trace program --no-cleanup` skips the
whole-program Oxc passes while retaining the per-function string, property,
array, and receiver stages; see the [program output
contract](../tracing/trace-cli.md#program-output).

```rust
use shape::emitted_cleanup::cleanup;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "function trace(x) { let unused = 1; return x; }";
    let result = cleanup(source)?;
    assert_eq!(result.report.removed_declarators, 1);
    assert!(!result.code.contains("unused"));
    println!("{}", result.code);
    Ok(())
}
```

The source-only API is `cleanup(source)` /
`cleanup_with_options(source, &EmittedCleanupOptions::default())`.
Call `cleanup_emitted_javascript(&emitted)` instead when the input is an
`EmittedJavaScript` with statement and node maps that must be regenerated.

The separate [emitted property-write cleanup](emitted-property-writes.md) pass
(`simplify_property_writes`) rewrites proven-equivalent
`Object.defineProperty` data writes to ordinary assignments. It is independent
of declaration cleanup. Completed `shape trace thread` emission applies this
property pass after the proof-backed emitted-string fold; the declaration
cleanup pass remains opt-in, so explicit `$SLOT_<slot>` bindings stay visible.

Completed `shape trace thread` emission then applies the narrow array
construction fold. It turns a fresh array with `void 0` placeholders followed
immediately by dense indexed assignments into one literal, for example:

```js
let $ARRAY_16765 = [void 0, void 0, void 0, void 0, void 0];
$ARRAY_16765[0] = 0;
$ARRAY_16765[1] = 1;
$ARRAY_16765[2] = 0;
$ARRAY_16765[3] = 130;
$ARRAY_16765[4] = 0;
```

becomes `let $ARRAY_16765 = [0, 1, 0, 130, 0];`. The pass resolves the array
binding through Oxc symbols; variable names do not affect decisions. RHS text
is copied losslessly, including string escapes, surrogate escapes, `-0`, and
parentheses. It accepts only primitive literals, `void 0`, and pure numeric
unary forms. Identifier RHS expressions—including SSA, SLOT, local, and
parameter bindings—are intentionally retained because slots are mutable
program state and this pass never propagates source values. The array must be
non-empty and every consumed write must target an existing own dense writable
`void 0` element. Empty arrays, holes, spreads, duplicate or non-dense keys,
partial writes, effectful RHS expressions, self-reference, TDZ or uncertain
local initialization, and dynamic scope are retained. Direct-body reads,
calls, or escapes before construction completes, and nested-function captures,
are also retained; a post-completion escape such as `return g(a)` is safe and
supported. Only the outer emitted function body is considered; nested function
payloads and control-flow blocks are retained.

The outcome's `considered` count is the number of declarators in direct-body
variable declarations scanned, including non-candidates. `skipped` is a
small, non-exhaustive list containing only recognized candidates rejected by
the receiver proof, with the offending reference span and reason. Unsupported
shapes, dynamic scope, and declarations outside the primary body may remain
unlisted; callers must not infer `considered == compacted_arrays + skipped.len()`.

Because assignment statements are removed, the array result reports
`StatementMapStatus::Regenerated`. Original trace statement and node IDs stay
unchanged; entries for consumed writes are remapped to the folded declaration,
and later entries receive their new output statement indexes. The CLI reports
the compacted-array and removed-write counts on stderr while keeping stdout
JavaScript-only.

## What is removed

A declaration is removed only when **all** of these hold:

1. It is a `var`, `let`, or `const` declaration that is a direct statement of a
   function body (including function expressions and arrow-function blocks).
2. Every declarator uses a plain binding identifier. Destructuring patterns are
   retained.
3. The binding has no resolved references outside an initializer that is also
   proven removable: reads, writes, closure captures, `typeof`,
   assignment-pattern targets, and exports otherwise retain it. Removability
   is solved to a fixed point so removing an unused closure can expose another
   dead binding without requiring a second cleanup run. A binding with a
   formal-parameter role is retained even when the graph reports no such
   reference: sloppy-mode `arguments[index]` aliasing is not represented by
   Oxc's symbol references, and parameter-redeclaring `var` initializers can
   therefore be observable. This conservative rule also covers destructured,
   default, rest, duplicate, and arrow-function parameters, plus collisions
   with function declarations.
4. The initializer is absent or is in the conservative pure subset below.

Whole dead declarations are replaced by an empty statement (`;`). A declaration
with a mix of dead and live declarators keeps its keyword, semicolon, and live
declarators, for example `let dead = 1, kept = use();` becomes
`let kept = use();`.

Names are never inspected for decisions. A binding called `_0xdead` is retained
when it is used or effectful, and a binding called `$` is removed when it is dead
and pure.

## Conservative purity

Accepted initializers cannot call user code and cannot throw:

- boolean, null, numeric, bigint, string, and regexp literals;
- template literals with no substitutions;
- `new.target` (but not `this`, which can throw before `super()` in a derived
  constructor);
- array literals with no spread and only pure elements/elisions;
- object literals with no spread or computed keys and only pure property values;
- function and arrow expressions (creating a closure executes no body);
- conditional, logical, and sequence expressions whose operands are all pure;
- `!`, `void`, and `typeof` of a pure expression;
- arithmetic, comparison, and bitwise operations whose operands are literal
  primitives (BigInt mixing is retained because it throws);
- parenthesized forms of the above.

Everything else is retained: calls, `new`, member reads (getters/proxies),
optional chains, assignments, updates, spreads, template substitutions,
`await`/`yield`, tagged templates, class expressions, `in`, `instanceof`,
`delete`, identifiers (locals, globals, TDZ reads, unresolved reads, coercion
hooks), and anything not listed above.

## Preserved behaviour

- Calls, getters, property writes, throws, coercions, and unresolved reads stay
  in their original position and order.
- Effectful unused initializers are retained rather than hoisted or moved.
- Directive prologues are untouched.
- Branch bodies, nested blocks, loop heads, `try`/`catch`/`finally`, `switch`,
  and destructuring declarations are retained; only direct function-body
  statements are candidates.
- Binding identity comes from Oxc scoping, so shadowed bindings in closures and
  nested functions are kept or removed independently.
- Direct-body `var` bindings that share an Oxc parameter-role symbol are kept;
  this protects sloppy parameter/`arguments` aliasing without attempting to
  model `arguments` itself.
- `let`/`const` TDZ and self-reference are retained because identifier reads are
  not considered pure.
- Any `with` statement, or any identifier reference named `eval` (direct,
  indirect, or merely read), disables the pass for the entire input.

## Report and coordinates

`EmittedCleanupReport` records:

- `status`: `Found`, `Unknown`, or `Unsupported` (dynamic scope);
- `eligible_declarations`, `removed_statements`, `removed_declarators`, and
  `retained_declarations`;
- `removed`: original input byte ranges, whether the whole statement was
  removed, and report-only binding names;
- `removed_top_level_function_statements`: zero-based indexes removed from the
  first top-level function declaration body;
- `statement_map_status`;
- input/output byte lengths and notes.

`RemovedRange::span` always refers to the **input** text. The output is a set of
exact source edits, so bytes after the first removal have different offsets;
those new offsets are not reported as old coordinates. Removed statements become
`;` rather than empty text so adjacent tokens cannot be joined across the
deletion.

## Statement maps

`EmittedStatementMap` and `EmittedJavaScript::node_map` are index-based. If a
whole declaration is removed from the primary emitted function body, it is
replaced by an empty statement (`;`), so later parsed output statement indexes
do not shift. The standalone report still marks the old map `Invalidated`
because the removed entry no longer describes meaningful emitted code.

`cleanup_emitted_javascript` regenerates both maps from the exact removed
indexes and reports `Regenerated`: entries that pointed at a removed statement
are dropped, while surviving entries retain their actual parsed output indexes.
Declarator-only removals, and removals in nested functions, leave the top-level
statement indexes unchanged and report `Preserved`.

## Deliberate limitations

- Program-level `var` and lexical declarations are retained. Top-level `var`
  bindings are global properties and top-level lexical bindings are visible to
  later scripts, so removing them is observable even with no in-source
  references. Emitted code is function-scoped.
- No inlining, constant folding, renaming, dead-code elimination of effectful
  statements, or codegen/minification. Formatting outside removed ranges is
  unchanged.
- No cleanup of nested blocks, loop declarations, destructuring, class static
  blocks, or class field initializers.
- A parse failure or an invalid splice returns `EmittedCleanupError`; the pass
  fails closed rather than returning partial output.

## Inspecting the program stages

To see what the per-function and whole-program stages remove from one trace,
use the
program command's stage dump. It writes the final JavaScript to `--out` (or
stdout when it is omitted) and intermediate JavaScript files to `--dump-stages`:

```sh
./target/debug/shape trace program tests/fixtures/kernels/javascript/kernel.raw.js \
  --out /tmp/shape-program.js --dump-stages /tmp/shape-program-stages
```

Each function's stages are in `functions/<function>/pipeline/` (`01-…13b-`);
compare one function's `00-emitted.js` with its `pipeline/13b-tidy-expressions.js`,
and `00-linked.js` with `13e-dotted-properties.js`, before judging a pass from
the final file alone. The numeric prefixes reflect the executed pass order;
the [pipeline table](emitted-program-cleanup.md#pipeline) maps each dump to its
pass. `06-readable-property-writes.js` is the standard-builtins readability
rewrite (the strict rewrite does not run on program output, where it changed
nothing); its assumptions are documented in
[property writes](emitted-property-writes.md#linked-program-output). Carry,
SSA, tuple, helper, and loop passes run later in each function;
`11g-loop-catch-names.js` renames `for` counters to `i`/`j`/`k` and `catch`
parameters to `e`, and `11h-host-globals.js` rewrites global reads from
`host["x"]` to the bare global `x`. The whole-program stages then rebuild
classes and arrows, name bindings, and run the late tidies, which assume
unchanging built-ins (see
[late whole-program passes](emitted-program-cleanup.md#late-whole-program-passes)).
A change in
line count is a useful size observation, not proof that a rewrite is correct;
the source-mapped trace and the relevant regression test establish that.

The stage order makes a dependency visible. Adjacent SSA cleanup can replace
`let $SSA_1 = expression; return $SSA_1;` with `return expression;` only when
the binding has one resolved use and moving the expression does not change
evaluation. Literal SSA cleanup can then replace a proven primitive constant
at its uses. A two-arm carry join can become a conditional value when both
arms assign the same local and no intervening effect observes the assignment.
Removing those relays often exposes another alias or dead declaration, which
is why some passes repeat for a bounded number of rounds. The VM helper and
tuple stages run after that simplification so a small wrapper can be inlined
without dragging obsolete temporaries into the caller. The whole-program
renames shorten generated identities; they do not establish a new semantic
proof.

If a stage barely changes one program, inspect its *candidate count and
decline reasons* before assuming the pass is broken. An SSA read captured by a
closure, a carry used after a loop backedge, or an array observed before its
last write can all block a rewrite while the output remains correct. Compare
the relevant consecutive stage files and the focused tests in
[`src/transforms/emitted_js/`](../../src/transforms/emitted_js/) to see the exact admitted
shape. The default program stages must not be confused with the standalone
dead-declaration API above, which is narrower and retains effects.
