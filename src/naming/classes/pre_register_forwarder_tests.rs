use super::*;

const OWN: &str = r#"
function main() {
    function Base() {}
    function capturedHook() { opaqueExternalEffect(); }
    function makeForwarder(base) {
        class $THREAD_9_17 extends base {
            constructor() { super(...arguments); }
            register(value) { capturedHook(); super.register(value); return this; }
        }
        return $THREAD_9_17;
    }
    let Result = makeForwarder(Base);
    new Result();
}
main();
"#;

fn inspect<R>(source: &str, check: impl FnOnce(&Model<'_, '_>) -> R) -> R {
    let allocator = oxc_allocator::Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source).expect("valid fixture");
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        source,
        &program,
        &scoping,
        &nodes,
        crate::signals::schema::ProgramPropertiesLimits::default(),
    )
    .expect("model");
    check(&model)
}
fn accepts(source: &str) {
    inspect(source, |model| {
        let claims = super::super::claims(model);
        assert_eq!(claims.len(), 1);
        assert!(matches!(&claims[0], FinderClaim::ClassBinding {
            role: "owned_pre_register_hook_forwarder_class", evidence, ..
        } if evidence.iter().any(|s| s.contains("explicitly unproved"))));
    });
}
fn refuses(source: &str) {
    inspect(source, |model| {
        assert!(super::super::claims(model).is_empty(), "{source}")
    });
}
fn changed(old: &str, new: &str) -> String {
    assert!(OWN.contains(old));
    OWN.replacen(old, new, 1)
}
#[test]
fn forwarding_neutral_positive_and_alpha_use_scoped_identities() {
    accepts(OWN);
    accepts(
        &OWN.replace("capturedHook", "unrelatedSpelling")
            .replace("base", "Ancestor")
            .replace("value", "payload")
            .replace("$THREAD_9_17", "$THREAD_3_456"),
    );
    accepts(&changed(
        "return $THREAD_9_17;",
        "let Alias = $THREAD_9_17; return Alias;",
    ));
    accepts(&changed(
        "new Result();",
        "let Alias = Result; new Alias();",
    ));
    // A different enclosing hook has its own identity, and asserts no native
    // meaning merely because another same-spelled declaration exists elsewhere.
    accepts(&changed(
        "function main()",
        "function elsewhere() { function capturedHook() {} } function main()",
    ));
}
#[test]
fn forwarding_neutral_complete_protocol_refusals() {
    for (old, new) in [
        (
            "capturedHook(); super.register(value);",
            "super.register(value); capturedHook();",
        ),
        ("capturedHook();", "capturedHook(value);"),
        ("capturedHook();", "capturedHook?.();"),
        ("capturedHook();", "otherHook();"),
        (
            "capturedHook();",
            "let capturedHook = donor; capturedHook();",
        ),
        (
            "capturedHook();",
            "try { capturedHook(); } catch (error) {}",
        ),
        ("super.register(value);", "super.register();"),
        ("super.register(value);", "super.register(...value);"),
        ("super.register(value);", "super.register(value, value);"),
        (
            "super.register(value); return this;",
            "return super.register(value);",
        ),
        ("return this;", "return value;"),
        ("return this;", "donor(); return this;"),
        ("register(value)", "async register(value)"),
        ("register(value)", "*register(value)"),
        ("register(value)", "register(value = donor())"),
        ("register(value)", "register(capturedHook)"),
        ("register(value)", "register(value, capturedHook)"),
        ("constructor()", "constructor(value)"),
        ("super(...arguments);", "super(arguments);"),
        ("super(...arguments);", "donor(); super(...arguments);"),
        ("return $THREAD_9_17;", "donor(); return $THREAD_9_17;"),
        ("return $THREAD_9_17;", "return Base;"),
        ("extends base", "extends donor"),
        (
            "function makeForwarder(base)",
            "function makeForwarder(capturedHook)",
        ),
        ("makeForwarder(Base)", "makeForwarder(capturedHook)"),
        ("new Result();", "useElsewhere(Result);"),
        (
            "new Result();",
            "Result.prototype.register = donor; new Result();",
        ),
        (
            "function capturedHook() { opaqueExternalEffect(); }",
            "let capturedHook = donor;",
        ),
        (
            "function capturedHook() { opaqueExternalEffect(); }",
            "function capturedHook() {} capturedHook = donor;",
        ),
        ("constructor() {", "extra() { donor(); } constructor() {"),
    ] {
        refuses(&changed(old, new));
    }
}
#[test]
fn forwarding_neutral_does_not_displace_existing_native_proof() {
    let source = include_str!("../../../tests/fixtures/program/lazy-registry/native-reverse.js");
    inspect(source, |model| {
        let claims = super::super::claims(model);
        assert_eq!(claims.len(), 1);
        assert!(matches!(
            &claims[0],
            FinderClaim::ClassBinding {
                role: "owned_lazy_hook_registry_class",
                ..
            }
        ));
    });
}
