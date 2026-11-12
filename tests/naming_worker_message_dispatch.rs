use shape::naming::name_threads;

const ANIMATION_FRAME: &str = r#"
function main() {
  let frames = [];
  let active = false;
  let interval = 16;
  let context = { requestAnimationFrame(callback) { return callback; } };
  let rearm;
  function $THREAD_31_1(timestamp) {
    frames.push(Math.round(timestamp / interval));
    active = true;
    if (frames.length < 20) {
      context.requestAnimationFrame(rearm);
    }
  }
  rearm = $THREAD_31_1;
  context.requestAnimationFrame($THREAD_31_1);
  return $THREAD_31_1;
}
main();
"#;

const WORKER_MESSAGE: &str = r#"
function main() {
  function $THREAD_32_1() {
    self.__wx = 0;
    function $THREAD_32_2(message) {
      let seq = Number(self.__wx);
      self.__wx = seq + 1;
      vmCreateThread(message.data[3], message.data[1], message.data[0])(seq, message.data[2]);
    }
    self.onmessage = $THREAD_32_2;
  }
  return $THREAD_32_1;
}
main();
"#;

fn strict_animation_frame() -> String {
    ANIMATION_FRAME
        .replace("    active = true;\n", "")
        .replace("  let rearm;", "  {\n  let rearm;")
        .replace("  return $THREAD_31_1;", "  }\n  return frames;")
}

fn contract(expected: &str) -> (&'static str, &'static str) {
    match expected {
        "handleWorkerMessage" => ("runtime_worker_message_dispatch", "$THREAD_32_2"),
        "recordAnimationFrameSample" => ("graphics_animation_frame_sample", "$THREAD_31_1"),
        "sampleNormalizedAnimationFrame" => ("shared_browser_frame_sampler", "$THREAD_31_1"),
        _ => panic!("unknown test contract: {expected}"),
    }
}

fn applied_to(source: &str, expected: &str, callable: &str) -> bool {
    let (evaluator, _) = contract(expected);
    let result =
        name_threads(source, true).unwrap_or_else(|error| panic!("fixture did not parse: {error}"));
    result.report.outcomes.iter().any(|outcome| {
        outcome.binding.name == callable
            && outcome.applied
            && outcome.evaluator.as_deref() == Some(evaluator)
            && outcome.role_name.as_deref() == Some(expected)
            && outcome.proposed_name.as_deref() == Some(expected)
            && result.report.function_roles.iter().any(|role| {
                role.function_span.start == outcome.binding.declaration.start
                    && role.function_span.end == outcome.binding.declaration.end
                    && role
                        .candidates
                        .iter()
                        .any(|candidate| candidate == expected)
                    && role
                        .evaluators
                        .iter()
                        .any(|candidate| candidate == evaluator)
            })
    })
}

fn applied(source: &str, expected: &str) -> bool {
    applied_to(source, expected, contract(expected).1)
}

fn refused_with_donor(source: &str, expected: &str, donor: &str) -> bool {
    assert!(
        applied(donor, expected),
        "missing registered positive donor for {expected}"
    );
    !applied(source, expected)
}

fn refused(source: &str, expected: &str) -> bool {
    let donor = match expected {
        "handleWorkerMessage" => WORKER_MESSAGE.to_string(),
        "recordAnimationFrameSample" => ANIMATION_FRAME.to_string(),
        "sampleNormalizedAnimationFrame" => strict_animation_frame(),
        _ => panic!("unknown test contract: {expected}"),
    };
    refused_with_donor(source, expected, &donor)
}

#[test]
fn animation_frame_timing_sampler_is_named_and_alpha_renaming_is_irrelevant() {
    assert!(applied(ANIMATION_FRAME, "recordAnimationFrameSample"));
    let renamed = ANIMATION_FRAME
        .replace("$THREAD_31_1", "$THREAD_900_77")
        .replace("frames", "samples")
        .replace("active", "touched")
        .replace("interval", "duration")
        .replace("context", "runtime")
        .replace("rearm", "again")
        .replace("timestamp", "frameTime");
    assert!(applied_to(
        &renamed,
        "recordAnimationFrameSample",
        "$THREAD_900_77"
    ));
    // The seed registers the sampler from an explicit block (an `else` block),
    // so cover that enclosing-statement-list shape as well as a function body.
    let block = ANIMATION_FRAME
        .replace("  let rearm;", "  {\n  let rearm;")
        .replace("  return $THREAD_31_1;", "  }\n  return frames;");
    assert!(applied(&block, "recordAnimationFrameSample"));
}

#[test]
fn animation_frame_sampler_refuses_changed_operations_and_registration() {
    for (from, to) in [
        ("frames.push", "frames.unshift"),
        (
            "Math.round(timestamp / interval)",
            "Math.floor(timestamp / interval)",
        ),
        ("frames.length < 20", "frames.length <= 20"),
        ("active = true;", "active = false;"),
        (
            "context.requestAnimationFrame(rearm)",
            "context.setTimeout(rearm)",
        ),
    ] {
        assert!(ANIMATION_FRAME.contains(from), "fixture lacks {from}");
        assert!(
            refused(
                &ANIMATION_FRAME.replace(from, to),
                "recordAnimationFrameSample"
            ),
            "accepted {from} -> {to}"
        );
    }
    let no_registration =
        ANIMATION_FRAME.replace("  context.requestAnimationFrame($THREAD_31_1);\n", "");
    assert!(applied(&no_registration, "recordAnimationFrameSample"));
    let strict = strict_animation_frame();
    assert!(refused(
        &strict.replace("  context.requestAnimationFrame($THREAD_31_1);\n", ""),
        "sampleNormalizedAnimationFrame"
    ));
    let extra_return = ANIMATION_FRAME.replace(
        "    if (frames.length < 20) {",
        "    return;\n    if (frames.length < 20) {",
    );
    assert!(refused(&extra_return, "recordAnimationFrameSample"));
}

#[test]
fn animation_frame_sampler_refuses_changed_formals_and_alias_mutation() {
    let defaulted = ANIMATION_FRAME.replace(
        "function $THREAD_31_1(timestamp)",
        "function $THREAD_31_1(timestamp = 0)",
    );
    assert!(refused(&defaulted, "recordAnimationFrameSample"));
    let destructured = ANIMATION_FRAME.replace(
        "function $THREAD_31_1(timestamp)",
        "function $THREAD_31_1({ value: timestamp })",
    );
    assert!(refused(&destructured, "recordAnimationFrameSample"));
    let mutated = ANIMATION_FRAME.replace(
        "  rearm = $THREAD_31_1;",
        "  rearm = $THREAD_31_1;\n  rearm = function () {};",
    );
    assert!(applied(&mutated, "recordAnimationFrameSample"));
    let strict_mutated = strict_animation_frame().replace(
        "  rearm = $THREAD_31_1;",
        "  rearm = $THREAD_31_1;\n  rearm = function () {};",
    );
    assert!(refused(&strict_mutated, "sampleNormalizedAnimationFrame"));
}

#[test]
fn worker_message_dispatch_is_named_and_alpha_renaming_is_irrelevant() {
    assert!(applied(WORKER_MESSAGE, "handleWorkerMessage"));
    let renamed = WORKER_MESSAGE
        .replace("$THREAD_32_1", "$THREAD_901_88")
        .replace("$THREAD_32_2", "$THREAD_902_99")
        .replace("seq", "ticket")
        .replace("$THREAD_902_99(message)", "$THREAD_902_99(event)")
        .replace("message.data", "event.data");
    assert!(applied_to(
        &renamed,
        "handleWorkerMessage",
        "$THREAD_902_99"
    ));
}

#[test]
fn worker_message_dispatch_refuses_changed_counter_and_slots() {
    for (from, to) in [
        ("Number(self.__wx)", "parseInt(self.__wx)"),
        ("self.__wx = seq + 1;", "self.__wx = seq + 2;"),
        ("message.data[3]", "message.data[4]"),
        ("vmCreateThread(message.data[3], message.data[1], message.data[0])(seq, message.data[2])",
         "vmCreateThread(message.data[3], message.data[1], message.data[0])(seq, message.data[3])"),
        ("self.onmessage = $THREAD_32_2;", "self.onmessage = $THREAD_31_1;"),
    ] {
        assert!(WORKER_MESSAGE.contains(from), "fixture lacks {from}");
        assert!(
            refused(&WORKER_MESSAGE.replace(from, to), "handleWorkerMessage"),
            "accepted handle {from} -> {to}"
        );
    }
    let changed_initial = WORKER_MESSAGE.replace("self.__wx = 0;", "self.__wx = 1;");
    assert!(applied(&changed_initial, "handleWorkerMessage"));
}

#[test]
fn worker_message_handler_requires_an_unbound_factory_producer() {
    let local_factory = WORKER_MESSAGE.replace(
        "  function $THREAD_32_1() {",
        "  let vmCreateThread = function () {};\n  function $THREAD_32_1() {",
    );
    assert!(refused(&local_factory, "handleWorkerMessage"));
}

#[test]
fn meaningful_names_and_role_collisions_are_preserved() {
    let human = ANIMATION_FRAME.replace("$THREAD_31_1", "recordAnimationFrameSample");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
    let collision = ANIMATION_FRAME.replace(
        "function main() {",
        "function main() { function recordAnimationFrameSample() {}",
    );
    assert!(refused(&collision, "recordAnimationFrameSample"));

    let handler_collision = WORKER_MESSAGE.replace(
        "  function $THREAD_32_1() {",
        "  function $THREAD_32_1() {\n    function handleWorkerMessage() {}",
    );
    assert!(refused(&handler_collision, "handleWorkerMessage"));
}

#[test]
fn opaque_callback_and_strict_self_registration_have_distinct_contracts() {
    let strict = strict_animation_frame();
    assert!(applied(&strict, "sampleNormalizedAnimationFrame"));
    for (from, to) in [
        ("rearm = $THREAD_31_1;", "rearm = function () {};"),
        (
            "context.requestAnimationFrame($THREAD_31_1);",
            "context.setTimeout($THREAD_31_1);",
        ),
    ] {
        assert!(ANIMATION_FRAME.contains(from));
        assert!(strict.contains(from));
        assert!(applied(
            &ANIMATION_FRAME.replace(from, to),
            "recordAnimationFrameSample"
        ));
        assert!(refused(
            &strict.replace(from, to),
            "sampleNormalizedAnimationFrame"
        ));
    }
}

#[test]
#[should_panic(expected = "missing registered positive donor")]
fn negative_helper_rejects_a_missing_positive_donor() {
    refused_with_donor(
        ANIMATION_FRAME,
        "recordAnimationFrameSample",
        WORKER_MESSAGE,
    );
}

#[test]
#[should_panic(expected = "unknown test contract")]
fn helper_rejects_unknown_evaluator_contracts() {
    applied(ANIMATION_FRAME, "unregisteredRole");
}

#[test]
fn claim_evidence_must_belong_to_the_requested_callable() {
    assert!(applied(ANIMATION_FRAME, "recordAnimationFrameSample"));
    assert!(!applied_to(
        ANIMATION_FRAME,
        "recordAnimationFrameSample",
        "main"
    ));
    assert!(applied(WORKER_MESSAGE, "handleWorkerMessage"));
    assert!(!applied_to(
        WORKER_MESSAGE,
        "handleWorkerMessage",
        "$THREAD_32_1"
    ));
}
