use crate::signals::schema::ProgramPropertiesLimits;
use super::*;

const REVERSE_FIXTURE: &str =
    include_str!("../../../tests/fixtures/program/callback-notifier/native-reverse.js");
const FORWARD_FIXTURE: &str =
    include_str!("../../../tests/fixtures/program/callback-notifier/native-forward.js");
const STATIC_BASE_MUTATION_FIXTURE: &str = include_str!(
    "../../../tests/fixtures/program/callback-notifier/counterexample-static-base-mutation.js"
);

fn inspect(source: &str, f: impl FnOnce(&Model<'_, '_>)) {
    let wrapped = format!("function bootstrap() {{\n{source}\n}} bootstrap();");
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &wrapped, SourceType::default()).parse();
    assert!(parsed.errors.is_empty(), "{:?}\n{source}", parsed.errors);
    let built = SemanticBuilder::new().build(&parsed.program);
    assert!(built.errors.is_empty(), "{:?}", built.errors);
    let (scoping, nodes) = built.semantic.into_scoping_and_nodes();
    let model = Model::new(
        &wrapped,
        &parsed.program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .expect("valid negative/positive Model");
    f(&model);
}
fn roles(source: &str) -> Vec<&'static str> {
    let mut result = Vec::new();
    inspect(source, |model| {
        result = claims(model)
            .iter()
            .map(|c| {
                let FinderClaim::ClassBinding { role, .. } = c else {
                    panic!("native classes only")
                };
                *role
            })
            .collect()
    });
    result
}
fn refuses(source: &str) {
    assert!(roles(source).is_empty(), "{source}");
}

// Inspect the actual native declaration, not its generated spelling, so the
// native claim and the dependency API must agree on the same scoped base.
fn native_base_contract(source: &str, expected_role: Option<&str>) {
    inspect(source, |model| {
        let base = model
            .nodes
            .iter()
            .find_map(|node| match node.kind() {
                AstKind::Class(class) if class.super_class.is_none() => Some(class),
                _ => None,
            })
            .expect("native base declaration retained");
        let symbol = base.id.as_ref().unwrap().symbol_id.get().unwrap();
        let found = claims(model);
        let base_roles: Vec<_> = found
            .iter()
            .filter_map(|claim| match claim {
                FinderClaim::ClassBinding {
                    class_span, role, ..
                } if *class_span == base.span => Some(*role),
                _ => None,
            })
            .collect();
        assert_eq!(
            base_roles,
            expected_role.into_iter().collect::<Vec<_>>(),
            "{source}"
        );
        assert_eq!(
            proves_base(model, symbol),
            expected_role.is_some(),
            "{source}"
        );
    });
}

fn with_subclass_element(source: &str, element: &str) -> String {
    let superclass = source.find(" extends ").expect("real returned subclass");
    let body_start = superclass + source[superclass..].find('{').unwrap() + 1;
    format!(
        "{}\n{element}\n{}",
        &source[..body_start],
        &source[body_start..]
    )
}

#[test]
fn real_native_transports_prove_distinct_protocols() {
    assert_eq!(
        roles(REVERSE_FIXTURE),
        ["owned_callback_notifier_reverse_class"]
    );
    assert_eq!(
        roles(FORWARD_FIXTURE),
        ["owned_callback_notifier_forward_class"]
    );
    native_base_contract(
        REVERSE_FIXTURE,
        Some("owned_callback_notifier_reverse_class"),
    );
    native_base_contract(
        FORWARD_FIXTURE,
        Some("owned_callback_notifier_forward_class"),
    );
    assert_eq!(roles(REVERSE), ["owned_callback_notifier_reverse_class"]);
    assert_eq!(
        roles(&forward_source()),
        ["owned_callback_notifier_forward_class"]
    );
    // Locals are alpha-renamed by resolved SymbolId, property keys stay exact.
    assert_eq!(
        roles(
            &REVERSE
                .replace("payload", "$P")
                .replace("snapshot", "$S")
                .replace("count", "$N")
                .replace("accumulator", "$A")
        ),
        ["owned_callback_notifier_reverse_class"]
    );
}

#[test]
fn complete_contract_rejects_receiver_argument_catch_sparse_and_order_changes() {
    for (from, to) in [
        ("this.callbacks = [];", "this.callbacks = foreign;"),
        (
            "this.callbacks.push(callback)",
            "other.callbacks.push(callback)",
        ),
        ("return this;", "return void 0;"),
        ("this.callbacks[i] = void 0", "this.callbacks[i] = null"),
        (
            "this.callbacks[i] === callback",
            "this.callbacks[i] !== callback",
        ),
        (
            "Array.prototype.slice.call(this.callbacks)",
            "this.callbacks",
        ),
        (
            "Array.prototype.slice.call(this.callbacks)",
            "Array.prototype.slice.call(other.callbacks)",
        ),
        (
            "Array.prototype.slice.call(this.callbacks)",
            "Array.prototype.slice.call(this.callbacks, 1)",
        ),
        ("count - 1 >= 0", "count - 1 > 0"),
        ("--count", "++count"),
        ("snapshot[count - 1] != null", "true"),
        (
            ".call(void 0, payload, accumulator)",
            ".call(this, payload, accumulator)",
        ),
        (
            ".call(void 0, payload, accumulator)",
            ".call(void 0, accumulator, payload)",
        ),
        ("catch (caught) {}", "catch (caught) { throw caught; }"),
        ("return accumulator;", "return payload;"),
        ("let accumulator = null", "let accumulator = void 0"),
        (
            "let count = snapshot.length",
            "let count = this.callbacks.length",
        ),
        ("snapshot[count - 1].call", "this.callbacks[count - 1].call"),
    ] {
        assert!(REVERSE.contains(from), "{from}");
        refuses(&REVERSE.replace(from, to));
    }
    for (from, to) in [
        ("i < this.callbacks.length", "i <= this.callbacks.length"),
        ("i++", "i--"),
        ("this.callbacks[i].call", "other.callbacks[i].call"),
        ("let i = 0", "let i = 1"),
    ] {
        refuses(&forward_source().replace(from, to));
    }
}

#[test]
fn donor_nested_callables_duplicate_static_keys_and_unknown_effects_refuse() {
    for changed in [
        REVERSE.replace(
            "this.callbacks.push(callback);",
            "function donor(callback) { this.callbacks.push(callback); }",
        ),
        REVERSE.replace(
            "this.callbacks.push(callback);",
            "(() => this.callbacks.push(callback))();",
        ),
        REVERSE.replacen(
            "\n    register(callback) {",
            "\n    static register(callback) {",
            1,
        ),
        REVERSE.replace("clear()", "['clear']()"),
        REVERSE.replace("clear()", "register(callback) { return this; } clear()"),
        REVERSE.replace(
            "return accumulator;",
            "foreign(this.callbacks); return accumulator;",
        ),
        REVERSE.replace("notify(payload)", "async notify(payload)"),
        REVERSE.replace("snapshot[count - 1].call", "donor[count - 1].call"),
        REVERSE.replace("--count;", "snapshot = other; --count;"),
    ] {
        refuses(&changed);
    }
    refuses(&format!("let Array = foreign; {REVERSE}"));
    // A correct sibling remains independently provable; it cannot repair the
    // malformed candidate's own initialization or contribute a second claim.
    assert_eq!(
        roles(&format!(
            "{}\n{REVERSE}",
            REVERSE
                .replace("Notifier", "Sibling")
                .replace("this.callbacks = [];", "this.callbacks = null;")
        )),
        ["owned_callback_notifier_reverse_class"]
    );
}

#[test]
fn constructor_alias_mutation_unknown_escape_and_installation_refuse() {
    for suffix in [
        "Notifier = Other;",
        "Notifier.prototype.notify = foreign;",
        "Notifier[unknown] = foreign;",
        "delete Notifier.prototype.notify;",
        "escape(Notifier);",
        "let alias = Notifier; alias.prototype.notify = foreign;",
        "let alias = Notifier; alias = Other;",
        "Object.defineProperty(Notifier, 'prototype', {writable:true});",
        "Object.defineProperty(Notifier, 'prototype', {writable:false,value:{}});",
        "Object.defineProperty(Notifier.prototype, 'notify', foreign);",
    ] {
        refuses(&format!("{REVERSE}\n{suffix}"));
    }
    refuses(&format!("let Object = foreign; {REVERSE}\nObject.defineProperty(Notifier, 'prototype', {{writable:false}});"));
    refuses(&format!("let alias = Notifier; {REVERSE} new alias();"));
    // Transport may not let the base escape, mutate it or substitute a donor.
    for extra in [
        "escape(Base);",
        "Base.prototype.notify = foreign;",
        "Base = Other;",
    ] {
        refuses(&format!("{REVERSE}\nfunction factory(Base) {{ {extra} class Child extends Base {{}} return Child; }} let Child = factory(Notifier); new Child();"));
    }
}

#[test]
fn proves_base_uses_scoped_origin_and_refuses_ordinary_name_only_bases() {
    inspect(
        &format!("{REVERSE} let first = Notifier; let second = first; new second();"),
        |model| {
            for name in ["Notifier", "first", "second"] {
                let symbol = model
                    .scoping
                    .symbol_ids()
                    .find(|s| model.scoping.symbol_name(*s) == name)
                    .unwrap();
                assert!(proves_base(model, symbol), "{name}");
            }
        },
    );
    inspect("function CallbackNotifierConstructor() { this.callbacks = []; } function initializeCallbackList() { this.callbacks = []; }", |model| {
        for symbol in model.scoping.symbol_ids().filter(|s| matches!(model.scoping.symbol_name(*s), "CallbackNotifierConstructor" | "initializeCallbackList")) {
            assert!(!proves_base(model, symbol));
        }
    });
}

#[test]
fn factory_class_definition_and_indirect_superclass_mutation_refuse() {
    for methods in [
        "static { this.__proto__.prototype.notify = foreign; }",
        "static [foreign()]() {}",
        "register(cb) { super.constructor.prototype.notify = foreign; return this; }",
        "register(cb) { leak(this); return this; }",
        "register(cb) { this.constructor.__proto__.prototype.notify = foreign; return this; }",
    ] {
        refuses(&format!("{REVERSE}\nfunction factory(Base) {{ class Child extends Base {{ {methods} }} return Child; }} let Child = factory(Notifier); new Child();"));
    }
}

#[test]
fn real_returned_subclass_effects_invalidate_native_claim_and_base_dependency() {
    // Exact counterexample bytes, including the original factory,
    // returned subclass, alias and constructor caller.
    native_base_contract(STATIC_BASE_MUTATION_FIXTURE, None);
    for source in [REVERSE_FIXTURE, FORWARD_FIXTURE] {
        for element in [
            "static { super.prototype.notify = function () { return 99; }; }",
            "static [foreign()]() {}",
            "[foreign()]() {}",
            "mutate() { super.constructor.prototype.notify = foreign; return this; }",
            "mutate() { this.constructor.__proto__.prototype.notify = foreign; return this; }",
            "escape() { leak(this); return this; }",
        ] {
            native_base_contract(&with_subclass_element(source, element), None);
        }
    }
}

const LOWERED_NATIVE_FIXTURE: &str = include_str!(
    "../../../tests/fixtures/program/callback-notifier/ordinary-callback-base.js"
);
const ORDINARY_REVERSE_FIXTURE: &str = include_str!(
    "../../../tests/fixtures/program/callback-notifier/ordinary-reverse-base.js"
);
const ORDINARY_FORWARD_FIXTURE: &str = include_str!(
    "../../../tests/fixtures/program/callback-notifier/ordinary-forward-base.js"
);

fn ordinary_contract(source: &str, expected: bool) {
    inspect(source, |model| {
        let symbol = model.nodes.iter().find_map(|n| match n.kind() {
            AstKind::Function(f) if f.params.items.is_empty()
                && f.body.as_ref().is_some_and(|b| b.statements.len() == 1) =>
                f.id.as_ref().and_then(|id| id.symbol_id.get()),
            _ => None,
        }).expect("retained ordinary constructor");
        assert_eq!(proves_base(model, symbol), expected, "{source}");
        assert!(claims(model).is_empty(), "ordinary dependency is not a native/function gain");
    });
}

#[test]
fn current_lowered_native_reverse_preserves_complete_protocol() {
    native_base_contract(LOWERED_NATIVE_FIXTURE, Some("owned_callback_notifier_reverse_class"));
    for (from, to) in [
        ("i = Number($AM)", "i = Number($AM) + 1"),
        ("$AM = i - 1", "$AM = i + 1"),
        ("!($AM >= 0)", "!($AM > 0)"),
        ("$AK[$AM]", "this.callbacks[$AM]"),
        ("$AN($AJ, $AL)", "$AN($AL, $AJ)"),
        ("$AN($AJ, $AL)", "$AN.call(this, $AJ, $AL)"),
        ("catch (e) {}", "catch (e) { throw e; }"),
        ("return $AL;", "return $AJ;"),
    ] {
        assert!(LOWERED_NATIVE_FIXTURE.contains(from));
        native_base_contract(&LOWERED_NATIVE_FIXTURE.replace(from, to), None);
    }
    native_base_contract(&format!("let Number = foreign; {LOWERED_NATIVE_FIXTURE}"), None);
}

#[test]
fn ordinary_reverse_and_live_forward_are_complete_owned_base_dependencies() {
    ordinary_contract(ORDINARY_REVERSE_FIXTURE, true);
    ordinary_contract(ORDINARY_FORWARD_FIXTURE, true);
    for source in [ORDINARY_REVERSE_FIXTURE, ORDINARY_FORWARD_FIXTURE] {
        ordinary_contract(&with_subclass_element(source,
            "static { super.prototype.notify = foreign; }"), false);
        ordinary_contract(&source.replace("this.callbacks = [];", "this.callbacks = null;"), false);
        ordinary_contract(&source.replace(".call(void 0,", ".call(this,"), false);
        ordinary_contract(&source.replace("catch (e) {}", "catch (e) { throw e; }"), false);
        ordinary_contract(&source.replace(".prototype.notify =", ".prototype.other ="), false);
        ordinary_contract(&source.replace(".prototype.clear =", ".prototype.register ="), false);
        ordinary_contract(&format!("return; {source}"), false);
    }
    // Origin aliases are scoped, stable and traversed at every hop.
    let aliased = ORDINARY_REVERSE_FIXTURE.replace(
        "let $BR = $THREAD_3_76512($THREAD_5_99971);",
        "let first = $THREAD_5_99971; let second = first; let $BR = $THREAD_3_76512(second);",
    );
    ordinary_contract(&aliased, true);
    inspect(&aliased, |model| {
        for name in ["first", "second"] {
            let symbol = model.scoping.symbol_ids().find(|s| model.scoping.symbol_name(*s) == name).unwrap();
            assert!(proves_base(model, symbol));
        }
    });
    ordinary_contract(&format!("{aliased} first = foreign;"), false);
    ordinary_contract(&format!("{aliased} escape(second);"), false);
    ordinary_contract(&format!("{aliased} second.prototype.notify = foreign;"), false);
    ordinary_contract(&ORDINARY_REVERSE_FIXTURE.replace(
        "$THREAD_5_99971.prototype.notify = $THREAD_2_99666;",
        "if (false) { $THREAD_5_99971.prototype.notify = $THREAD_2_99666; }",
    ), false);
    ordinary_contract(&ORDINARY_REVERSE_FIXTURE.replace(
        "$THREAD_5_99971.prototype.notify = $THREAD_2_99666;",
        "let dead = $THREAD_5_99971.prototype.notify;",
    ), false);
    ordinary_contract(&ORDINARY_REVERSE_FIXTURE.replace(
        "$THREAD_5_99971.prototype.notify = $THREAD_2_99666;",
        "let premature = new $THREAD_5_99971(); $THREAD_5_99971.prototype.notify = $THREAD_2_99666;",
    ), false);
}

fn descriptors(source: &str) -> String {
    let mut result = String::new();
    for line in source.lines() {
        if let Some((left, right)) = line.trim().strip_suffix(';').and_then(|s| s.split_once(" = ")) {
            if let Some((base, method)) = left.split_once(".prototype.") {
                result.push_str(&format!("Object.defineProperty({base}.prototype, '{method}', {{value:{right}, writable:true, configurable:true, enumerable:false}});\n"));
                continue;
            }
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

#[test]
fn ordinary_descriptors_are_exact_owned_installations_and_never_donor_bags() {
    for source in [ORDINARY_REVERSE_FIXTURE, ORDINARY_FORWARD_FIXTURE] {
        let complete = descriptors(source);
        ordinary_contract(&complete, true);
        for (from, to) in [
            ("writable:true", "writable:false"),
            ("configurable:true", "configurable:foreign()"),
            ("enumerable:false", "get:foreign"),
            ("enumerable:false", "enumerable:false, extra:true"),
            ("enumerable:false", "enumerable:false, value:foreign"),
            ("Object.defineProperty", "Other.defineProperty"),
            ("'notify'", "'other'"),
        ] {
            ordinary_contract(&complete.replace(from, to), false);
        }
        ordinary_contract(&format!("let Object = foreign; {complete}"), false);
        ordinary_contract(&with_subclass_element(&complete, "[foreign()]() {}"), false);
    }
    ordinary_contract(&ORDINARY_REVERSE_FIXTURE.replace(
        "let $BR = $THREAD_3_76512($THREAD_5_99971);",
        "return; let $BR = $THREAD_3_76512($THREAD_5_99971);",
    ), false);
    let unused = ORDINARY_REVERSE_FIXTURE.lines().filter(|line| !line.contains("new $BR()"))
        .collect::<Vec<_>>().join("\n");
    ordinary_contract(&unused, false);
    ordinary_contract(&format!("{unused} let unusedAlias = $BR;"), false);
    ordinary_contract(&format!("{ORDINARY_REVERSE_FIXTURE} escape($THREAD_2_99666);"), false);
    ordinary_contract(&ORDINARY_REVERSE_FIXTURE.replace(
        "$THREAD_5_99971.prototype.notify = $THREAD_2_99666;",
        "function donorInstaller() { $THREAD_5_99971.prototype.notify = $THREAD_2_99666; }",
    ), false);
}

// The definition label is useful on actual indexed declarations without
// becoming a base-protocol proof for this module's downstream consumers.
#[test]
fn indexed_definition_cannot_authorize_base_or_lazy_registry_proofs() {
    let definition = format!("{}{}\n}}", REVERSE.split("notify(payload)").next().unwrap(), INDEXED_DEFINITION_NOTIFY);
    for result_use in [
        "let instance = new Derived(); instance.constructor.__proto__.prototype.notify = foreign;",
        "let instance = new Derived(); let alias = instance; alias.constructor.__proto__.prototype.notify = foreign;",
        "externalInstance = new Derived(); escape(externalInstance);",
        "captured = new Derived(); function recover() { captured.constructor.__proto__.prototype.notify = foreign; }",
        "escape(Derived);",
    ] {
        let source = format!("{definition}\nfunction factory(Base) {{ class Derived extends Base {{ constructor() {{ super(...arguments); }} register(cb) {{ install(); super.register(cb); return this; }} }} return Derived; }} let BaseAlias = Notifier; let Derived = factory(BaseAlias); {result_use}");
        inspect(&source, |model| {
            let class = model.nodes.iter().find_map(|n| match n.kind() {
                AstKind::Class(c) if c.super_class.is_none() => Some(c), _ => None,
            }).unwrap();
            let binding = class.id.as_ref().unwrap().symbol_id.get().unwrap();
            assert!(definition_claim(model, class).is_some());
            assert!(native_role(model, class).is_none());
            assert!(!proves_base(model, binding));
            for s in model.scoping.symbol_ids().filter(|s| model.scoping.symbol_name(*s) == "BaseAlias") {
                assert!(!proves_base(model, s), "aliases cannot turn a definition into an algorithm fact");
            }
            assert!(claims(model).iter().any(|c| matches!(c, FinderClaim::ClassBinding {
                role: "owned_reverse_snapshot_notifier_definition", class_span, ..
            } if *class_span == class.span)));
            assert!(crate::naming::classes::lazy_registry::claims(model).iter().all(|c| c.role() != "owned_lazy_hook_registry_class"));
            assert!(crate::signals::analysis::roles::finder_role("owned_reverse_snapshot_notifier_definition").is_none(), "definition role is not a generic function-helper vocabulary entry");
        });
    }
}

#[test]
fn real_lazy_consumer_positive_loses_only_strong_proof_when_base_becomes_indexed_definition() {
    let accepted = include_str!("../../../tests/fixtures/program/lazy-registry/native-reverse.js");
    let mut changed = accepted.to_string();
    inspect(accepted, |model| {
        let base = model.nodes.iter().find_map(|n| match n.kind() {
            AstKind::Class(c) if c.super_class.is_none() => Some(c), _ => None,
        }).unwrap();
        let symbol = base.id.as_ref().unwrap().symbol_id.get().unwrap();
        assert!(proves_base(model, symbol), "real supported legacy base positive control");
        assert!(crate::naming::classes::lazy_registry::claims(model).iter().any(|c| c.role() == "owned_lazy_hook_registry_class"), "real complete installer positive control");
        let replacement = format!("{}{}\n}}", REVERSE.split("notify(payload)").next().unwrap(), INDEXED_DEFINITION_NOTIFY)
            .replacen("Notifier", base.id.as_ref().unwrap().name.as_str(), 1);
        // This is a test mutation of the actual own declaration, preserving the
        // operational installer and caller, not borrowed evidence for the base.
        let prefix = "function bootstrap() {\n".len();
        changed.replace_range(base.span.start as usize - prefix..base.span.end as usize - prefix, &replacement);
    });
    inspect(&changed, |model| {
        let base = model.nodes.iter().find_map(|n| match n.kind() {
            AstKind::Class(c) if c.super_class.is_none() => Some(c), _ => None,
        }).unwrap();
        assert!(definition_claim(model, base).is_some());
        assert!(!proves_base(model, base.id.as_ref().unwrap().symbol_id.get().unwrap()));
        assert!(crate::naming::classes::lazy_registry::claims(model).iter().all(|c| c.role() != "owned_lazy_hook_registry_class"));
    });
}
