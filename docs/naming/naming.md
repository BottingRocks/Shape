# Internal semantic naming APIs

Use [focused function naming](threads.md) for fast source edits. The former
broad annotation and broad rename CLI commands have been removed. They are not
aliases for the focused command and reject immediately, including `--help`.

The general Rust APIs remain available for existing integrations and library
tests. They reconstruct more information and are not the focused execution path:

- `shape::naming::semantic_names::plan_semantic_names(source)` returns the full naming plan,
  including candidates, structural evidence, ambiguities and refusals.
- `shape::naming::program::rename_semantic_names_with_options(source,
  SemanticRenameOptions { anonymous, local_names })` applies that plan.
  `AnonymousNameOptions { name, annotate }` controls optional anonymous-function
  names/comments; `LocalNameOptions::enabled()` or `disabled()` controls local
  naming. These are Rust options, not flags on a remaining CLI command.
- `identify_and_transform_with_local_names(source, IdentifyOptions { rename,
  annotate }, local_options)` is the internal property/comment adapter.
- `transform_program(source, explicit_renames, annotations, options)` is the
  checked source-edit engine. `ProgramBindingRename::new(identifier_start, name)`
  addresses a lexical binding in the exact original UTF-8 source. It preserves
  static property keys, expands shorthands, and refuses capture, collision,
  dynamic scope, unsupported bindings or invalid rewrites. Its validated edit
  ledger supports exact replay.

The focused `threads` implementation reuses standalone function finder packages
and purpose finders on its existing scoped parse. It does **not** call the full
naming plan, property/layout extraction, role graph, annotation or local naming.
Only generated function bindings are rename targets; existing meaningful KEY,
local and human function names are kept.

For reusable labels across builds, the focused report separates independently
recognized function roles from inherited structural labels. Reference creation
requires an unpruned readable source and its proven equal-build original trace.
A source already pruned differently cannot be joined by function order. Across
other builds, literal/API-preserving structural signatures and scoped capture
identity determine matches; original IDs, PCs and source order do not.
