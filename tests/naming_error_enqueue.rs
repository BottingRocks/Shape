use shape::naming::name_threads;

const ENQUEUE: &str = r#"function main(){
let ring = {};
function $THREAD_8_86476($CB){
let $KEY_2 = host.Object.create(null);
$KEY_2.message = $CB.message;
if($CB.name != null){$KEY_2.name = $CB.name;}
if($CB.description != null){$KEY_2.description = $CB.description;}
if($CB.stack != null){$KEY_2.stack = $CB.stack;}
if($CB.stacktrace != null){$KEY_2.stacktrace = $CB.stacktrace;}
if($CB.num != null){$KEY_2.num = $CB.num;}
if($CB.signalPosition != null){$KEY_2.signalPosition = $CB.signalPosition;}
ring.put($KEY_2);
}
return $THREAD_8_86476;
}
main();"#;

const EVALUATOR: &str = "signals_error_enqueue";

fn named(source: &str, expected: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .outcomes
            .iter()
            .any(|o| o.applied && o.proposed_name.as_deref() == Some(expected))
    })
}

fn our_claim(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .function_roles
            .iter()
            .any(|role| role.evaluators.iter().any(|e| e == EVALUATOR))
    })
}

fn parses(source: &str) -> bool {
    name_threads(source, true).is_ok()
}

fn denied(source: &str) {
    assert!(parses(source), "fixture did not parse: {source}");
    assert!(
        !our_claim(source),
        "unexpected frontier claim for: {source}"
    );
}

#[test]
fn enqueue_accepts_generated_alpha_and_namespace_variants() {
    let ours = [
        ENQUEUE.to_string(),
        ENQUEUE
            .replace("$THREAD_8_86476", "$THREAD_903_71902_2")
            .replace("$CB", "$KEY_400")
            .replace("$KEY_2", "$KEY_77")
            .replace("host", "b"),
        ENQUEUE.replace("host.Object.create(null)", "globalThis.Object.create(null)"),
        ENQUEUE.replace("let $KEY_2", "const $KEY_2"),
    ];
    for source in ours {
        assert!(named(&source, "enqueueSignalError"), "{source}");
        assert!(our_claim(&source), "not the frontier claim: {source}");
    }
    // A bare `Object.create(null)` receiver is already accepted by the legacy
    // p05 finder, so the frontier must decline and let the older claim stand.
    let bare = ENQUEUE.replace("host.Object.create(null)", "Object.create(null)");
    assert!(named(&bare, "enqueueSignalError"), "{bare}");
}

#[test]
fn enqueue_accepts_distinct_field_subsets_and_order() {
    let subsets = [
        ENQUEUE
            .replace("if($CB.description != null){$KEY_2.description = $CB.description;}", "")
            .replace("if($CB.num != null){$KEY_2.num = $CB.num;}", ""),
        ENQUEUE.replace(
            "if($CB.name != null){$KEY_2.name = $CB.name;}\nif($CB.description != null){$KEY_2.description = $CB.description;}\nif($CB.stack != null){$KEY_2.stack = $CB.stack;}",
            "if($CB.stack != null){$KEY_2.stack = $CB.stack;}\nif($CB.description != null){$KEY_2.description = $CB.description;}\nif($CB.name != null){$KEY_2.name = $CB.name;}",
        ),
        ENQUEUE.replace(
            "if($CB.stacktrace != null){$KEY_2.stacktrace = $CB.stacktrace;}\nif($CB.num != null){$KEY_2.num = $CB.num;}\nif($CB.signalPosition != null){$KEY_2.signalPosition = $CB.signalPosition;}",
            "if($CB.signalPosition != null){$KEY_2.signalPosition = $CB.signalPosition;}\nif($CB.stacktrace != null){$KEY_2.stacktrace = $CB.stacktrace;}\nif($CB.num != null){$KEY_2.num = $CB.num;}",
        ),
    ];
    for source in subsets {
        assert!(our_claim(&source), "{source}");
        assert!(named(&source, "enqueueSignalError"), "{source}");
    }
}

#[test]
fn enqueue_rejects_changed_copies_put_and_body_shape() {
    for (from, to) in [
        (
            "$KEY_2.message = $CB.message;",
            "$KEY_2.message = $CB.stack;",
        ),
        (
            "$KEY_2.message = $CB.message;",
            "$KEY_2.other = $CB.message;",
        ),
        (
            "$KEY_2.stack = $CB.stack;",
            "$KEY_2.stack = $CB.stacktrace;",
        ),
        (
            "if($CB.name != null){$KEY_2.name = $CB.name;}",
            "if($CB.name != null){$KEY_2.name = other.name;}",
        ),
        (
            "if($CB.stack != null){$KEY_2.stack = $CB.stack;}",
            "$KEY_2.stack = $CB.stack;",
        ),
        (
            "if($CB.stack != null){$KEY_2.stack = $CB.stack;}",
            "if($CB.stack == null){$KEY_2.stack = $CB.stack;}",
        ),
        ("ring.put($KEY_2);", "ring.put($KEY_2, 0);"),
        ("ring.put($KEY_2);", "ring.put(other);"),
        ("ring.put($KEY_2);", "other.put($KEY_2);"),
        ("ring.put($KEY_2);", "ring.put($KEY_2);return $KEY_2;"),
        ("ring.put($KEY_2);", "return ring.put($KEY_2);"),
        (
            "let $KEY_2 = host.Object.create(null);",
            "let $KEY_2 = host.Object.create({});",
        ),
        (
            "let $KEY_2 = host.Object.create(null);",
            "let $KEY_2 = Object.create(null, {});",
        ),
        (
            "let $KEY_2 = host.Object.create(null);",
            "let $KEY_2 = host.Object.create(null);$KEY_2 = other;",
        ),
        (
            "let $KEY_2 = host.Object.create(null);",
            "let $KEY_2 = host.Object.create(null);var $KEY_2;",
        ),
    ] {
        denied(&ENQUEUE.replace(from, to));
    }
    denied(&ENQUEUE.replace(
        "if($CB.description != null){$KEY_2.description = $CB.description;}",
        "if($CB.extra != null){$KEY_2.extra = $CB.extra;}",
    ));
    denied(&ENQUEUE.replace(
        "ring.put($KEY_2);",
        "if($CB.name != null){$KEY_2.name = $CB.name;}\nring.put($KEY_2);",
    ));
}

#[test]
fn enqueue_rejects_crossed_capture_binding_origins() {
    let capture_in_scope = ENQUEUE.replace("let ring = {};\n", "").replace(
        "function $THREAD_8_86476($CB){",
        "function $THREAD_8_86476($CB){let ring = {};",
    );
    denied(&capture_in_scope);

    let parameter_capture = ENQUEUE.replace("function main(){", "function main(ring){");
    denied(&parameter_capture);

    let function_capture = ENQUEUE.replace("let ring = {};", "function ring(){}");
    denied(&function_capture);

    denied(&ENQUEUE.replace("function main(){", "function main(){let host = {};"));
    denied(
        &ENQUEUE
            .replace(
                "function main(){",
                "function main(){let root = {Object: {create: () => ({})}};",
            )
            .replace("host.Object.create(null)", "root.Object.create(null)"),
    );
    let resource_capture = ENQUEUE.replace("let ring = {};", "using ring = {};");
    if parses(&resource_capture) {
        denied(&resource_capture);
    }
}

#[test]
fn enqueue_rejects_changed_function_shape() {
    for (from, to) in [
        (
            "function $THREAD_8_86476($CB){",
            "async function $THREAD_8_86476($CB){",
        ),
        (
            "function $THREAD_8_86476($CB){",
            "function* $THREAD_8_86476($CB){",
        ),
        (
            "function $THREAD_8_86476($CB){",
            "function $THREAD_8_86476($CB = other){",
        ),
        (
            "function $THREAD_8_86476($CB){",
            "function $THREAD_8_86476(...$CB){",
        ),
        (
            "function $THREAD_8_86476($CB){",
            "function $THREAD_8_86476({$CB}){",
        ),
        (
            "function $THREAD_8_86476($CB){",
            "function $THREAD_8_86476($CB, other){",
        ),
        (
            "$KEY_2.message = $CB.message;",
            "\"use strict\";$KEY_2.message = $CB.message;",
        ),
    ] {
        denied(&ENQUEUE.replace(from, to));
    }
}

#[test]
fn enqueue_needs_message_stack_and_two_optional_fields() {
    denied(&ENQUEUE.replace("if($CB.stack != null){$KEY_2.stack = $CB.stack;}", ""));
    denied(
        &ENQUEUE
            .replace("if($CB.name != null){$KEY_2.name = $CB.name;}", "")
            .replace(
                "if($CB.description != null){$KEY_2.description = $CB.description;}",
                "",
            )
            .replace(
                "if($CB.stacktrace != null){$KEY_2.stacktrace = $CB.stacktrace;}",
                "",
            )
            .replace("if($CB.num != null){$KEY_2.num = $CB.num;}", "")
            .replace(
                "if($CB.signalPosition != null){$KEY_2.signalPosition = $CB.signalPosition;}",
                "",
            ),
    );
}
#[test]
fn enqueue_preserves_human_names_and_collision_guards() {
    let source = ENQUEUE.replace("$THREAD_8_86476", "enqueueSignalError");
    assert_eq!(name_threads(&source, true).unwrap().source, source);

    for collision in [
        ENQUEUE.replace(
            "function main(){",
            "function main(){let enqueueSignalError = 1;",
        ),
        ENQUEUE.replace(
            "function main(){",
            "function main(){function enqueueSignalError(){}",
        ),
    ] {
        assert!(!named(&collision, "enqueueSignalError"), "{collision}");
    }
}
