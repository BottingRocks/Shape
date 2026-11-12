# Method interception scope

[full_scope.js](full_scope.js) is a JavaScript scope retaining the method installer,
descriptor lookup, original-value caches, wrapper publication and failure-report
transport needed to analyze method interception as a complete protocol.

[The integration target](../../naming_method_interceptor_family.rs) derives
single-cache, boolean-cache, copied-descriptor and helper-backed forms in memory.
It checks owned_method_interceptor_lowered_external_helpers claims while varying
cache reset paths, frozen/writable descriptor tests and failure payload layouts.
[Transport unit tests](../../../src/naming/functions/runtime/function_hooks_transport_tests.rs)
consume the same full scope.

Positive renamed and equivalent layouts retain receiver and registry identity.
Negative controls change helper origins, publication order, cursor lifetimes,
payload escape or exposed property order; matching property names alone are
insufficient evidence for an installer.

Keep the complete scope, statement order and exact mutation anchors unchanged.
This recovered source is static test data, not a script to install browser hooks.
Read next: [runtime naming rules](../../../src/naming/functions/runtime/README.md)
and [the fixture collection](../).
