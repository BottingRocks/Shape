# Renaming generated bindings after the program pipeline

This is a hand-off plan for a final naming pass over emitted program JavaScript.
It is not implemented yet. Every anchor below is a file and line in the current
tree.

## 1. Why the renamer runs last

The linked program pipeline already shortens generated identities, but it stops
one step short of a name a reader can use. `short_generated_names`
(`src/transforms/emitted_js/names/short_generated_names.rs`, stage
`12-short-generated-names.js`) turns `$SSA_2_46953_entry_9965` into
`$SSA_9965_0`: it keeps the emitter's binding serial and appends the linker's
function number. That is stable and collision-free, but both numbers are
identity bookkeeping. `$SSA_9965_0` means "the 9965th generated value, in
function 0" — nothing about where it is or what it holds.

A renamer that runs *after* cleanup can replace that bookkeeping with small,
local names, for two reasons:

- every earlier pass recognizes generated bindings by these prefixes, so the
  names must survive until the last cleanup step; and
- renaming a binding that a later pass deletes wastes the name and makes the
  stage diff misleading.

So the renamer is the final step. It reads the finished program and assigns the
names; it removes no statements and proves no new semantic fact.

## 2. What is generated today

| Prefix | Produced by | Example | After stage 12 |
| --- | --- | --- | --- |
| `$SSA_`, `$CARRY_`, `$MERGE_`, `$ARRAY_`, `$OBJECT_` | `preferred_generated_name` and `generated_value_name` (`src/tracing/symbolic/emit.rs` 1912-1949) | `$SSA_1201_0`, `$CARRY_50_0` | `$SSA_1201_0` -> `$SSA_1201_0` (already `<serial>_<function>`) |
| `$THREAD_` | `preferred_generated_name` (`emit.rs` 1926); nested by `src/trace_worker/program_nest.rs` | `$THREAD_0_40005` | unchanged: `$THREAD_` is not in the stage-12 prefix list |
| `$KEY_<slot>` | `program_key` (`emit.rs` 1865) | `$KEY_0`, `$KEY_397` | unchanged |
| `$SLOT_<slot>` | `memory_slot_name` (`emit.rs` 1906) | `$SLOT_3` | unchanged |
| `$PARAMA..Z` | `parameter_name` (`src/transforms/emitted_js/helpers/vm_sub_parameters.rs` 60-69) | `$PARAMA` | already per-function letters |
| `$THREAD_SELF`, `$THIS` | constants `PROGRAM_SELF` / `PROGRAM_THIS` (`emit.rs` 556, 561) | `$THREAD_SELF` | not bindings; never rename |

The stage-12 prefix list is `src/transforms/emitted_js/names/short_generated_names.rs`
line 20: `$SSA_`, `$CARRY_`, `$MERGE_`, `$ARRAY_`, `$OBJECT_`. `$KEY_`,
`$SLOT_`, and `$THREAD_` are not shortened today.

Residual occurrence counts on the AAAgxmeh seed (this repository's reference
trace), after the full pipeline: `$SSA_` 16,258, `$KEY_` 7,283, `$CARRY_`
5,624, `$ARRAY_` 1,396, `$THREAD_` 1,267, `$OBJECT_` 384. `$SLOT_`, `$MERGE_`,
and `$PARAM` count zero after the per-thread cleanup the pipeline runs.

## 3. The chosen scheme

Per prefix, within each function, in declaration order: `$SSA_1`, `$SSA_2`, …,
and likewise `$KEY_1`, `$CARRY_1`, `$ARRAY_1`, `$OBJECT_1`, `$THREAD_1`. The
counter restarts in each function. Two sibling functions both use `$SSA_1` and
`$SSA_2`; that is the point — the numbers are small and local rather than the
emitter's serial plus a global function number.

Digits, not letters: `vm_sub_parameters.rs` already uses `$PARAMA..Z` for one
specific class of parameter, and a digits scheme keeps a distinct shape for
"linked temporary" while not running out at 26.

Before and after, one function:

```js
function $THREAD_0_40005() {
  let $SSA_1201_0, $CARRY_50_0, $KEY_0, $OBJECT_7_0;
  $OBJECT_7_0 = { mode: "x" };
  $SSA_1201_0 = $KEY_0["x"];
  $CARRY_50_0 = $SSA_1201_0 + 1;
}
```

```js
function $THREAD_1() {
  let $SSA_1, $CARRY_1, $KEY_1, $OBJECT_1;
  $OBJECT_1 = { mode: "x" };
  $SSA_1 = $KEY_1["x"];
  $CARRY_1 = $SSA_1 + 1;
}
```

The cost of the scheme, stated so it is a decision and not a surprise: the
emitter serial and the linker function number disappear. The old name was
stable across a re-trace and located a binding globally; the new one is only
meaningful inside its function. A reader who needs the old identity should keep
`--dump-stages` output, not the final name.

## 4. The rule that makes it sound

`short_generated_names` sidesteps name resolution by making every replacement
globally unique and failing when it is not (`short_generated_names.rs` 40-46).
The new scheme deliberately reuses names in sibling functions, so a textual
rename is unsound: a replacement string that already occurs anywhere makes the
stage-12-style check fail, and reusing `$SSA_1` while another scope holds a
`$SSA_1` can silently capture a reference.

The renamer must rename by symbol. For a binding declared in function `F`,
`$PREFIX<n>` is allowed only when no name visible at any use of that binding is
already `$PREFIX<n>`. Visible at a use means:

- a binding declared in `F` or in any enclosing function (a capture);
- a free reference written inside `F` (a global); or
- a name already assigned to an enclosing binding, new or pre-existing,
  because that binding is visible in `F`.

Process functions outermost first and reserve each assigned name immediately, so
an inner function cannot choose a name an outer binding already holds. When a
candidate is taken, advance `n`.

The two concrete failures this prevents:

- an inner `$SSA_1` shadowing an outer `$SSA_1` that a closure reads — after the
  rename the read would resolve to the inner binding and the program would read
  the wrong value;
- a free `$SSA_1` reference captured by a newly named local `$SSA_1`.

`name_loop_and_catch_bindings.rs` (the stage-`11g` pass added with this plan's
sibling work) is the working precedent: it renames by `scoping.symbol_span`
plus `scoping.get_resolved_references`, restricts itself to names starting with
`$`, and declines under `with`/`eval`. The renamer is the same shape with a
different name table and a capture-aware allocation.

## 5. Algorithm

1. Parse with `analysis::parse_program` and analyze with `analysis::analyze`.
2. Collect every binding whose name starts with a generated prefix, in
   declaration order, with its declaring function and its prefix. A `Collect`
   visitor mirroring `name_loop_and_catch_bindings.rs` gives the function spans;
   the declaring function of a symbol is the smallest function span containing
   its declaration span.
3. Order functions outermost first (a span that contains another sorts first).
4. For each function, per prefix, assign the next serial to that prefix's
   bindings in declaration order, skipping any serial whose `$PREFIX<n>` is
   visible under the rule in section 4. Record the assigned name as reserved for
   the function's subtree.
5. Emit edits: the declaration span (`scoping.symbol_span`) and every resolved
   reference span (`scoping.get_resolved_references` then the reference node's
   span) become the new name. Skip a binding whose new name equals the old one.
6. Apply with `analysis::apply_edits`; fail on any rejected or overlapping edit;
   reparse the output to verify it is valid.
7. Decline the whole input on `with`/`eval` (scope is not statically known).
   Never rename a name the pass cannot prove is a binding.

Determinism: iterate declaration order, never a hash map, so parallel trace
scheduling cannot change the output.

The result type matches the surrounding passes:

```rust
pub struct RenameResult {
    pub code: String,
    pub renamed: usize,
}

pub fn rename_generated_bindings(source: &str) -> Result<RenameResult, String>;
```

## 6. Where it runs

### 6.1 The linked stage (`shape trace program`)

Add a stage after `13-unwrap-host-globals.js` in
`src/trace_worker/commands/program.rs`, at the tail of the linked closure
(the stage list ends there today):

```
14-rename-generated-bindings.js
```

(Stage `13` is the host-global unwrap, which landed after this plan was
written; run the renamer after it so its capture check sees the bare global
names that unwrap introduces.)

The closure returns `(code, counts…)`; the renamer's result is
`RenameResult { code, renamed }`, and `renamed` can join the existing summary or
be left unsigned. No report schema changes; only `--dump-stages` gains a file,
and the run summary's shape stays as it is.

Keep stage 12 or replace it:

- keep (recommended): the renamer matches by prefix, so it accepts both
  `$SSA_9965_0` (stage 12 output) and `$THREAD_0_40005` (untouched); the
  existing stage-12 regression tests keep passing unchanged;
- replace: feed the renamer the raw `<serial>_<function identity>` names and
  drop stage 12, for one fewer parse of a program this size.

### 6.2 The standalone command (`shape rename`)

`shape rename` renames one already-emitted JavaScript file. It does not clean.
The dispatcher is `shape.rs`:

- add a `RENAME_USAGE` constant beside `DEOB_USAGE` (`shape.rs` 53-57);
- add a `"rename" => run_rename(args.collect())` arm to the `match command`
  table (`shape.rs` 101-114) and a usage line to `TOP_USAGE` (29-51);
- `run_rename` mirrors `run_deob` (117-186): read with `cli::read_source`
  (`cli/mod.rs` 27), call `shape::emitted_cleanup::rename_generated_bindings`,
  then `cli::write_stdout` (`cli/mod.rs` 47) when no output path is given or
  `write_source_atomically` (`shape.rs` 316) with `--force` otherwise;
- `parse_rename` mirrors `parse_deob` (`shape.rs` 383-455).

The call shape:

```
shape rename <source> [-o|--output <output>] [--force] [-v|--verbose]
```

Stdout is the default when `-o` is omitted, matching `shape deob`; `--force`
replaces an existing output file; `-v` prints `renamed N bindings` on stderr.
This is the same output-path convention the rest of the CLI uses, so a user can
pipe `shape rename` exactly like `shape deob`.

The public API entry point, defined in a new
`src/transforms/emitted_js/rename_generated_bindings.rs`, exported from
`src/transforms/emitted_js/mod.rs` beside
`name_loop_and_catch_bindings` (mod.rs 115-117), and reachable through the
existing `shape::emitted_cleanup` alias (`src/lib.rs`):

```rust
shape::emitted_cleanup::rename_generated_bindings(source: &str)
    -> Result<RenameResult, String>
```

## 7. Interaction with the existing passes

- Keep the prefix matchers working: `adjacent_ssa.rs` 526-527, `alias_ssa.rs`
  136/417, `branch_value.rs` 227, `carry_coalesce.rs` 71-72, `carry_join.rs` 86,
  `carry_self_assign.rs` 102, and `short_generated_names.rs` 20 all recognize
  the old names. They run before the renamer, so they are unaffected. Add no
  pass after the renamer that depends on these names.
- `name_loop_and_catch_bindings` (stage 11g) already renamed some `$CARRY_` and
  `$SSA_` bindings to `i`/`e`. The renamer only touches `$`-prefixed names, so
  the two cannot fight over a binding.
- `vm_sub_parameters` names its own `$PARAMA..Z` and its tests assert those
  names. Either leave them (recommended: already readable, and the tests stay
  valid) or add `$PARAM` to the prefix list and renumber them too.
- `$KEY_<slot>` loses its slot number. That is safe: the slot number is an
  emitter-side identity built by `program_key` (`emit.rs` 1865), nothing
  downstream reads it, and the same slot in another function is a different
  variable — which the per-function numbering now makes explicit.
- `$THREAD_<state>_<pc>` loses the state and program counter it was named for.
  That information was only ever visible in the emitted text and is never read
  by a later pass.

## 8. Tests

Unit, in `rename_generated_bindings.rs`:

- per-function restart: two sibling functions, each with two `$SSA_` bindings,
  both become `$SSA_1`, `$SSA_2`;
- capture: an inner function that declares a temporary and reads an outer
  temporary leaves the outer `$SSA_1` and gives the inner `$SSA_2`;
- free-name guard: an unresolved `$SSA_1` reference forces the local to
  `$SSA_2`;
- untouched: an `i` loop counter from stage 11g, `$THREAD_SELF`, hand-written
  names, string contents, and comments;
- `$KEY_` and `$THREAD_` renumbering;
- `with`/`eval` decline.

Integration:

- extend `tests/program_generated_names.rs` (65-80) with a case that pushes
  emitted output through the renamer and asserts no `<serial>_<function>`
  shape remains and that two sibling functions share `$SSA_1`;
- add `tests/rename_generated_bindings.rs` for the standalone API;
- add a `tests/cli.rs` case: `shape rename` writes to stdout by default and
  honours `--force`;
- behavioural check: run the renamed program and the pre-rename program through
  the differential harness in `tests/emitted_js_cleanup.rs` and compare results.
  Renaming must not change behavior; this is the check that the capture rule
  holds on real output.

## 9. Files to touch

- new `src/transforms/emitted_js/rename_generated_bindings.rs`
- `src/transforms/emitted_js/mod.rs` (module declaration and `pub use`)
- `src/trace_worker/commands/program.rs` (stage `14`, after stage `13`)
- `shape.rs` (`RENAME_USAGE`, the dispatch arm at 101-114, `run_rename`,
  `parse_rename`, `TOP_USAGE`)
- the tests above
- docs: the stage table in `docs/emitted-program-cleanup.md` (122-159) and its
  pass catalog (488-517), `docs/emitted-js-cleanup.md` (211-234),
  `docs/trace-cli.md` (174-179), `src/transforms/emitted_js/README.md` (15-34
  and 55-72), and this file

## 10. Open decisions

- Keep stage 12 or replace it (recommended: keep).
- Fold `$PARAM` in or leave it (recommended: leave).
- A top-level `shape rename` or `shape trace rename` (recommended: top-level,
  next to `deob`, because the pass works on any emitted file and needs no trace).
