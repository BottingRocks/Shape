use oxc_ast::AstKind;
use sha2::{Digest, Sha256};
use shape::naming::name_threads;

const ROLE: &str = "owned_reverse_snapshot_notifier_definition";
const NAME: &str = "ReverseSnapshotNotifier";
const CLASS: &str = "$THREAD_901_43";
const CANONICAL: &str =
    include_str!("fixtures/program/reverse-snapshot-notifier-definition/operational-canonical.js");
const HELDOUT: &str = include_str!(
    "fixtures/program/reverse-snapshot-notifier-definition/operational-reordered-heldout.js"
);
const ACTUAL: &str = include_str!(
    "fixtures/program/reverse-snapshot-notifier-definition/captured-notifier-parent.js"
);

// Independently reparse both sides of the edits; spelling/count alone cannot
// establish that a constructor reference still denotes this exact declaration.
fn class_references(source: &str, start: u32) -> (u32, Vec<u32>) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::default()).parse();
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let built = oxc_semantic::SemanticBuilder::new().build(&parsed.program);
    assert!(built.errors.is_empty(), "{:?}", built.errors);
    let semantic = built.semantic;
    let class = semantic
        .nodes()
        .iter()
        .find_map(|node| match node.kind() {
            AstKind::Class(class) if class.span.start == start => Some(class),
            _ => None,
        })
        .expect("same exact class node");
    let id = class.id.as_ref().unwrap();
    let symbol = id.symbol_id.get().unwrap();
    let mut references: Vec<_> = semantic
        .scoping()
        .get_resolved_references(symbol)
        .map(|r| {
            let AstKind::IdentifierReference(id) = semantic.nodes().kind(r.node_id()) else {
                panic!("identifier reference");
            };
            id.span.start
        })
        .collect();
    references.sort_unstable();
    (id.span.start, references)
}

fn check(
    source: &str,
    binding: &str,
    expected: bool,
) -> shape::naming::NamedThreads {
    let result = name_threads(source, true).expect("valid source and scoped rename plan");
    let class = result
        .report
        .class_outcomes
        .iter()
        .find(|o| o.binding.name == binding)
        .expect("native class outcome");
    assert_eq!(class.applied, expected, "{class:?}");
    assert_eq!(
        class.class_id,
        format!("class@{}..{}", class.class_span.start, class.class_span.end)
    );
    assert!(result
        .report
        .outcomes
        .iter()
        .all(|o| o.binding.name != binding));
    if expected {
        assert_eq!(class.proposed_name.as_deref(), Some(NAME));
        assert_eq!(class.evaluator.as_deref(), Some(ROLE));
        assert_eq!(class.evidence.len(), 1);
        assert_eq!(class.evidence[0].start, class.class_span.start);
        assert_eq!(class.evidence[0].end, class.class_span.end);
        assert!(class.explanation.iter().any(|s| s.contains("client mutation, inherited runtime behavior and callback/intrinsic purity are not established")));
        assert!(class.reference_edits + class.shorthand_edits > 0);
        let mut replay = source.to_string();
        let mut edits: Vec<_> = result.report.edits.iter().collect();
        edits.sort_by_key(|edit| edit.start);
        for edit in edits.iter().rev() {
            replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
        }
        assert_eq!(replay, result.source);
        let offset = |position: u32| -> u32 {
            let delta: i64 = edits
                .iter()
                .filter(|edit| edit.end <= position)
                .map(|edit| edit.replacement.len() as i64 - i64::from(edit.end - edit.start))
                .sum();
            (i64::from(position) + delta).try_into().unwrap()
        };
        let (declaration, references) = class_references(source, class.class_span.start);
        let (renamed_declaration, renamed_references) =
            class_references(&result.source, offset(class.class_span.start));
        assert_eq!(renamed_declaration, offset(declaration));
        assert_eq!(
            renamed_references,
            references
                .into_iter()
                .map(|position| {
                    // Shorthand expansion keeps the original property key and
                    // places the renamed reference after that key and colon.
                    let within_edit = edits
                        .iter()
                        .find(|edit| edit.start == position)
                        .and_then(|edit| edit.replacement.rfind(NAME))
                        .unwrap_or(0) as u32;
                    offset(position) + within_edit
                })
                .collect::<Vec<_>>()
        );
        assert_eq!(
            name_threads(&result.source, true).unwrap().source,
            result.source
        );
    } else {
        assert!(result.source.contains(binding));
    }
    result
}

fn refuses_definition(source: &str, binding: &str) {
    let result = check(source, binding, false);
    let class = result
        .report
        .class_outcomes
        .iter()
        .find(|o| o.binding.name == binding)
        .unwrap();
    assert_ne!(
        class.evaluator.as_deref(),
        Some(ROLE),
        "wrong own semantics cannot retain the definition role"
    );
}

#[test]
fn reverse_snapshot_native_class_is_definition_only() {
    let result = check(ACTUAL, "$THREAD_3_117814", true);
    let class = result
        .report
        .class_outcomes
        .iter()
        .find(|o| o.class_id == "class@452817..453500")
        .unwrap();
    assert_eq!(class.binding.name, "$THREAD_3_117814");
    assert_eq!(class.binding.identifier.start, 452823);
    assert_eq!(class.binding.identifier.end, 452839);
    assert_eq!(class.evaluator.as_deref(), Some(ROLE));
    assert_eq!(class.proposed_name.as_deref(), Some(NAME));
}

#[test]
fn operational_canonical_and_independent_reordered_definition() {
    check(CANONICAL, CLASS, true);
    check(HELDOUT, "$THREAD_902_17", true);
    let unicode = format!("// 🧭 byte-offset control\n{CANONICAL}");
    check(&unicode, CLASS, true);
    let dry = name_threads(CANONICAL, false).unwrap();
    assert_eq!(dry.source, CANONICAL);
    assert!(dry.report.edits.is_empty());
    let readable = CANONICAL.replace(CLASS, "UserRegistry");
    assert_eq!(name_threads(&readable, true).unwrap().source, readable);
}

#[test]
fn owned_effect_receiver_accumulator_snapshot_and_lifecycle_changes_refuse() {
    for (from, to) in [
        (
            "Array.prototype.slice.call(this.callbacks)",
            "this.callbacks",
        ),
        (
            "Array.prototype.slice.call(this.callbacks)",
            "Array.prototype.slice.call(this.callbacks, 1)",
        ),
        ("copy.length - 1", "copy.length"),
        ("position >= 0", "position > 0"),
        ("position--", "position++"),
        ("copy[position]", "this.callbacks[position]"),
        ("copy[position] != null", "true"),
        (
            ".call(void 0, message, previous)",
            ".call(this, message, previous)",
        ),
        (
            ".call(void 0, message, previous)",
            ".call(void 0, previous, message)",
        ),
        ("let previous = null", "let previous = 0"),
        ("catch (failure) {}", "catch (failure) { throw failure; }"),
        ("return previous;", "foreignEffect(); return previous;"),
        ("return previous;", "return message;"),
        ("this.callbacks = [];", "this.callbacks = null;"),
        (
            "this.callbacks.push(listener)",
            "other.callbacks.push(listener)",
        ),
        (
            "this.callbacks[slot] === listener",
            "this.callbacks[slot] == listener",
        ),
        (
            "this.callbacks[slot] = void 0",
            "this.callbacks[slot] = null",
        ),
        ("return this;", "return void 0;"),
        (
            "clear() { this.callbacks = []; return this; }",
            "clear() { this.callbacks.length = 0; return this; }",
        ),
        ("notify(message)", "async notify(message)"),
        ("notify(message)", "notify(message = foreign())"),
        ("notify(message)", "notify(...message)"),
        ("constructor() {", "constructor() { foreignEffect();"),
        (
            "class $THREAD_901_43 {",
            "class $THREAD_901_43 { extra() { foreignEffect(); }",
        ),
    ] {
        assert!(
            CANONICAL.contains(from),
            "negative must change own source: {from}"
        );
        refuses_definition(&CANONICAL.replace(from, to), CLASS);
    }
    refuses_definition(
        &CANONICAL.replace(
            "function exerciseRegistry() {",
            "function exerciseRegistry() { let Array = foreign;",
        ),
        CLASS,
    );
}

#[test]
fn client_mutation_can_keep_definition_but_not_promote_its_scope() {
    for client in [
        "const instance = new Registry(); instance.constructor.prototype.notify = foreign;",
        "const instance = new Registry(); const alias = instance; alias.constructor.prototype.notify = foreign;",
        "externalInstance = new Registry(); escape(externalInstance);",
        "function factory(Base) { class Derived extends Base {} return Derived; } const Derived = factory(Registry); const instance = new Derived(); instance.constructor.__proto__.prototype.notify = foreign;",
        "function factory(Base) { class Derived extends Base {} return Derived; } const Derived = factory(Registry); captured = new Derived(); escape(captured);",
    ] {
        let source = CANONICAL.replace("const registry = new Registry();", &format!("{client}\nconst registry = new Registry();"));
        check(&source, CLASS, true);
    }
}

#[test]
fn collision_binding_write_and_expression_identity_are_not_relaxed() {
    let collision = CANONICAL.replace("function exerciseRegistry() {", "function exerciseRegistry() { let ReverseSnapshotNotifier = foreign; consume(ReverseSnapshotNotifier);");
    check(&collision, CLASS, false);
    check(
        &CANONICAL.replace(
            "const Registry =",
            "$THREAD_901_43 = foreign; const Registry =",
        ),
        CLASS,
        false,
    );
    let expression = CANONICAL
        .replace(
            "class $THREAD_901_43 {",
            "const $THREAD_901_43 = class InnerRegistry {",
        )
        .replace("    const Registry =", "    ; const Registry =");
    check(&expression, CLASS, false);
    let shorthand = CANONICAL.replace(
        "const Registry =",
        "const bag = {$THREAD_901_43}; const Registry =",
    );
    check(&shorthand, CLASS, true);
    let result = name_threads(&shorthand, true).unwrap();
    assert!(
        result
            .source
            .contains("$THREAD_901_43: ReverseSnapshotNotifier")
            || result
                .source
                .contains("$THREAD_901_43:ReverseSnapshotNotifier")
    );
}

#[test]
fn native_identities_edits_and_previous_outcomes_remain_exact() {
    let ledger: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "fixtures/program/reverse-snapshot-notifier-definition/naming-outcome-ledger.json"
    ))
    .unwrap();
    assert_eq!(ledger.len(), 21);
    // No source identifier, span, name, or body table enters production recognition. ACTUAL is
    // checked in; other affected full inputs require the optional local reports tree. Preserve
    // that external-corpus gating.
    let mut checked = 0;
    for row in ledger {
        let source = if row["key"] == "notifier-parent-create-callback-registry-register-callback-remove-registered-callback-merge-registered-callbacks-notify-callbacks-install-method-interceptor" {
            ACTUAL.to_string()
        } else {
            match std::fs::read_to_string(row["cleaned"].as_str().unwrap()) {
                Ok(source) => source,
                Err(_) => continue,
            }
        };
        checked += 1;
        assert_eq!(
            Sha256::digest(source.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
            row["sha256"].as_str().unwrap()
        );
        let target = &row["target"];
        let binding = target["binding"]["name"].as_str().unwrap();
        let result = check(&source, binding, true);
        let fresh = serde_json::to_value(&result.report).unwrap();
        let class = fresh["class_outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["class_id"] == target["class_id"])
            .unwrap();
        assert_eq!(class["class_span"], target["class_span"]);
        assert_eq!(class["binding"], target["binding"]);
        for (old_field, fresh_field, identity) in [
            ("prior_applied_classes", "class_outcomes", "class_id"),
            ("prior_applied_functions", "outcomes", "function_id"),
        ] {
            for old in row[old_field].as_array().unwrap() {
                let current = fresh[fresh_field]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|o| {
                        if identity == "function_id" {
                            o["binding"] == old["binding"]
                        } else {
                            o[identity] == old[identity]
                        }
                    })
                    .expect("prior outcome retained");
                assert_eq!(
                    current, old,
                    "prior metadata changed in {}: {}",
                    row["key"], old["binding"]["name"]
                );
            }
        }
        assert!(
            fresh["class_outcomes"].as_array().unwrap().iter().all(|o| {
                o["evaluator"] != "owned_lazy_hook_registry_class"
                    || row["prior_applied_classes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|old| {
                            old["class_id"] == o["class_id"] && old["evaluator"] == o["evaluator"]
                        })
            }),
            "definition label must not produce lazy-registry gains"
        );
    }
    assert!(checked >= 1, "the checked-in reverse snapshot notifier row must run");
}

#[test]
fn actual_owned_notify_changes_refuse_at_the_same_native_declaration() {
    let own = &ACTUAL[452817..453500];
    for (from, to) in [
        (
            "Array.prototype.slice.call(this.callbacks)",
            "this.callbacks",
        ),
        (".call(void 0, $AO, $AQ)", ".call(this, $AO, $AQ)"),
        ("let $AQ = null", "let $AQ = 0"),
        ("return $AQ;", "foreignEffect(); return $AQ;"),
    ] {
        assert!(own.contains(from));
        let mut changed = ACTUAL.to_string();
        changed.replace_range(452817..453500, &own.replace(from, to));
        refuses_definition(&changed, "$THREAD_3_117814");
    }
}

#[test]
fn live_ascending_is_not_definition_only_and_preserves_metadata() {
    let live = CANONICAL
        .replace("let copy = Array.prototype.slice.call(this.callbacks);", "")
        .replace("let position = copy.length - 1;", "let position = 0;")
        .replace(
            "position >= 0; position--",
            "position < this.callbacks.length; position++",
        )
        .replace("copy[position]", "this.callbacks[position]");
    let result = name_threads(&live, true).unwrap();
    let class = result
        .report
        .class_outcomes
        .iter()
        .find(|o| o.binding.name == CLASS)
        .unwrap();
    assert!(class.applied);
    assert_eq!(
        class.evaluator.as_deref(),
        Some("owned_callback_notifier_forward_class")
    );
    assert_eq!(class.proposed_name.as_deref(), Some("LiveCallbackNotifier"));
    assert_eq!(class.explanation, vec!["complete owned sparse callback lifecycle; same live callbacks list on ascending bounds and reads (reentrant mutation preserved), void-zero receiver, payload/accumulator order, swallowed callback failure, returned accumulator; stable constructor aliases and closed uses"]);
}
