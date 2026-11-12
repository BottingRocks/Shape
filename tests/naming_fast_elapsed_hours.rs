use shape::naming::name_threads;

const ELAPSED_HOURS: &str = r#"function main() {
  let record, Constructor, start;
  Constructor = host.Date;
  start = Number(new Constructor());
  record = { top: 0, seen: 1 };
  function $THREAD_6_72713() {
    record.top = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);
  }
  return $THREAD_6_72713;
}
main();"#;

fn named(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.outcomes.iter().any(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some("updateElapsedHours")
                && outcome.evaluator.as_deref() == Some("shared_elapsed_hours_state_write")
        })
    })
}

fn refused(source: &str) -> bool {
    let result =
        name_threads(source, true).unwrap_or_else(|error| panic!("fixture did not parse: {error}"));
    !result.report.outcomes.iter().any(|outcome| {
        outcome.applied
            && outcome.proposed_name.as_deref() == Some("updateElapsedHours")
            && outcome.evaluator.as_deref() == Some("shared_elapsed_hours_state_write")
    })
}

#[test]
fn captured_elapsed_hours_write_is_named_and_alpha_renaming_is_irrelevant() {
    assert!(named(ELAPSED_HOURS));
    let renamed = ELAPSED_HOURS
        .replace("$THREAD_6_72713", "$THREAD_900_77")
        .replace("record", "metrics")
        .replace("Constructor", "Clock")
        .replace("start", "beginning");
    assert!(named(&renamed), "{renamed}");
}

#[test]
fn setup_assigned_after_the_nested_declaration_is_still_bounded() {
    let late_setup = r#"function main() {
  let record, Constructor, start;
  function $THREAD_6_72713() {
    record.top = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);
  }
  Constructor = host.Date;
  start = Number(new Constructor());
  record = { top: 0 };
  return $THREAD_6_72713;
}
main();"#;
    assert!(named(late_setup), "{late_setup}");
}

#[test]
fn number_alias_and_captured_alias_are_still_proven_semantically() {
    let aliased = ELAPSED_HOURS
        .replace(
            "let record, Constructor, start;",
            "let record, Constructor, start;\n  const $N = Number;\n  const $Clock = host.Date;",
        )
        .replace("Constructor = host.Date;", "Constructor = $Clock;")
        .replace("start = Number(new Constructor());", "start = $N(new Constructor());")
        .replace(
            "Number((Number(new Constructor()) - start)",
            "$N(($N(new Constructor()) - start)",
        );
    assert!(named(&aliased), "{aliased}");
}

#[test]
fn elapsed_hours_write_refuses_changed_arithmetic() {
    for (from, to) in [
        (
            "Number((Number(new Constructor()) - start) / 1e3 / 60 / 60)",
            "Number((Number(new Constructor()) - start) / 1e3 / 60)",
        ),
        (
            "Number((Number(new Constructor()) - start) / 1e3 / 60 / 60)",
            "Number((Number(new Constructor()) - start) / 1e3 / 60 / 24)",
        ),
        (
            "Number((Number(new Constructor()) - start) / 1e3 / 60 / 60)",
            "Number((Number(new Constructor()) + start) / 1e3 / 60 / 60)",
        ),
        (
            "Number((Number(new Constructor()) - start) / 1e3 / 60 / 60)",
            "Number((Number(new Constructor()) - start) / 100 / 60 / 60)",
        ),
        (
            "Number((Number(new Constructor()) - start) / 1e3 / 60 / 60)",
            "(Number(new Constructor()) - start) / 1e3 / 60 / 60",
        ),
        (
            "Number((Number(new Constructor()) - start) / 1e3 / 60 / 60)",
            "Number((new Constructor() - start) / 1e3 / 60 / 60)",
        ),
        (
            "Number((Number(new Constructor()) - start) / 1e3 / 60 / 60)",
            "Number((Number(new Constructor()) - start) / 1e3 / 60 * 60)",
        ),
        (
            "Number((Number(new Constructor()) - start) / 1e3 / 60 / 60)",
            "Number((Number(new Constructor(0)) - start) / 1e3 / 60 / 60)",
        ),
    ] {
        assert!(ELAPSED_HOURS.contains(from), "fixture lacks {from}");
        assert!(
            refused(&ELAPSED_HOURS.replace(from, to)),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn elapsed_hours_write_refuses_non_write_and_wrong_target_shapes() {
    for (from, to) in [
        (
            "record.top = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);",
            "return Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);",
        ),
        (
            "record.top = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);",
            "record.top = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60); return;",
        ),
        (
            "record.top = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);",
            "record[\"top\"] = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);",
        ),
        (
            "record.top = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);",
            "top = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);",
        ),
        (
            "record.top = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);",
            "record.top += Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);",
        ),
        (
            "record.top = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);",
            "record.top = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60) || 0;",
        ),
    ] {
        assert!(ELAPSED_HOURS.contains(from), "fixture lacks {from}");
        assert!(
            refused(&ELAPSED_HOURS.replace(from, to)),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn elapsed_hours_write_requires_bounded_captures() {
    let global_record = ELAPSED_HOURS.replace(
        "  let record, Constructor, start;",
        "  let Constructor, start;",
    );
    assert!(refused(&global_record), "{global_record}");

    let function_constructor = ELAPSED_HOURS
        .replace("  let record, Constructor, start;", "  let record, start;\n  function Constructor() {}")
        .replace("  Constructor = host.Date;\n", "");
    assert!(refused(&function_constructor), "{function_constructor}");

    let literal_start = ELAPSED_HOURS
        .replace("  let record, Constructor, start;", "  let record, Constructor;")
        .replace("  start = Number(new Constructor());\n", "")
        .replace(" - start)", " - 0)");
    assert!(refused(&literal_start), "{literal_start}");

    let reassigned_start = ELAPSED_HOURS.replace(
        "  return $THREAD_6_72713;",
        "  start = Number(new Constructor());\n  return $THREAD_6_72713;",
    );
    assert!(refused(&reassigned_start), "{reassigned_start}");

    let reassigned_constructor = ELAPSED_HOURS.replace(
        "  return $THREAD_6_72713;",
        "  Constructor = host.Other;\n  return $THREAD_6_72713;",
    );
    assert!(refused(&reassigned_constructor), "{reassigned_constructor}");

    let parameter_record = r#"function main() {
  let Constructor, start;
  Constructor = host.Date;
  start = Number(new Constructor());
  function $THREAD_6_72713(record) {
    record.top = Number((Number(new Constructor()) - start) / 1e3 / 60 / 60);
  }
  return $THREAD_6_72713;
}
main();"#;
    assert!(refused(parameter_record), "{parameter_record}");
}

#[test]
fn elapsed_hours_write_refuses_shadowed_number_and_other_function_shapes() {
    let shadowed_number = ELAPSED_HOURS
        .replace("function main() {", "function main() {\n  function Number(value) { return value; }")
        .replace(
            "Number((Number(new Constructor()) - start)",
            "Number((Number(new Constructor()) - start)",
        );
    assert!(refused(&shadowed_number), "{shadowed_number}");

    for (from, to) in [
        ("function $THREAD_6_72713()", "async function $THREAD_6_72713()"),
        ("function $THREAD_6_72713()", "function* $THREAD_6_72713()"),
        (
            "function $THREAD_6_72713()",
            "function $THREAD_6_72713(extra)",
        ),
        (
            "function $THREAD_6_72713() {\n    record.top",
            "function $THREAD_6_72713() {\n    \"use strict\";\n    record.top",
        ),
    ] {
        assert!(ELAPSED_HOURS.contains(from), "fixture lacks {from}");
        assert!(
            refused(&ELAPSED_HOURS.replace(from, to)),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn readable_names_and_collisions_remain_protected() {
    let human = ELAPSED_HOURS.replace("$THREAD_6_72713", "updateElapsedHours");
    assert!(!named(&human), "{human}");

    let collision = ELAPSED_HOURS.replace(
        "  function $THREAD_6_72713() {",
        "  function updateElapsedHours() { return 1; }\n  function $THREAD_6_72713() {",
    );
    assert!(!named(&collision), "{collision}");

    let collision_variable = ELAPSED_HOURS.replace(
        "  function $THREAD_6_72713() {",
        "  let updateElapsedHours = 1;\n  function $THREAD_6_72713() {",
    );
    assert!(!named(&collision_variable), "{collision_variable}");
}
