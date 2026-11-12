# Captured handle cleanup tests

This directory holds [tests.rs](tests.rs) for the sibling
[captured_handle_termination.rs](../captured_handle_termination.rs), an
[observable rule](../README.md). The production recognizer remains in that
sibling file.

The claimed body conditionally invokes a captured handle's `terminate()`, then
invokes `complete()` on a separate captured receiver. These are opaque method
protocols: they establish no native worker, graphics or observable identity.

The source proof accounts for captured slot writers and readers. Its cleanup
slot reader uses an exact `typeof slot === "function"` guard before the nullary
call; this permits synchronous reads without assuming callbacks run later.
Lexical ordering is used only between direct sequential statements, with
intervening closures refused by the statement-owner check.

Tests inspect the intended declaration inside its whole surrounding scope,
exercise scoped renaming, and alter guards, receivers, arguments and effect
order. Retain that target-specific testing style so another matching cleanup
cannot mask a refusal. See also the
[integration tests](../../../../../tests/naming_captured_handle_termination.rs).
