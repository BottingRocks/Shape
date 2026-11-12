use shape::naming::name_threads;

const DISTINCT_VECTOR: &str = r#"
function main() {
  function $THREAD_41_1001(source, state) {
    let alias = source;
    if (state.a.some(item => {
      let equal;
      if (alias == null || item == null || alias.length !== item.length) {
        equal = false;
      } else {
        let index = 0;
        for (;;) {
          if (index >= alias.length) { equal = true; break; }
          if (alias[index] === item[index]) { ++index; continue; }
          equal = false;
          break;
        }
      }
      return equal;
    })) return void 0;
    state.b = Number(state.b) + 1;
    if (state.a.length > 5) return void 0;
    else {
      state.a.push(Array.prototype.slice.call(alias));
      return void 0;
    }
  }
  return $THREAD_41_1001;
}
main();
"#;

const VISIBILITY: &str = r#"
function main() {
  let counter = 0;
  let samples = { put(value) { return value; } };
  let clock = function () { return 12; };
  let reporter = { error(value) { return value; } };
  let seen = false;
  function $THREAD_42_1002(event) {
    try {
      ++counter;
      let receiver = samples;
      let put = samples.put;
      put.call(receiver, {
        visible: event.target.visibilityState === "visible",
        timestamp: clock()
      });
      seen = true;
    } catch (error) {
      reporter.error(error);
    }
  }
  return $THREAD_42_1002;
}
main();
"#;

fn applied(source: &str, expected: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        let evaluator = match expected {
            "recordPixelVectorSample" => "graphics_distinct_vector_sample",
            "recordVisibilityChangeSample" => "shared_anchor_visibility_publish",
            _ => return false,
        };
        result.report.function_roles.iter().any(|role| {
            role.candidates
                .iter()
                .any(|candidate| candidate == expected)
                && role
                    .evaluators
                    .iter()
                    .any(|candidate| candidate == evaluator)
        }) && result
            .report
            .outcomes
            .iter()
            .any(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(expected))
    })
}

fn refused(source: &str, expected: &str) -> bool {
    let result =
        name_threads(source, true).unwrap_or_else(|error| panic!("fixture did not parse: {error}"));
    !applied(source, expected)
}

#[test]
fn distinct_vector_recorder_is_named_and_alpha_renaming_is_irrelevant() {
    assert!(applied(DISTINCT_VECTOR, "recordPixelVectorSample"));
    let renamed = DISTINCT_VECTOR
        .replace("$THREAD_41_1001", "$THREAD_900_77")
        .replace("source", "payload")
        .replace("state", "record")
        .replace("alias", "sample")
        .replace("item", "candidate")
        .replace("equal", "same");
    assert!(applied(&renamed, "recordPixelVectorSample"));
}

#[test]
fn distinct_vector_recorder_refuses_meaningful_near_misses() {
    for (from, to) in [
        ("state.a.some", "state.a.every"),
        (
            "alias.length !== item.length",
            "alias.length === item.length",
        ),
        ("Number(state.b) + 1", "Number(state.b) + 2"),
        ("state.a.length > 5", "state.a.length >= 5"),
        (
            "Array.prototype.slice.call(alias)",
            "Array.prototype.concat.call(alias)",
        ),
        (
            "if (alias[index] === item[index])",
            "if (alias[index] !== item[index])",
        ),
        ("for (;;)", "for (; index < 10;)"),
    ] {
        assert!(DISTINCT_VECTOR.contains(from));
        assert!(
            refused(
                &DISTINCT_VECTOR.replace(from, to),
                "recordPixelVectorSample"
            ),
            "accepted {from} -> {to}"
        );
    }
    let mutated = DISTINCT_VECTOR.replace(
        "let alias = source;",
        "let alias = source;\n    alias = state.a;",
    );
    assert!(refused(&mutated, "recordPixelVectorSample"));
    let destructured = DISTINCT_VECTOR.replace("item =>", "({ value: item }) =>");
    assert!(refused(&destructured, "recordPixelVectorSample"));
    // A comparison that reads a DIFFERENT declared index than the initialized
    // scan cursor is not the claimed complete element-by-element scan.
    let separate_index = DISTINCT_VECTOR
        .replace("function main() {", "function main() {\n  let other = 0;")
        .replace(
            "if (alias[index] === item[index])",
            "if (alias[other] === item[index])",
        )
        .replace("++index; continue;", "++other; continue;");
    assert!(refused(&separate_index, "recordPixelVectorSample"));
    // The end guard must not carry an uninspected else branch.
    let end_else = DISTINCT_VECTOR.replace(
        "if (index >= alias.length) { equal = true; break; }",
        "if (index >= alias.length) { equal = true; break; } else { equal = false; }",
    );
    assert!(refused(&end_else, "recordPixelVectorSample"));
}

#[test]
fn visibility_publisher_is_named_and_requires_owned_capture_identities() {
    assert!(applied(VISIBILITY, "recordVisibilityChangeSample"));
    let renamed = VISIBILITY
        .replace("$THREAD_42_1002", "$THREAD_77_4")
        .replace("event", "input")
        .replace("samples", "bucket")
        .replace("receiver", "target")
        .replace("reporter", "sink")
        .replace("seen", "recorded");
    assert!(applied(&renamed, "recordVisibilityChangeSample"));
    for (from, to) in [
        (
            "visibilityState === \"visible\"",
            "visibilityState !== \"visible\"",
        ),
        ("timestamp: clock()", "timestamp: clock(1)"),
        ("put.call(receiver", "put.call(samples"),
        ("reporter.error(error)", "reporter.warn(error)"),
        ("++counter", "counter += 1"),
    ] {
        assert!(VISIBILITY.contains(from));
        assert!(
            refused(
                &VISIBILITY.replace(from, to),
                "recordVisibilityChangeSample"
            ),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn visibility_publisher_refuses_non_callable_and_wrong_clock_origins() {
    let declaration = "let clock = function () { return 12; };";
    let practice = "clock = function () { return 12; };";
    assert!(VISIBILITY.contains(declaration));

    let noncallable = VISIBILITY.replace(declaration, "let clock = 0;");
    assert!(refused(&noncallable, "recordVisibilityChangeSample"));

    let bare = VISIBILITY.replace(declaration, "let clock;");
    assert!(refused(&bare, "recordVisibilityChangeSample"));

    let future = VISIBILITY.replace(declaration, "let clock;").replace(
        "return $THREAD_42_1002;",
        &format!("{practice}\n  return $THREAD_42_1002;"),
    );
    assert!(
        refused(&future, "recordVisibilityChangeSample"),
        "accepted a clock assigned after the use site"
    );

    let conditional = VISIBILITY.replace(
        declaration,
        &format!("let clock;\n  if (true) {{ {practice} }}"),
    );
    assert!(
        refused(&conditional, "recordVisibilityChangeSample"),
        "accepted a conditional clock origin"
    );

    let setup_callback = VISIBILITY.replace(
        declaration,
        &format!("let clock;\n  register(function () {{ {practice} }});"),
    );
    assert!(
        refused(&setup_callback, "recordVisibilityChangeSample"),
        "accepted a setup-callback clock origin"
    );

    let loop_body = VISIBILITY.replace(
        declaration,
        &format!("let clock;\n  for (let i = 0; i < 1; i++) {{ {practice} }}"),
    );
    assert!(
        refused(&loop_body, "recordVisibilityChangeSample"),
        "accepted a loop-body clock origin"
    );

    let resource = VISIBILITY.replace(declaration, "using clock = makeClock();");
    assert!(
        refused(&resource, "recordVisibilityChangeSample"),
        "accepted a resource clock binding"
    );

    let member_clock = VISIBILITY.replace("timestamp: clock()", "timestamp: clockSource.now()");
    assert!(
        refused(&member_clock, "recordVisibilityChangeSample"),
        "assumed an unknown member clock producer is native"
    );
}

#[test]
fn visibility_publisher_refuses_resource_receiver_binding() {
    let resource_receiver = VISIBILITY.replace(
        "let samples = { put(value) { return value; } };",
        "using samples = makeSamples();",
    );
    assert!(
        refused(&resource_receiver, "recordVisibilityChangeSample"),
        "accepted a resource API receiver"
    );
}

#[test]
fn meaningful_names_and_role_collisions_are_preserved() {
    let human = DISTINCT_VECTOR.replace(
        "function $THREAD_41_1001(source, state)",
        "function recordPixelVectorSample(source, state)",
    );
    assert_eq!(name_threads(&human, true).unwrap().source, human);
    let collision = DISTINCT_VECTOR.replace(
        "function main() {",
        "function main() { function recordPixelVectorSample() {}",
    );
    assert!(refused(&collision, "recordPixelVectorSample"));
}
