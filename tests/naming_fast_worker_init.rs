use shape::naming::name_threads;

// Fast-loop family worker-dispatch-initializer.
// The outer initializer owns a zero-initialized `self` counter, one nested
// handler whose body is independently re-proved, and the exact
// `self.onmessage = handler` installation. The inner handler role is a
// different family and is deliberately not claimed here.
const EVALUATOR: &str = "fast_worker_init_dispatcher";
const NAME: &str = "initWorkerMessageDispatcher";

const WORKER_INIT: &str = r#"function main(){
function $THREAD_1_120507(){
self.__wx=0;
function $THREAD_4_82974(message){
let sequence=Number(self.__wx);
self.__wx=sequence+1;
vmCreateThread(message.data[3],message.data[1],message.data[0])(sequence,message.data[2]);
}
self.onmessage=$THREAD_4_82974;
}
return $THREAD_1_120507;
}
main();"#;

fn named(source: &str, expected: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.outcomes.iter().any(|outcome| {
            outcome.applied && outcome.proposed_name.as_deref() == Some(expected)
        })
    })
}

fn our_claim(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result
            .report
            .function_roles
            .iter()
            .any(|role| role.evaluators.iter().any(|evaluator| evaluator == EVALUATOR))
    })
}

fn parses(source: &str) -> bool {
    name_threads(source, true).is_ok()
}

fn denied(source: &str) {
    assert!(parses(source), "fixture did not parse: {source}");
    assert!(!named(source, NAME), "unexpected {NAME} for: {source}");
    assert!(!our_claim(source), "unexpected fast claim for: {source}");
}

#[test]
fn initializer_accepts_portable_alpha_and_opaque_spellings() {
    let ours = [
        WORKER_INIT.to_string(),
        WORKER_INIT
            .replace("$THREAD_1_120507", "$THREAD_900_777_2")
            .replace("$THREAD_4_82974", "$THREAD_42_31337")
            .replace("__wx", "$KEY_77")
            .replace("message", "event")
            .replace("onevent", "onmessage")
            .replace("sequence", "cursor")
            .replace("vmCreateThread", "launchWorker"),
        // The counter property spelling is opaque data, not role evidence.
        WORKER_INIT.replace("__wx", "resourceToken"),
        // A different unbound factory spelling is the same consumer protocol.
        WORKER_INIT.replace("vmCreateThread", "providedHandlerFactory"),
        // The nested handler may keep a readable name; the parent is still the
        // independently proved outer initializer.
        WORKER_INIT.replace("$THREAD_4_82974", "handleMessage"),
        // Both `let` and `var` are plain immutable cursor declarations.
        WORKER_INIT.replace("let sequence", "var sequence"),
        WORKER_INIT.replace("let sequence", "const sequence"),
    ];
    for source in ours {
        assert!(named(&source, NAME), "not named: {source}");
        assert!(our_claim(&source), "not the fast claim: {source}");
    }
}

#[test]
fn initializer_claims_only_the_outer_function() {
    let result = name_threads(WORKER_INIT, true).unwrap();
    let claimed: Vec<_> = result
        .report
        .function_roles
        .iter()
        .filter(|role| role.evaluators.iter().any(|evaluator| evaluator == EVALUATOR))
        .collect();
    assert_eq!(claimed.len(), 1, "expected exactly the outer initializer");
    assert_eq!(
        claimed[0].current_name.as_deref(),
        Some("$THREAD_1_120507")
    );
    let inner = result
        .report
        .function_roles
        .iter()
        .find(|role| role.current_name.as_deref() == Some("$THREAD_4_82974"))
        .expect("the nested handler must exist");
    assert!(
        !inner.evaluators.iter().any(|evaluator| evaluator == EVALUATOR),
        "the inner handler belongs to a different family"
    );
}

#[test]
fn initializer_requires_one_counter_property_identity() {
    for source in [
        // Read a different property than the one initialized and advanced.
        WORKER_INIT.replace("Number(self.__wx)", "Number(self.__wy)"),
        // Advance a different property than the one read.
        WORKER_INIT.replace("self.__wx=sequence+1", "self.__wy=sequence+1"),
        // Initialize a different property than the one read later.
        WORKER_INIT.replace("self.__wx=0", "self.__wy=0"),
        // Computed counter reads/writes are not the proved static protocol.
        WORKER_INIT.replace("Number(self.__wx)", "Number(self[\"__wx\"])"),
        WORKER_INIT.replace("self.__wx=sequence+1", "self[\"__wx\"]=sequence+1"),
        // Case/name differences are different property identities.
        WORKER_INIT.replace("Number(self.__wx)", "Number(self.__WX)"),
    ] {
        denied(&source);
    }
}

#[test]
fn initializer_requires_global_self_and_unshadowed_number() {
    for source in [
        // `self` is a formal, not the unshadowed worker global.
        WORKER_INIT.replace("function main(){", "function main(self){"),
        // `self` is declared in the initializer's own scope.
        WORKER_INIT.replace(
            "function $THREAD_1_120507(){",
            "function $THREAD_1_120507(){let self={};",
        ),
        // `Number` is shadowed by a local binding.
        WORKER_INIT.replace(
            "function main(){",
            "function main(){let Number=function(x){return x;};",
        ),
        // A different conversion operation is not `Number`.
        WORKER_INIT.replace("Number(self.__wx)", "parseInt(self.__wx)"),
        WORKER_INIT.replace("Number(self.__wx)", "Number.parseInt(self.__wx)"),
        // `Number` takes exactly the counter read.
        WORKER_INIT.replace("Number(self.__wx)", "Number(self.__wx,0)"),
        // A different global root is not the proved `self`.
        WORKER_INIT
            .replace("self.__wx", "globalThis.__wx")
            .replace("self.onmessage", "globalThis.onmessage"),
    ] {
        denied(&source);
    }
}

#[test]
fn initializer_requires_exact_indices_and_argument_order() {
    for (from, to) in [
        (
            "vmCreateThread(message.data[3],message.data[1],message.data[0])",
            "vmCreateThread(message.data[1],message.data[3],message.data[0])",
        ),
        (
            "vmCreateThread(message.data[3],message.data[1],message.data[0])",
            "vmCreateThread(message.data[3],message.data[0],message.data[1])",
        ),
        (
            "vmCreateThread(message.data[3],message.data[1],message.data[0])",
            "vmCreateThread(message.data[3],message.data[2],message.data[0])",
        ),
        (
            "vmCreateThread(message.data[3],message.data[1],message.data[0])",
            "vmCreateThread(message.data[3],message.data[1])",
        ),
        (
            "vmCreateThread(message.data[3],message.data[1],message.data[0])",
            "vmCreateThread(message.data[3],message.data[1],message.data[0],message.data[2])",
        ),
        (
            "(sequence,message.data[2])",
            "(message.data[2],sequence)",
        ),
        (
            "(sequence,message.data[2])",
            "(sequence+1,message.data[2])",
        ),
        (
            "(sequence,message.data[2])",
            "(sequence,message.data[3])",
        ),
        (
            "(sequence,message.data[2])",
            "(sequence)",
        ),
        (
            "(sequence,message.data[2])",
            "(sequence,message.data[2],0)",
        ),
        // `.data` is a static protocol member.
        (
            "message.data[3]",
            "message[\"data\"][3]",
        ),
    ] {
        denied(&WORKER_INIT.replace(from, to));
    }
}

#[test]
fn initializer_requires_an_unbound_consumer_factory() {
    for source in [
        // The factory is shadowed by a local binding.
        WORKER_INIT.replace(
            "function main(){",
            "function main(){let vmCreateThread=function(){return function(){};};",
        ),
        // A member expression would assert a receiver/static identity that the
        // consumer protocol does not prove.
        WORKER_INIT.replace("vmCreateThread(message", "host.vmCreateThread(message"),
        // A nested function declaration is a resolved local callee, not the
        // provided unbound factory input.
        WORKER_INIT.replace(
            "function main(){",
            "function main(){function vmCreateThread(){return function(){};}",
        ),
    ] {
        denied(&source);
    }
}

#[test]
fn initializer_requires_the_whole_parent_and_child_shape() {
    for (from, to) in [
        // Parent shape.
        ("self.__wx=0;", "self.__wx=0;other();"),
        ("self.__wx=0;", "self.__wx=1;"),
        (
            "self.onmessage=$THREAD_4_82974;",
            "self.onmessage=$THREAD_4_82974;other();",
        ),
        (
            "self.onmessage=$THREAD_4_82974;",
            "self.onmessage=$THREAD_4_82974;return;",
        ),
        (
            "self.onmessage=$THREAD_4_82974;",
            "self.onmessage=other;",
        ),
        (
            "self.onmessage=$THREAD_4_82974;",
            "self[\"onmessage\"]=$THREAD_4_82974;",
        ),
        (
            "self.onmessage=$THREAD_4_82974;",
            "self.onmessage=$THREAD_4_82974.bind(null);",
        ),
        (
            "function $THREAD_4_82974(message){",
            "function $THREAD_4_82974(message,extra){",
        ),
        (
            "function $THREAD_4_82974(message){",
            "function $THREAD_4_82974(message=other){",
        ),
        (
            "function $THREAD_4_82974(message){",
            "function $THREAD_4_82974(...message){",
        ),
        (
            "function $THREAD_4_82974(message){",
            "function $THREAD_4_82974({message}){",
        ),
        (
            "function $THREAD_4_82974(message){",
            "async function $THREAD_4_82974(message){",
        ),
        (
            "function $THREAD_4_82974(message){",
            "function* $THREAD_4_82974(message){",
        ),
        (
            "function $THREAD_1_120507(){",
            "async function $THREAD_1_120507(){",
        ),
        (
            "function $THREAD_1_120507(){",
            "function* $THREAD_1_120507(){",
        ),
        (
            "function $THREAD_1_120507(){",
            "function $THREAD_1_120507(extra){",
        ),
        // Child body/return shape.
        ("self.__wx=sequence+1;", "self.__wx=sequence+2;"),
        ("self.__wx=sequence+1;", "self.__wx=other+1;"),
        ("sequence,message.data[2]", "other,message.data[2]"),
        (
            "vmCreateThread(message.data[3],message.data[1],message.data[0])(sequence,message.data[2]);",
            "return vmCreateThread(message.data[3],message.data[1],message.data[0])(sequence,message.data[2]);",
        ),
        (
            "vmCreateThread(message.data[3],message.data[1],message.data[0])(sequence,message.data[2]);",
            "vmCreateThread(message.data[3],message.data[1],message.data[0])(sequence,message.data[2]);return sequence;",
        ),
        (
            "vmCreateThread(message.data[3],message.data[1],message.data[0])(sequence,message.data[2]);",
            "void vmCreateThread(message.data[3],message.data[1],message.data[0])(sequence,message.data[2]);",
        ),
        (
            "let sequence=Number(self.__wx);",
            "let sequence=Number(self.__wx);sequence=other;",
        ),
        (
            "let sequence=Number(self.__wx);",
            "let sequence=Number(self.__wx);let sequence;",
        ),
        ("let sequence=Number(self.__wx);", "let sequence;"),
    ] {
        denied(&WORKER_INIT.replace(from, to));
    }
}

#[test]
fn initializer_preserves_human_names_and_collision_guards() {
    let human = WORKER_INIT.replace("$THREAD_1_120507", NAME);
    let result = name_threads(&human, true).unwrap();
    assert!(result.source.contains(&format!("function {NAME}(")));
    assert!(!result.report.outcomes.iter().any(|outcome| {
        outcome.binding.name == NAME && outcome.applied
    }));

    for collision in [
        WORKER_INIT.replace(
            "function main(){",
            "function main(){let initWorkerMessageDispatcher=17;useValue(initWorkerMessageDispatcher);",
        ),
        WORKER_INIT.replace(
            "function main(){",
            "function main(){function initWorkerMessageDispatcher(){}useValue(initWorkerMessageDispatcher);",
        ),
    ] {
        assert!(parses(&collision), "{collision}");
        assert!(!named(&collision, NAME), "{collision}");
    }
}

#[test]
fn initializer_edits_are_identifier_only_replacements() {
    // The integrated source already names the nested handler through the existing worker
    // family. Replay that same source so the assertion stays scoped to the outer initializer
    // identifier edit.
    let source = WORKER_INIT.replace("$THREAD_4_82974", "handleWorkerMessage");
    let result = name_threads(&source, true).unwrap();
    let applied: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(NAME))
        .collect();
    assert_eq!(applied.len(), 1, "expected exactly one applied target");
    let mut replay = source;
    for edit in result.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}
