# Reflection and accessor protocols

JavaScript source wrappers isolate proxy prototype probing, function traits, and correlated accessor-registry lookup.

[`proxy.js`](proxy.js) relates a `getPrototypeOf` trap, marker, and returned object. [`function-shape.js`](function-shape.js) collects callability, source, prototype, and deletion traits into fields `a` through `d`. [`accessor.js`](accessor.js) preserves the selected registry record and original getter relationship.

Consumed by:

- [naming_function_and_proxy_protocols.rs](../../../naming_function_and_proxy_protocols.rs)
- [naming_accessor_protocols.rs](../../../naming_accessor_protocols.rs)

These are positive structural witnesses. Negatives alter the trap/object identity, marker value, receiver, prototype key, deletion effects, cached-record alias, or getter origin; alpha variants preserve scoped identities.

Keep branch correlation and ordered field writes intact. Apparent `main()` calls are fixture source text for parsing, not instructions to execute browser probes or interceptors.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../README.md) and the [program fixture overview](../README.md).
