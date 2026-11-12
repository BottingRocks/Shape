# Joined observable constructor epochs

[epochs.rs](epochs.rs) supports the sibling
[joined_observables.rs](../joined_observables.rs) in the
[observable rules](../README.md). The parent owns the operational join proof;
the helper owns a local constructor-lifetime proof.

Epoch collection enumerates every constructor writer by `SymbolId`. It starts
from an uninitialized plain declaration in a function body and checks ordinary
assignment statements and their maker calls. Loops, deferred writers and
compound assignments do not establish an epoch. Dynamic scope is refused.

Constructor reads remain closed: supported construction, direct static calls
and type/identity checks cannot be replaced with arbitrary publication or
reflective use. The proof separates a complete class maker from evidence that
initialization executed before callable publication.

The parent's `constructor_epochs` adapter shares only that lifetime proof.
A caller must still prove its own entire operator body; no join role transfers
from a sibling. Parent inline tests and
[join tests](../../../../../tests/naming_joined_observables.rs) cover full
contexts; [operator epoch tests](../observable_operator_epochs_tests.rs) exercise
related constructor/exposure boundaries.
