use shape::naming::name_threads;

// Fast-loop family: custom-event exchange teardown.
const EVALUATOR: &str = "posthost_exchange_teardown";
const NAME: &str = "emitExchangeTeardown";
const TARGET: &str = "$THREAD_4_48667";

// The teardown is an owned nested function. Its captured handler resolves to itself, the
// timer is captured, and the guarded exchange reports the captured output arrays.
const POSITIVE: &str = r#"function host(){
let $KEY_NAMES = ["x", "y"];
let $KEY_HANDLER, $KEY_TIMER, $KEY_START, $KEY_BASE, $KEY_OUT_A, $KEY_OUT_B, $KEY_PAYLOAD, $KEY_RUNTIME;
$KEY_TIMER = 0;
$KEY_START = 0;
$KEY_BASE = 0;
$KEY_OUT_A = [];
$KEY_OUT_B = [];
$KEY_PAYLOAD = "payload";
$KEY_RUNTIME = { teardown: function(){ } };
function $THREAD_4_48667($EVENT){
let i = 0;
for (; i < $KEY_NAMES.length; i++){
removeEventListener($KEY_NAMES[i], $KEY_HANDLER, true);
}
clearTimeout($KEY_TIMER);
const $ELAPSED = $KEY_START ? $KEY_BASE + (Date.now() - $KEY_START) / 1e3 / 60 : -1;
if ($EVENT.detail != null && typeof $EVENT.detail.exchange === "function"){
$KEY_OUT_A.push("AMDhPm2hAQAAIcDwrxM9XSMwv2VVDRkLZGQXV4qbI8cYSwQ2zSH7OSpPjqm-");
$KEY_OUT_B.push($KEY_PAYLOAD);
$EVENT.detail.exchange($KEY_OUT_A, $KEY_OUT_B, $ELAPSED);
}
$KEY_RUNTIME.teardown();
}
$KEY_HANDLER = $THREAD_4_48667;
return $THREAD_4_48667;
}
host();"#;

// Alpha/spelling variant: every local and captured spelling changed, the
// capture-flag loop uses an expression update, the guard is reordered, the
// token is a captured binding, and the elapsed value is a `let`. Only symbol
// identity may decide the family.
const ALPHA: &str = r#"function host(){
var H, NAMES, TIMER, START, BASE, OUT_A, OUT_B, PAYLOAD, TOKEN, RUNTIME;
function $THREAD_90_777_2($E){
for (let k = 0; k < NAMES.length; k = k + 1){
removeEventListener(NAMES[k], H, true);
}
clearTimeout(TIMER);
let M = START ? BASE + (Date.now() - START) / 1e3 / 60 : -1;
if (typeof $E.detail.exchange === "function" && $E.detail != null){
OUT_A.push(TOKEN);
OUT_B.push(PAYLOAD);
$E.detail.exchange(OUT_A, OUT_B, M);
}
RUNTIME.teardown();
}
H = $THREAD_90_777_2;
NAMES = ["a", "b"]; TIMER = 0; START = 5; BASE = 2; OUT_A = []; OUT_B = []; PAYLOAD = {}; TOKEN = "tok"; RUNTIME = { teardown: function(){} };
return $THREAD_90_777_2;
}
host();"#;

// Extra statements never decide the family: the proven pieces are still the
// function's own direct statements.
const INTERLEAVED: &str = r#"function host(){
let $KEY_NAMES = ["x", "y"];
let $KEY_HANDLER, $KEY_TIMER, $KEY_START, $KEY_BASE, $KEY_OUT_A, $KEY_OUT_B, $KEY_PAYLOAD, $KEY_RUNTIME;
$KEY_TIMER = 0;
$KEY_START = 0;
$KEY_BASE = 0;
$KEY_OUT_A = [];
$KEY_OUT_B = [];
$KEY_PAYLOAD = "payload";
$KEY_RUNTIME = { teardown: function(){ } };
function $THREAD_4_48667($EVENT){
let i = 0;
let $KEY_SCRATCH = 1;
console.log($KEY_NAMES[0], $KEY_SCRATCH);
for (; i < $KEY_NAMES.length; i++){
removeEventListener($KEY_NAMES[i], $KEY_HANDLER, true);
}
clearTimeout($KEY_TIMER);
const $ELAPSED = $KEY_START ? $KEY_BASE + (Date.now() - $KEY_START) / 1e3 / 60 : -1;
if ($EVENT.detail != null && typeof $EVENT.detail.exchange === "function"){
$KEY_OUT_A.push("AMDhPm2hAQAAIcDwrxM9XSMwv2VVDRkLZGQXV4qbI8cYSwQ2zSH7OSpPjqm-");
$KEY_OUT_B.push($KEY_PAYLOAD);
$EVENT.detail.exchange($KEY_OUT_A, $KEY_OUT_B, $ELAPSED);
}
$KEY_RUNTIME.teardown();
}
$KEY_HANDLER = $THREAD_4_48667;
return $THREAD_4_48667;
}
host();"#;

// The output arrays are locals of the teardown, so they are not captured.
const LOCAL_OUTPUT: &str = r#"function host(){
let $KEY_NAMES = ["x", "y"];
let $KEY_HANDLER, $KEY_TIMER, $KEY_START, $KEY_BASE, $KEY_PAYLOAD, $KEY_RUNTIME;
$KEY_TIMER = 0;
$KEY_START = 0;
$KEY_BASE = 0;
$KEY_PAYLOAD = "payload";
$KEY_RUNTIME = { teardown: function(){ } };
function $THREAD_4_48667($EVENT){
let i = 0;
let $KEY_OUT_A = [];
let $KEY_OUT_B = [];
for (; i < $KEY_NAMES.length; i++){
removeEventListener($KEY_NAMES[i], $KEY_HANDLER, true);
}
clearTimeout($KEY_TIMER);
const $ELAPSED = $KEY_START ? $KEY_BASE + (Date.now() - $KEY_START) / 1e3 / 60 : -1;
if ($EVENT.detail != null && typeof $EVENT.detail.exchange === "function"){
$KEY_OUT_A.push("AMDhPm2hAQAAIcDwrxM9XSMwv2VVDRkLZGQXV4qbI8cYSwQ2zSH7OSpPjqm-");
$KEY_OUT_B.push($KEY_PAYLOAD);
$EVENT.detail.exchange($KEY_OUT_A, $KEY_OUT_B, $ELAPSED);
}
$KEY_RUNTIME.teardown();
}
$KEY_HANDLER = $THREAD_4_48667;
return $THREAD_4_48667;
}
host();"#;

// The owned teardown is nested in a container; the container must not borrow
// the descendant's evidence.
const NESTED: &str = r#"function host(){
let $KEY_NAMES = ["x", "y"];
function $THREAD_OUTER($EVENT){
let $KEY_HANDLER, $KEY_TIMER, $KEY_START, $KEY_BASE, $KEY_OUT_A, $KEY_OUT_B, $KEY_PAYLOAD, $KEY_RUNTIME;
$KEY_TIMER = 0;
$KEY_START = 0;
$KEY_BASE = 0;
$KEY_OUT_A = [];
$KEY_OUT_B = [];
$KEY_PAYLOAD = "payload";
$KEY_RUNTIME = { teardown: function(){ } };
function $THREAD_4_48667($E2){
let i = 0;
for (; i < $KEY_NAMES.length; i++){
removeEventListener($KEY_NAMES[i], $KEY_HANDLER, true);
}
clearTimeout($KEY_TIMER);
const $ELAPSED = $KEY_START ? $KEY_BASE + (Date.now() - $KEY_START) / 1e3 / 60 : -1;
if ($E2.detail != null && typeof $E2.detail.exchange === "function"){
$KEY_OUT_A.push("AMDhPm2hAQAAIcDwrxM9XSMwv2VVDRkLZGQXV4qbI8cYSwQ2zSH7OSpPjqm-");
$KEY_OUT_B.push($KEY_PAYLOAD);
$E2.detail.exchange($KEY_OUT_A, $KEY_OUT_B, $ELAPSED);
}
$KEY_RUNTIME.teardown();
}
$KEY_HANDLER = $THREAD_4_48667;
return $THREAD_4_48667;
}
return $THREAD_OUTER;
}
host();"#;

fn named(source: &str, target: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.outcomes.iter().any(|outcome| {
            outcome.applied
                && outcome.binding.name == target
                && outcome.proposed_name.as_deref() == Some(NAME)
        })
    })
}

fn our_claim(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.function_roles.iter().any(|role| {
            role.evaluators.iter().any(|evaluator| evaluator == EVALUATOR)
        })
    })
}

fn claimed(source: &str) -> Vec<String> {
    name_threads(source, true)
        .map(|result| {
            result
                .report
                .function_roles
                .iter()
                .filter(|role| role.evaluators.iter().any(|e| e == EVALUATOR))
                .filter_map(|role| role.current_name.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn parses(source: &str) -> bool {
    name_threads(source, true).is_ok()
}

fn denied(source: &str, target: &str) {
    assert!(parses(source), "fixture did not parse: {source}");
    assert!(!named(source, target), "unexpected {NAME} for: {source}");
    assert!(
        !our_claim(source),
        "unexpected exchange-teardown claim for: {source}"
    );
}

fn refuses(template: &str, target: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        denied(&template.replace(from, to), target);
    }
}

#[test]
fn accepts_portable_family_shapes() {
    let literal_timer = POSITIVE.replace("clearTimeout($KEY_TIMER);", "clearTimeout(0);");
    for (label, source, target) in [
        ("positive", POSITIVE, TARGET),
        ("clean-console-stack-access-flags-proxy-tamper-flags-layouts", literal_timer.as_str(), TARGET),
        ("alpha", ALPHA, "$THREAD_90_777_2"),
        ("interleaved", INTERLEAVED, TARGET),
    ] {
        assert!(parses(source), "{label} did not parse");
        assert!(named(source, target), "{label} was not named");
        assert!(our_claim(source), "{label} was not our claim");
    }
}

#[test]
fn claims_only_the_owned_teardown() {
    let result = name_threads(POSITIVE, true).unwrap();
    let claimed: Vec<_> = result
        .report
        .function_roles
        .iter()
        .filter(|role| role.evaluators.iter().any(|e| e == EVALUATOR))
        .collect();
    assert_eq!(claimed.len(), 1, "expected exactly the owned teardown");
    assert_eq!(claimed[0].current_name.as_deref(), Some(TARGET));
}

#[test]
fn denies_broken_loop_timer_and_elapsed() {
    refuses(
        POSITIVE,
        TARGET,
        &[
            // The capture flag is lowered away.
            (
                "removeEventListener($KEY_NAMES[i], $KEY_HANDLER, true)",
                "removeEventListener($KEY_NAMES[i], $KEY_HANDLER, false)",
            ),
            // The removed handler is a fresh anonymous function, not this one.
            (
                "$KEY_HANDLER = $THREAD_4_48667;",
                "$KEY_HANDLER = function(){ };",
            ),
            // The loop removes a different name list.
            ("$KEY_NAMES[i], $KEY_HANDLER", "$KEY_OUT_A[i], $KEY_HANDLER"),
            // The timer is a local of the teardown, not a captured binding.
            ("clearTimeout($KEY_TIMER);", "clearTimeout($ELAPSED);"),
            // clearTimeout disappears.
            ("clearTimeout($KEY_TIMER);", ""),
            // The elapsed minutes stop reading the shared captured start.
            (
                "(Date.now() - $KEY_START)",
                "(Date.now() - $KEY_BASE)",
            ),
            // The clock is not Date.now().
            ("Date.now()", "$KEY_START"),
            // The ternary fallback is not -1.
            (": -1;", ": 0;"),
        ],
    );
}

#[test]
fn denies_broken_exchange_guard_and_arrays() {
    refuses(
        POSITIVE,
        TARGET,
        &[
            // The detail key is not `exchange`.
            (
                "typeof $EVENT.detail.exchange === \"function\"",
                "typeof $EVENT.detail.callback === \"function\"",
            ),
            (
                "$EVENT.detail.exchange($KEY_OUT_A, $KEY_OUT_B, $ELAPSED);",
                "$EVENT.detail.callback($KEY_OUT_A, $KEY_OUT_B, $ELAPSED);",
            ),
            // The guard is replaced by an unconditional body.
            (
                "$EVENT.detail != null && typeof $EVENT.detail.exchange === \"function\"",
                "true",
            ),
            // The detail object is a captured payload rather than the formal.
            ("$EVENT.detail != null", "$KEY_PAYLOAD.detail != null"),
            // The exchange call swaps the exact output arrays.
            (
                "$EVENT.detail.exchange($KEY_OUT_A, $KEY_OUT_B, $ELAPSED);",
                "$EVENT.detail.exchange($KEY_OUT_B, $KEY_OUT_A, $ELAPSED);",
            ),
            // The elapsed value is not forwarded.
            (
                "$EVENT.detail.exchange($KEY_OUT_A, $KEY_OUT_B, $ELAPSED);",
                "$EVENT.detail.exchange($KEY_OUT_A, $KEY_OUT_B, -1);",
            ),
            // Both pushes target the same output array.
            (
                "$KEY_OUT_B.push($KEY_PAYLOAD);",
                "$KEY_OUT_A.push($KEY_PAYLOAD);",
            ),
            // The payload is a local, not a captured binding.
            ("$KEY_OUT_B.push($KEY_PAYLOAD);", "$KEY_OUT_B.push($ELAPSED);"),
            // The token is a descendant call rather than opaque captured data.
            (
                "$KEY_OUT_A.push(\"AMDhPm2hAQAAIcDwrxM9XSMwv2VVDRkLZGQXV4qbI8cYSwQ2zSH7OSpPjqm-\");",
                "$KEY_OUT_A.push(makeToken());",
            ),
            // A push disappears.
            ("$KEY_OUT_B.push($KEY_PAYLOAD);", ""),
        ],
    );
    denied(&LOCAL_OUTPUT, TARGET);
}

#[test]
fn denies_wrong_captures_and_runtime() {
    refuses(
        POSITIVE,
        TARGET,
        &[
            // The runtime is the formal, not a captured binding.
            ("$KEY_RUNTIME.teardown();", "$EVENT.teardown();"),
            // A trailing statement means the captured teardown is not last.
            (
                "$KEY_RUNTIME.teardown();",
                "$KEY_RUNTIME.teardown();\n$KEY_OUT_A.push($KEY_PAYLOAD);",
            ),
            // The teardown call takes an argument.
            ("$KEY_RUNTIME.teardown();", "$KEY_RUNTIME.teardown($KEY_TIMER);"),
            // The runtime member is not `teardown`.
            ("$KEY_RUNTIME.teardown();", "$KEY_RUNTIME.stop();"),
        ],
    );
    // A captured helper removes a *different* owned function.
    let foreign_handler = POSITIVE
        .replace(
            "$KEY_HANDLER = $THREAD_4_48667;",
            "function $THREAD_9_1(){ }\n$KEY_HANDLER = $THREAD_9_1;",
        );
    denied(&foreign_handler, TARGET);
}

#[test]
fn denies_shadowed_host_globals() {
    for declaration in [
        "let removeEventListener = function(){ };",
        "let clearTimeout = function(){ };",
        "let Date = { now: function(){ return 0; } };",
    ] {
        let shadowed = POSITIVE.replace(
            "function host(){\n",
            &format!("function host(){{\n{declaration}\n"),
        );
        denied(&shadowed, TARGET);
    }
}

#[test]
fn a_container_borrows_no_nested_teardown_evidence() {
    assert!(parses(NESTED), "nested fixture did not parse");
    assert_eq!(
        claimed(NESTED),
        vec![TARGET.to_string()],
        "only the nested owned teardown may be claimed"
    );
    assert!(named(NESTED, TARGET));
    assert!(!named(NESTED, "$THREAD_OUTER"));
}

#[test]
fn preserves_human_names_and_collision_guards() {
    let human = POSITIVE.replace(TARGET, NAME);
    let result = name_threads(&human, true).unwrap();
    assert!(result.source.contains(&format!("function {NAME}(")));
    assert!(!result
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.binding.name == NAME && outcome.applied));

    let collision = POSITIVE.replace(
        "function host(){\n",
        &format!("function host(){{\nlet {NAME} = 17; useValue({NAME});\n"),
    );
    assert!(parses(&collision), "{collision}");
    assert!(!named(&collision, TARGET), "{collision}");
}
