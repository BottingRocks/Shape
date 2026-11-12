//! Focused whole-target coverage for the split-guard DOM-ready callback flusher. The exact
//! body retains one callable MAIN and live DOMContentLoaded registration while captured
//! spellings vary independently.

use shape::naming::name_threads;

const TARGET_FUNCTION: &str = r#"function $THREAD_3_38646() {
			let $BS;
			if ($KEY_2) {
				$BS = false;
			} else {
				$BS = $KEY_328;
			}
			if ($BS) {
				$KEY_2 = true;
				let i = 0;
				for (; i < $KEY_328.length; i++) {
					$KEY_328[i]();
				}
				$KEY_328 = null;
			}
		}"#;

fn carrier(function_source: &str, function_name: &str, flag: &str, queue: &str) -> String {
    format!(
        r#"(function main() {{
    let {flag}, {queue};
    {flag} = false;
    {queue} = [];
    {function_source}
    let readyListener = {function_name};
    document.addEventListener("DOMContentLoaded", readyListener);
    return readyListener;
}})();"#
    )
}

fn named(source: &str, generated: &str, readable: &str) -> bool {
    let result = name_threads(source, true).expect("fixture parses and analyzes");
    result
        .source
        .contains(&format!("function {readable}()"))
        && result.report.outcomes.iter().any(|outcome| {
            outcome.binding.name == generated
                && outcome.applied
                && outcome.proposed_name.as_deref() == Some(readable)
        })
}

fn rejected(source: &str) {
    let result = name_threads(source, true).expect("negative parses and analyzes");
    assert!(
        !result.source.contains("function flushReadyQueue()"),
        "invalid ready-queue carrier was named:\n{source}"
    );
}

#[test]
fn exact_split_guard_witness_alpha_and_named_donor_share_the_ready_queue_owner() {
    assert_eq!(TARGET_FUNCTION.len(), 254);
    let witness = carrier(TARGET_FUNCTION, "$THREAD_3_38646", "$KEY_2", "$KEY_328");
    assert!(named(&witness, "$THREAD_3_38646", "flushReadyQueue"));

    let alpha = TARGET_FUNCTION
        .replace("$THREAD_3_38646", "$THREAD_900_777_2")
        .replace("$KEY_2", "$FLAG")
        .replace("$KEY_328", "$CALLBACKS")
        .replace("$BS", "$GUARD");
    let alpha = carrier(&alpha, "$THREAD_900_777_2", "$FLAG", "$CALLBACKS");
    assert!(named(&alpha, "$THREAD_900_777_2", "flushReadyQueue"));

    let donor = TARGET_FUNCTION.replace("$THREAD_3_38646", "flushReadyQueue");
    let donor = carrier(&donor, "flushReadyQueue", "$KEY_2", "$KEY_328");
    let result = name_threads(&donor, true).expect("named donor parses and analyzes");
    assert!(result.source.contains("function flushReadyQueue()"));
}

#[test]
fn foreign_list_callback_argument_early_clear_extra_effect_and_wrong_listener_are_refused() {
    let witness = carrier(TARGET_FUNCTION, "$THREAD_3_38646", "$KEY_2", "$KEY_328");

    rejected(&witness.replace("$KEY_328[i]();", "foreign[i]();"));
    rejected(&witness.replace("$KEY_328[i]();", "$KEY_328[i](payload);"));
    rejected(&witness.replace(
        "\t\t\t\t$KEY_328 = null;\n\t\t\t}",
        "\t\t\t\t$KEY_328 = null;\n\t\t\t\tfor (; i < $KEY_328.length; i++) { $KEY_328[i](); }\n\t\t\t}",
    ));
    rejected(&witness.replace(
        "\t\t\t\t$KEY_2 = true;\n\t\t\t\tlet i = 0;",
        "\t\t\t\t$KEY_2 = true;\n\t\t\t\taudit();\n\t\t\t\tlet i = 0;",
    ));
    rejected(&witness.replace(
        "document.addEventListener(\"DOMContentLoaded\", readyListener)",
        "document.addEventListener(\"DOMContentLoaded\", foreignListener)",
    ));
}
