# Accessor hook capture proofs

[captured.rs](captured.rs) supports the sibling
[accessor_hooks.rs](../accessor_hooks.rs) under the
[runtime rules](../README.md). The parent proves the complete interceptor;
this helper independently inspects descriptor and cache helper origins.

The parent follows descriptor lookup, configurable/data-property dispatch,
original accessor retention, getter/setter wrappers, descriptor installation
and registration. Nested wrappers are re-proved as owned bodies; the outer
interceptor is the function claimed by that rule.

The capture helper consumes source and scoped writers, never proposal names or
sibling roles. Its bounded alias walk checks that a hoisted interceptor is not
exposed before capture initialization. A textually later reference inside a
hoisted nested function cannot demonstrate executed setup. Early aliases are
followed only with their own stability and exposure checks.

Apply/factory captures remain opaque call protocols. Keep a proved lookup/cache
body distinct from a claim about the implementation of those external ports.
See [accessor protocol tests](../../../../../tests/naming_accessor_protocols.rs)
and [hook tests](../../../../../tests/naming_posthost_accessor_hooks.rs) for
accepted descriptor forms and refusal controls.
