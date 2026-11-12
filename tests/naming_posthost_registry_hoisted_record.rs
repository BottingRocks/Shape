use shape::naming::name_threads;

// Focused post-host family: the serializer extension registry whose extend
// method hoists the { detector, serialiser, deserialiser } record into a
// single-origin local instead of writing it inline. The serializer package
// runs only when a complete encoding tag table is present; this portable
// header supplies that unchanged context so the registry proof is reached.

const HEADER: &str = r#"function host(){
let encodingTags = {
    UINT14_BASE: 1,
    UINT16: 2,
    UINT24: 3,
    UINT32: 4,
    UINT64: 5,
    NINT4_BASE: 6,
    FLOAT32: 7,
    DOUBLE64: 8,
    STR5_BASE: 9,
    STRREF: 10,
    STRLUT: 11,
    ARRAY8: 12,
    BARRAY8: 13,
    MAP_: 14,
    BMAP_: 15,
    EXTENSION: 16
};
"#;

const FOOTER: &str = "\n}\nhost();";

fn program(body: &str) -> String {
    format!("{HEADER}{body}{FOOTER}")
}

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.outcomes.iter().any(|outcome| {
            outcome.applied
                && outcome.binding.name == target
                && outcome.proposed_name.as_deref() == Some(name)
        })
    })
}

fn named_registry(source: &str, factory: &str, constructor: &str, extend: &str) {
    for (target, name) in [
        (factory, "createExtensionRegistryClass"),
        (constructor, "ExtensionRegistry"),
        (extend, "registerExtension"),
    ] {
        assert!(named(source, target, name), "did not name {target} as {name}:\n{source}");
    }
}

/// A negative source must still parse and analyze; only the naming proof is
/// expected to fail. This keeps refusals from passing vacuously.
fn refused(source: &str, target: &str, name: &str) {
    let result = name_threads(source, true)
        .unwrap_or_else(|error| panic!("negative source must parse and analyze: {error}\n{source}"));
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes:\n{source}"));
    assert!(
        !(outcome.applied && outcome.proposed_name.as_deref() == Some(name)),
        "accepted {target} as {name}:\n{source}"
    );
}

fn refused_registry(source: &str, factory: &str, constructor: &str, extend: &str) {
    for (target, name) in [
        (factory, "createExtensionRegistryClass"),
        (constructor, "ExtensionRegistry"),
        (extend, "registerExtension"),
    ] {
        refused(source, target, name);
    }
}

// The already-applied inline-record base: the factory, the returned
// this-instanceof-guarded constructor and the unique extend writer must keep
// their three names.
const INLINE_BASE: &str = r#"
function $THREAD_10_1() {
    let $KEY_0;
    function $THREAD_11_2() {
        "use strict";
        if (this instanceof $KEY_0) {
            this.extensions = Object.create(null);
            return void 0;
        } else {
            throw new TypeError("Cannot call a class as a function");
        }
    }
    $KEY_0 = $THREAD_11_2;
    let $KEY_3 = { key: "extend" };
    function $THREAD_12_3($CA, $CB, $CC, $CD) {
        this.extensions[$CA] = {
            detector: $CB,
            serialiser: $CC,
            deserialiser: $CD
        };
    }
    $KEY_3.value = $THREAD_12_3;
    $KEY_9($KEY_0, [$KEY_3]);
    return $KEY_0;
}
"#;

// The record literal is hoisted into a local whose sole write is that literal.
const HOISTED_SINGLE_WRITE: &str = r#"
function $THREAD_20_1() {
    let $KEY_0;
    function $THREAD_21_2() {
        "use strict";
        if (this instanceof $KEY_0) {
            this.extensions = Object.create(null);
            return void 0;
        } else {
            throw new TypeError("Cannot call a class as a function");
        }
    }
    $KEY_0 = $THREAD_21_2;
    let $KEY_3 = { key: "extend" };
    function $THREAD_22_3($CA, $CB, $CC, $CD) {
        let $CE = {
            detector: $CB,
            serialiser: $CC,
            deserialiser: $CD
        };
        this.extensions[$CA] = $CE;
    }
    $KEY_3.value = $THREAD_22_3;
    $KEY_9($KEY_0, [$KEY_3]);
    return $KEY_0;
}
"#;

// Different formals and property order must not gate the record.
const HOISTED_REORDERED_FIELDS: &str = r#"
function $THREAD_30_1() {
    let $KEY_0;
    function $THREAD_31_2() {
        "use strict";
        if (this instanceof $KEY_0) {
            this.extensions = Object.create(null);
            return void 0;
        } else {
            throw new TypeError("Cannot call a class as a function");
        }
    }
    $KEY_0 = $THREAD_31_2;
    let $KEY_3 = { key: "extend" };
    function $THREAD_32_3($BX, $BY, $BZ, $CA) {
        let $CB = {
            deserialiser: $CA,
            serialiser: $BZ,
            detector: $BY
        };
        this.extensions[$BX] = $CB;
    }
    $KEY_3.value = $THREAD_32_3;
    $KEY_9($KEY_0, [$KEY_3]);
    return $KEY_0;
}
"#;

// Readable local and parameter spellings still require the hoisted record single origin.
const HOISTED_READABLE_LOCALS: &str = r#"
function $THREAD_40_1() {
    let $KEY_0;
    function $THREAD_41_2() {
        "use strict";
        if (this instanceof $KEY_0) {
            this.extensions = Object.create(null);
            return void 0;
        } else {
            throw new TypeError("Cannot call a class as a function");
        }
    }
    $KEY_0 = $THREAD_41_2;
    let $KEY_3 = { key: "extend" };
    function $THREAD_42_3(key, detector, serialiser, deserialiser) {
        let record = {
            detector: detector,
            serialiser: serialiser,
            deserialiser: deserialiser
        };
        this.extensions[key] = record;
    }
    $KEY_3.value = $THREAD_42_3;
    $KEY_9($KEY_0, [$KEY_3]);
    return $KEY_0;
}
"#;

// The record is declared in the extend scope but written inside a nested
// donor; the parent must not borrow the child's literal.
const NESTED_DONOR: &str = r#"
function $THREAD_50_1() {
    let $KEY_0;
    function $THREAD_51_2() {
        "use strict";
        if (this instanceof $KEY_0) {
            this.extensions = Object.create(null);
            return void 0;
        } else {
            throw new TypeError("Cannot call a class as a function");
        }
    }
    $KEY_0 = $THREAD_51_2;
    let $KEY_3 = { key: "extend" };
    function $THREAD_52_3($CA, $CB, $CC, $CD) {
        let $CE;
        function $THREAD_53_9() {
            $CE = {
                detector: $CB,
                serialiser: $CC,
                deserialiser: $CD
            };
        }
        $THREAD_53_9();
        this.extensions[$CA] = $CE;
    }
    $KEY_3.value = $THREAD_52_3;
    $KEY_9($KEY_0, [$KEY_3]);
    return $KEY_0;
}
"#;

#[test]
fn inline_registry_base_keeps_all_three_names() {
    let source = program(INLINE_BASE);
    named_registry(&source, "$THREAD_10_1", "$THREAD_11_2", "$THREAD_12_3");
}

#[test]
fn hoisted_registry_record_names_all_three_roles() {
    for (body, factory, constructor, extend) in [
        (HOISTED_SINGLE_WRITE, "$THREAD_20_1", "$THREAD_21_2", "$THREAD_22_3"),
        (HOISTED_REORDERED_FIELDS, "$THREAD_30_1", "$THREAD_31_2", "$THREAD_32_3"),
        (HOISTED_READABLE_LOCALS, "$THREAD_40_1", "$THREAD_41_2", "$THREAD_42_3"),
    ] {
        let source = program(body);
        named_registry(&source, factory, constructor, extend);
    }
}

#[test]
fn wrong_slot_refuses_the_whole_registry() {
    let source = program(&HOISTED_SINGLE_WRITE.replace(
        "detector: $CB,\n            serialiser: $CC,",
        "detector: $CC,\n            serialiser: $CB,",
    ));
    refused_registry(&source, "$THREAD_20_1", "$THREAD_21_2", "$THREAD_22_3");
}

#[test]
fn multiple_record_writes_refuse_the_whole_registry() {
    let source = program(&HOISTED_SINGLE_WRITE.replace(
        "        };\n        this.extensions[$CA] = $CE;",
        "        };\n        $CE = { detector: $CB, serialiser: $CC, deserialiser: $CD };\n        this.extensions[$CA] = $CE;",
    ));
    refused_registry(&source, "$THREAD_20_1", "$THREAD_21_2", "$THREAD_22_3");
}

#[test]
fn wrong_receiver_refuses_the_whole_registry() {
    let source = program(&HOISTED_SINGLE_WRITE.replace(
        "        this.extensions[$CA] = $CE;",
        "        let $KEY_other = {};\n        $KEY_other.extensions[$CA] = $CE;",
    ));
    refused_registry(&source, "$THREAD_20_1", "$THREAD_21_2", "$THREAD_22_3");
}

#[test]
fn shadowed_key_refuses_the_whole_registry() {
    let source = program(&HOISTED_SINGLE_WRITE.replace(
        "        this.extensions[$CA] = $CE;",
        "        {\n            let $CA = $CB;\n            this.extensions[$CA] = $CE;\n        }",
    ));
    refused_registry(&source, "$THREAD_20_1", "$THREAD_21_2", "$THREAD_22_3");
}

#[test]
fn non_literal_record_origin_refuses_the_whole_registry() {
    let source = program(&HOISTED_SINGLE_WRITE.replace(
        "        let $CE = {\n            detector: $CB,\n            serialiser: $CC,\n            deserialiser: $CD\n        };",
        "        let $CE = $makeRecord($CB, $CC, $CD);",
    ));
    refused_registry(&source, "$THREAD_20_1", "$THREAD_21_2", "$THREAD_22_3");
}

#[test]
fn nested_donor_refuses_the_whole_registry() {
    let source = program(NESTED_DONOR);
    refused_registry(&source, "$THREAD_50_1", "$THREAD_51_2", "$THREAD_52_3");
}

// Existing installer-factory/installer roles (`descriptor_loops`,
// `shared_class_member_installer`) and the null-prototype identity detector
// (`shared_immutable_wrapper`) must retain their names. Only the hoisted registry trio is
// added; adjacent serializer installer or sentinel roles must not displace these claims.
const PRESERVED_ROLES: &str = r#"
function $THREAD_80_1() {
    function $THREAD_80_2($BZ, $CA, $CB) {
        if ($CA) {
            let j = 0;
            for (; j < $CA.length; j++) {
                $CA[j].enumerable = $CA[j].enumerable || false;
                $CA[j].configurable = true;
                if ("value" in $CA[j]) {
                    $CA[j].writable = true;
                }
                Object.defineProperty($BZ.prototype, $CA[j].key, $CA[j]);
            }
        }
        if ($CB) {
            let i = 0;
            for (; i < $CB.length; i++) {
                $CB[i].enumerable = $CB[i].enumerable || false;
                $CB[i].configurable = true;
                if ("value" in $CB[i]) {
                    $CB[i].writable = true;
                }
                Object.defineProperty($BZ, $CB[i].key, $CB[i]);
            }
        }
        return $BZ;
    }
    return $THREAD_80_2;
}
function $THREAD_81_1() {
    function $THREAD_81_2($BZ, $CA, $CB) {
        if ($CA) {
            let j = 0;
            for (; j < $CA.length; j++) {
                $CA[j].enumerable = $CA[j].enumerable || false;
                $CA[j].configurable = true;
                if ("value" in $CA[j]) {
                    $CA[j].writable = true;
                }
                Object.defineProperty($BZ.prototype, $CA[j].key, $CA[j]);
            }
        }
        if ($CB) {
            let i = 0;
            for (; i < $CB.length; i++) {
                $CB[i].enumerable = $CB[i].enumerable || false;
                $CB[i].configurable = true;
                if ("value" in $CB[i]) {
                    $CB[i].writable = true;
                }
                Object.defineProperty($BZ, $CB[i].key, $CB[i]);
            }
        }
        return $BZ;
    }
    return $THREAD_81_2;
}
let $installer = $THREAD_80_1();
let $otherInstaller = $THREAD_81_1();
function $THREAD_82_1() {
    let $KEY_0;
    function $THREAD_82_2() {
        "use strict";
        if (this instanceof $KEY_0) {
            this.extensions = Object.create(null);
            return void 0;
        } else {
            throw new TypeError("Cannot call a class as a function");
        }
    }
    $KEY_0 = $THREAD_82_2;
    let $KEY_3 = { key: "extend" };
    function $THREAD_82_3($CA, $CB, $CC, $CD) {
        let $CE = {
            detector: $CB,
            serialiser: $CC,
            deserialiser: $CD
        };
        this.extensions[$CA] = $CE;
    }
    $KEY_3.value = $THREAD_82_3;
    $installer($KEY_0, [$KEY_3]);
    return $KEY_0;
}
const $token = Object.create(null);
function $THREAD_83_1($BZ) {
    return $BZ === $token;
}
let $sentinel = { detector: $THREAD_83_1 };
function $THREAD_83_2() {
}
$sentinel.serialiser = $THREAD_83_2;
function $THREAD_83_3() {
    return $token;
}
$sentinel.deserialiser = $THREAD_83_3;
"#;

#[test]
fn hoisted_registry_keeps_current_installer_and_sentinel_roles() {
    let source = program(PRESERVED_ROLES);
    // The reason this fixture is used: the hoisted trio itself must still be
    // owned, otherwise the preservation assertions below could pass vacuously.
    named_registry(&source, "$THREAD_82_1", "$THREAD_82_2", "$THREAD_82_3");
    for (target, name) in [
        ("$THREAD_80_2", "installPropertyDescriptors"),
        ("$THREAD_80_1", "createClassMembersInstaller"),
        ("$THREAD_81_1", "createClassMembersInstaller2"),
        ("$THREAD_83_1", "isNullPrototypeSentinel"),
    ] {
        assert!(
            named(&source, target, name),
            "hoisted registry displaced the existing role {name} on {target}:\n{source}"
        );
    }
    for (target, name) in [
        ("$THREAD_80_2", "defineClassMembers"),
        ("$THREAD_81_1", "createInstallPropertyDescriptors"),
        ("$THREAD_83_1", "detectSentinelValue"),
    ] {
        assert!(
            !named(&source, target, name),
            "hoisted registry assigned the adjacent role {name} on {target}:\n{source}"
        );
    }
}
