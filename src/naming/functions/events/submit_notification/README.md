# Submit notification support

The sibling [submit_notification.rs](../submit_notification.rs) is the entry
point under the [event rules](../README.md). It proves the notifier and each
forward/reset body independently, linking them by resolved `SymbolId`.

| File | Responsibility |
| --- | --- |
| [parent.rs](parent.rs) | Bounded parent producer and branch-wiring proof for a direct no-event call |
| [family_tests.rs](family_tests.rs) | Whole-context controls and protocol mutations |

The parent evaluator checks published callback slots: an earlier uninitialized
plain lexical declaration, one non-null publication after the function in its
own block, and only the permitted null resets. Reads before publication outside
the function cannot establish the slot's identity.

`parent.rs` supplies a narrow exception by proving the complete local receiver
producer and branch wiring. It checks event/original-target selection and the
parent walk rather than assuming a similarly named receiver is the form.
Captured notification receivers remain opaque lexical data; neither this proof
nor a forwarder establishes native browser origin or installation identity.

Start with the test file when extending a source form: its controls exercise
both notifier linkage and full surrounding state, so a sibling match cannot
hide refusal of the intended declaration.
