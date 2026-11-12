//! Focused, portable coverage for the Navigation-API form-submit interception
//! family (`handleNavigationFormSubmit` + `submitFormViaHiddenForm`).
//!
//! The portable positive is the representative navigate handler plus its
//! hidden-form executor, wrapped in a minimal carrier. The spelling variants
//! cover an inlined-url submission literal, a `document.body || document.head`
//! append, and a `function` callback instead of the arrow. The negatives are
//! meaningful receiver, property-key, guard, shadow, decoy, wrong-native
//! (`NavigateEvent.prototype.intercept` vs `HTMLFormElement.prototype.submit`),
//! disconnected-form record and duplicate-key changes that must stay unclaimed;
//! each of the two roles is proved independently from its own body. The single
//! full-seed static case lives in the separate full-input artifact, never in this
//! target.
use shape::naming::name_threads;

const EVALUATOR: &str = "thread_posthost_navigation_submit";
const HANDLER: &str = "handleNavigationFormSubmit";
const EXECUTOR: &str = "submitFormViaHiddenForm";

const POSITIVE: &str = r#"function main(){
  let $KEY_346 = {};
  let $KEY_149 = function(){};
  let $KEY_134 = $KEY_149(window.HTMLFormElement.prototype, "submit");
  let $KEY_150 = function(){};
  let $KEY_151 = $KEY_150(window.NavigateEvent?.prototype, "intercept");
  let $KEY_246, $KEY_4;
  function $THREAD_2_200($BO, $BP, $BQ) {
    let $KEY_1 = window.document.createElement("form");
    let $BR;
    if ($BQ != null) {
      $KEY_1.target = $BQ.target;
      $KEY_1.enctype = $BQ.enctype;
      $KEY_1.acceptCharset = $BQ.acceptCharset;
      $KEY_1.method = $BQ.method;
      $BR = $BQ.action;
    } else {
      $KEY_1.method = "POST";
      $BR = $BO;
    }
    $KEY_1.action = $BR;
    $BP.forEach(($BV, $BW) => {
      let $KEY_2 = window.document.createElement("input");
      $KEY_2.type = "hidden";
      $KEY_2.name = $BW;
      $KEY_2.value = $BV;
      $KEY_1.appendChild($KEY_2);
    });
    $KEY_1.setAttribute("style", "display: none !important;");
    let $BS; if (window.document.body) { $BS = window.document.body; } else { $BS = window.document.head; } $BS.appendChild($KEY_1);
    let $BT, $BU;
    if ($KEY_134 == null) {
      $BT = true;
      $BU = void 0;
    } else {
      $BU = $KEY_134.originals.value;
      $BT = $BU == null;
    }
    if (!$BT) {
      $BU.call($KEY_1);
    }
    $KEY_1.parentElement.removeChild($KEY_1);
  }
  function $THREAD_1_100($BQ) {
    if ($KEY_7) {
      return void 0;
    } else {
      let $BR = $BQ.formData;
      let $BS = $BQ.destination.url;
      try {
        if ($BR != null) {
          let $BT = window.document.getElementsByTagName("form");
          let $BU = 0;
          let $BV = null;
          while ($BU < $BT.length) {
            let $BW = $BT[$BU];
            if ($BW.action === $BS) {
              if ($BV == null) {
                ++$BU;
                $BV = $BW;
                continue;
              }
              $BR = { form: null, formData: $BR }; $BR.url = $BS;
              $BR = $KEY_346.notify($BR);
              if ($BR !== null) {
                if ($BQ.canIntercept && $BR.cancelSubmission) {
                  let $BX, $BY;
                  if ($KEY_151 == null) {
                    $BX = $KEY_151 == null;
                    $BY = void 0;
                  } else {
                    $BY = $KEY_151.originals.value;
                    $BX = $BY == null;
                  }
                  if (!$BX) {
                    $BY.call($BQ);
                  }
                }
                if ($BR.updatedFormData != null) {
                  if (window.navigation != null) {
                    window.navigation.removeEventListener("navigate", $KEY_4);
                  }
                  $KEY_246($BS, $BR.updatedFormData, null);
                }
              }
              return void 0;
            } else {
              ++$BU;
              continue;
            }
          }
          $BR = { form: $BV, formData: $BR }; $BR.url = $BS;
          $BR = $KEY_346.notify($BR);
          if ($BR !== null) {
            if ($BQ.canIntercept && $BR.cancelSubmission) {
              let $BZ, $CA;
              if ($KEY_151 == null) {
                $BZ = $KEY_151 == null;
                $CA = void 0;
              } else {
                $CA = $KEY_151.originals.value;
                $BZ = $CA == null;
              }
              if (!$BZ) {
                $CA.call($BQ);
              }
            }
            if ($BR.updatedFormData != null) {
              if (window.navigation != null) {
                window.navigation.removeEventListener("navigate", $KEY_4);
              }
              $KEY_246($BS, $BR.updatedFormData, $BV);
            }
          }
        }
      } catch (e) {}
      return void 0;
    }
  }
  $KEY_246 = $THREAD_2_200;
  $KEY_4 = $THREAD_1_100;
  return [$THREAD_2_200, $THREAD_1_100];
}
main();"#;

fn positive() -> String {
    POSITIVE.to_string()
}

/// `(role, evaluator)` claims in the report for one proposed name.
fn role_count(source: &str, name: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.candidates.iter().any(|candidate| candidate == name)
                && role.evaluators.iter().any(|evaluator| evaluator == EVALUATOR)
        })
        .count()
}

fn applied_count(source: &str, name: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some(name)
                && outcome.evaluator.as_deref() == Some(EVALUATOR)
        })
        .count()
}

/// Replays the reported edits in strictly descending byte offset. This never
/// reverses an unsorted vector: the sort key is the offset itself.
fn replay(source: &str) -> String {
    let report = name_threads(source, true).unwrap();
    let mut replay = source.to_string();
    let mut edits = report.report.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, report.source);
    replay
}

#[test]
fn witness_roles_are_claimed_and_renamed() {
    let source = positive();
    assert_eq!(role_count(&source, HANDLER), 1);
    assert_eq!(role_count(&source, EXECUTOR), 1);
    assert_eq!(applied_count(&source, HANDLER), 1);
    assert_eq!(applied_count(&source, EXECUTOR), 1);
    let renamed = replay(&source);
    assert!(renamed.contains("function handleNavigationFormSubmit("));
    assert!(renamed.contains("function submitFormViaHiddenForm("));
    assert!(!renamed.contains("$THREAD_1_100"));
    assert!(!renamed.contains("$THREAD_2_200"));
}

#[test]
fn actual_shape_variants_are_claimed() {
    // A submission literal that carries `url` inline instead of assigning it.
    let inline_url = positive()
        .replace(
            "{ form: null, formData: $BR }; $BR.url = $BS;",
            "{ form: null, formData: $BR, url: $BS };",
        )
        .replace(
            "{ form: $BV, formData: $BR }; $BR.url = $BS;",
            "{ form: $BV, formData: $BR, url: $BS };",
        );
    assert!(inline_url.contains("url: $BS"));
    assert_eq!(role_count(&inline_url, HANDLER), 1, "{inline_url}");
    assert_eq!(role_count(&inline_url, EXECUTOR), 1, "{inline_url}");

    // `document.body || document.head` instead of the branch pair.
    let logical_append = positive().replace(
        "let $BS; if (window.document.body) { $BS = window.document.body; } else { $BS = window.document.head; } $BS.appendChild($KEY_1);",
        "(window.document.body || window.document.head).appendChild($KEY_1);",
    );
    assert!(logical_append.contains("|| window.document.head"));
    assert_eq!(role_count(&logical_append, EXECUTOR), 1, "{logical_append}");
    assert_eq!(role_count(&logical_append, HANDLER), 1, "{logical_append}");

    // A `function` forEach callback instead of the arrow.
    let function_callback =
        positive().replace("$BP.forEach(($BV, $BW) => {", "$BP.forEach(function ($BV, $BW) {");
    assert!(function_callback.contains("forEach(function"));
    assert_eq!(role_count(&function_callback, EXECUTOR), 1, "{function_callback}");
    assert_eq!(role_count(&function_callback, HANDLER), 1, "{function_callback}");
}

#[test]
fn wrong_receiver_and_key_negatives_are_refused() {
    let source = positive();
    // receiver: the tag list is read from navigator, not the document.
    let wrong_receiver = source.replace(
        "window.document.getElementsByTagName",
        "window.navigator.getElementsByTagName",
    );
    // key: postData instead of formData.
    let wrong_data = source.replace("$BQ.formData", "$BQ.postData");
    // key: destination.href instead of destination.url.
    let wrong_url = source.replace("$BQ.destination.url", "$BQ.destination.href");
    // key: the form is matched on href instead of action.
    let wrong_action = source.replace("$BW.action === $BS", "$BW.href === $BS");
    // key: the removed listener is not the navigate handler.
    let wrong_event = source.replace(
        "removeEventListener(\"navigate\", $KEY_4)",
        "removeEventListener(\"navigates\", $KEY_4)",
    );
    for (label, candidate) in [
        ("wrong_receiver", wrong_receiver),
        ("wrong_data", wrong_data),
        ("wrong_url", wrong_url),
        ("wrong_action", wrong_action),
        ("wrong_event", wrong_event),
    ] {
        assert_eq!(role_count(&candidate, HANDLER), 0, "{label}:\n{candidate}");
        // The executor body is untouched by these handler-only changes.
        assert_eq!(role_count(&candidate, EXECUTOR), 1, "{label}:\n{candidate}");
    }
}

#[test]
fn wrong_guard_and_decoy_negatives_are_refused() {
    let source = positive();
    // guard: `||` instead of the exact `&&` conjunction.
    let or_guard = source.replace(
        "$BQ.canIntercept && $BR.cancelSubmission",
        "$BQ.canIntercept || $BR.cancelSubmission",
    );
    // guard: the response no longer carries cancelSubmission.
    let missing_cancel = source.replace("$BR.cancelSubmission", "$BR.cancelSubmit");
    // decoy: notify is not the registry the family notifies.
    let no_notify = source.replace("$KEY_346.notify($BR)", "$KEY_346.tell($BR)");
    // decoy: the re-submit routes to an unknown callee, not the exact executor.
    let unknown_route = source.replace("$KEY_246($BS, $BR.updatedFormData", "$KEY_999($BS, $BR.updatedFormData");
    for (label, candidate) in [
        ("or_guard", or_guard),
        ("missing_cancel", missing_cancel),
        ("no_notify", no_notify),
        ("unknown_route", unknown_route),
    ] {
        assert_eq!(role_count(&candidate, HANDLER), 0, "{label}:\n{candidate}");
    }
    // The unknown route only breaks the handler; the executor is unaffected.
    let executor_decoy = source.replace("$KEY_134.originals.value", "$KEY_134.value");
    assert_eq!(role_count(&executor_decoy, EXECUTOR), 0, "{executor_decoy}");
    // The handler depends on the exact executor: decoying the executor's
    // captured submit makes both roles unclaimable.
    assert_eq!(role_count(&executor_decoy, HANDLER), 0, "{executor_decoy}");
}

#[test]
fn exact_captured_native_and_record_negatives_are_refused() {
    let source = positive();

    // The handler's captured root must be the NavigateEvent intercept native,
    // never the form-submit native it later routes to.
    let wrong_handler_native = source.replace(
        "$KEY_150(window.NavigateEvent?.prototype, \"intercept\")",
        "$KEY_150(window.HTMLFormElement.prototype, \"submit\")",
    );
    assert!(wrong_handler_native.contains(
        "$KEY_150(window.HTMLFormElement.prototype, \"submit\")"
    ));
    assert_eq!(
        role_count(&wrong_handler_native, HANDLER),
        0,
        "{wrong_handler_native}"
    );
    assert_eq!(
        role_count(&wrong_handler_native, EXECUTOR),
        1,
        "{wrong_handler_native}"
    );

    // Same receiver, wrong native property: the capture arity still matches, so
    // only the exact property identity separates the two roles.
    let wrong_handler_property = source.replace(
        "$KEY_150(window.NavigateEvent?.prototype, \"intercept\")",
        "$KEY_150(window.NavigateEvent?.prototype, \"preventDefault\")",
    );
    assert_eq!(
        role_count(&wrong_handler_property, HANDLER),
        0,
        "{wrong_handler_property}"
    );

    // A genuine captured Event.prototype.preventDefault is still the wrong
    // native for this role: the navigation cancellation call site forwards the
    // NavigateEvent intercept capture.
    let wrong_handler_prevent_default = source.replace(
        "$KEY_150(window.NavigateEvent?.prototype, \"intercept\")",
        "$KEY_150(window.Event.prototype, \"preventDefault\")",
    );
    assert_eq!(
        role_count(&wrong_handler_prevent_default, HANDLER),
        0,
        "{wrong_handler_prevent_default}"
    );

    // The executor's captured root must be HTMLFormElement.prototype.submit.
    let wrong_executor_native = source.replace(
        "$KEY_149(window.HTMLFormElement.prototype, \"submit\")",
        "$KEY_149(window.Event.prototype, \"preventDefault\")",
    );
    assert_eq!(
        role_count(&wrong_executor_native, EXECUTOR),
        0,
        "{wrong_executor_native}"
    );
    // The handler independently proves the exact executor, so it is refused too.
    assert_eq!(
        role_count(&wrong_executor_native, HANDLER),
        0,
        "{wrong_executor_native}"
    );

    // A captured root with no proven parent-owned capture initializer is not a
    // native call holder.
    let missing_context = source.replace(
        "$KEY_151 = $KEY_150(window.NavigateEvent?.prototype, \"intercept\")",
        "$KEY_151 = {}",
    );
    assert_eq!(
        role_count(&missing_context, HANDLER),
        0,
        "{missing_context}"
    );

    // The submission record's form must be the exact matched local or null,
    // never an unrelated captured symbol.
    let disconnected_form = source
        .replace(
            "{ form: null, formData: $BR }",
            "{ form: $KEY_346, formData: $BR }",
        )
        .replace(
            "{ form: $BV, formData: $BR }",
            "{ form: $KEY_346, formData: $BR }",
        );
    assert!(disconnected_form.contains("{ form: $KEY_346, formData: $BR }"));
    assert_eq!(
        role_count(&disconnected_form, HANDLER),
        0,
        "{disconnected_form}"
    );
    assert_eq!(
        role_count(&disconnected_form, EXECUTOR),
        1,
        "{disconnected_form}"
    );

    // A repeated key silently overwrites the earlier value; it cannot prove the
    // record the downstream role consumes.
    let duplicate_keys = source
        .replace(
            "{ form: null, formData: $BR }",
            "{ form: null, form: null, formData: $BR }",
        )
        .replace(
            "{ form: $BV, formData: $BR }",
            "{ form: $BV, form: $BV, formData: $BR }",
        );
    assert_eq!(role_count(&duplicate_keys, HANDLER), 0, "{duplicate_keys}");
    assert_eq!(role_count(&duplicate_keys, EXECUTOR), 1, "{duplicate_keys}");
}

#[test]
fn shadow_and_arity_negatives_are_refused() {
    let source = positive();
    // shadow: the forEach callback shadows the executor's form local, so the
    // appended input no longer lands on the proven form.
    let shadowed = source.replace(
        "$KEY_1.appendChild($KEY_2);",
        "let $KEY_1 = {};$KEY_1.appendChild($KEY_2);",
    );
    assert!(shadowed.contains("let $KEY_1 = {};"));
    assert_eq!(role_count(&shadowed, EXECUTOR), 0, "{shadowed}");
    assert_eq!(role_count(&shadowed, HANDLER), 0, "{shadowed}");

    // arity: a three-parameter forEach callback is not the hidden-input build.
    let arity = source.replace("$BP.forEach(($BV, $BW) => {", "$BP.forEach(($BV, $BW, $BX) => {");
    assert_eq!(role_count(&arity, EXECUTOR), 0, "{arity}");
}

#[test]
fn unrelated_functions_never_gain_a_role() {
    // An empty stub, a class method, and an arrow are never candidates.
    let source = r#"function main(){
  function $THREAD_9_900() {}
  class Holder { $THREAD_8_800($BQ) { return $BQ; } }
  let $arrow = ($BQ) => $BQ;
  return [Holder, $arrow];
}
main();"#;
    let result = name_threads(source, true).unwrap();
    assert!(!result.report.function_roles.iter().any(|role| role
        .evaluators
        .iter()
        .any(|evaluator| evaluator == EVALUATOR)));
}

#[test]
fn prior_readable_name_is_retained_and_a_collision_is_not_applied() {
    let prior = positive().replace("$THREAD_1_100", HANDLER);
    let report = name_threads(&prior, true).unwrap();
    assert!(report.source.contains("function handleNavigationFormSubmit("));
    assert!(report.report.function_roles.iter().any(|role| {
        role.current_name.as_deref() == Some(HANDLER)
            && role.evaluators.iter().any(|evaluator| evaluator == EVALUATOR)
    }));
    // The executor is still renamed even though the handler is already named.
    assert!(report.source.contains("function submitFormViaHiddenForm("));

    let collided = positive().replace(
        "function main(){",
        "function main(){let handleNavigationFormSubmit=42;useIt(handleNavigationFormSubmit);",
    );
    let report = name_threads(&collided, true).unwrap();
    let outcome = report
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == "$THREAD_1_100")
        .unwrap();
    assert_eq!(outcome.proposed_name.as_deref(), Some(HANDLER));
    assert!(!outcome.applied);
}
