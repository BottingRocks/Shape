# Browser-related function rules

The [function evaluator layer](../README.md) routes these rules through the
sibling [browser.rs](../browser.rs). They recognize browser-facing structure,
including cases where a captured receiver's implementation remains opaque.

| File | Maintainer starting point |
| --- | --- |
| [url_query_merge.rs](url_query_merge.rs), [url_query_reader.rs](url_query_reader.rs) | Shared owned query parsing, then separate merge/read protocols |
| [persistent_id_migration.rs](persistent_id_migration.rs) | Migration prefix recognition and a separate whole-definition continuation protocol |
| [submit_instrumentation_teardown.rs](submit_instrumentation_teardown.rs) | Ordered listener/registry teardown; includes [guarded cleanup](submit_instrumentation_cleanup.rs) |
| [frame_start.rs](frame_start.rs), [frame_carrier.rs](frame_carrier.rs) | Frame startup and carrier roles |

The query reader imports parsing predicates from the merger, but still proves
its own two-formal body and owned filter/map projections. A captured parser or
similar sibling is not an interchangeable witness.

The migration definition protocol retains captured ports as opaque and
effectful. Its [contract](persistent_id_migration_contract.js) describes ordered
operations, not exported runtime helpers. Keep that role distinct from the
prefix recognizer in the same Rust file.

Submit teardown checks event strings, handler identities and receiver forms.
Its alternate cleanup implementation is included with a path attribute, rather
than declared as a separate capability in `browser.rs`.

See [cleanup tests](submit_instrumentation_cleanup_tests.rs) and
[query merge tests](../../../../tests/naming_url_query_merge.rs). The
[event submit guide](../events/submit_notification/README.md) covers notification
and forward/reset proofs rather than this teardown rule.
