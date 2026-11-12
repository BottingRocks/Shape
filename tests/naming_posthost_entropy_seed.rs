//! Focused, portable coverage for deriveSignalCodecSeed.
//!
//! The exact helper and spelling variants use renamed locals or swapped operands with
//! unsigned coercion, wrapped in minimal carriers declaring the captured counter. Negatives
//! change operations, receivers, indices, streams, counters, shadows, or nested donors and
//! must stay unclaimed. Full-input static coverage is separate.
use shape::naming::name_threads;

const EVALUATOR: &str = "posthost_entropy_seed";
const NAME: &str = "deriveSignalCodecSeed";

const POS1: &str = r#"function main(){let $KEY_370=0;
function $THREAD_5_69705() {
			let $KEY_3 = Math.random() * 4294967296 >>> 0;
			let $KEY_2 = 0;
			try {
				$KEY_2 = performance.now() >>> 0;
			} catch (e) {}
			let $KEY_1 = Number(new Date()) >>> 0;
			let $KEY_0 = 0;
			try {
				$KEY_0 = crypto.getRandomValues(new Uint32Array(1))[0];
			} catch (e_2) {}
			$KEY_3 = $KEY_3 ^ 1103515245;
			$KEY_2 = $KEY_2;
			$KEY_2 = $KEY_2 === $KEY_2 ? $KEY_3 ^ $KEY_2 : $KEY_3;
			$KEY_1 = $KEY_1 === $KEY_1 ? $KEY_2 ^ $KEY_1 : $KEY_2;
			$KEY_0 = $KEY_0;
			++$KEY_370;
			return ($KEY_1 ^ $KEY_0 ^ $KEY_370) >>> 0;
		}
return $THREAD_5_69705;}main();"#;

const POS2: &str = r#"function main(){let $KEY_56=0;
function $THREAD_7_4322() {
			let $KEY_4 = Math.random() * 4294967296 >>> 0;
			let $KEY_2 = 0;
			try {
				$KEY_2 = performance.now() >>> 0;
			} catch (e) {}
			let $KEY_0 = Number(new Date()) >>> 0;
			let $KEY_5 = 0;
			try {
				$KEY_5 = crypto.getRandomValues(new Uint32Array(1))[0];
			} catch (e_2) {}
			$KEY_4 = $KEY_4 ^ 1103515245;
			$KEY_2 = $KEY_2;
			$KEY_2 = $KEY_2 === $KEY_2 ? $KEY_4 ^ $KEY_2 : $KEY_4;
			$KEY_0 = $KEY_0 === $KEY_0 ? $KEY_2 ^ $KEY_0 : $KEY_2;
			$KEY_5 = $KEY_5;
			++$KEY_56;
			return ($KEY_0 ^ $KEY_5 ^ $KEY_56) >>> 0;
		}
return $THREAD_7_4322;}main();"#;

const POS3: &str = r#"function main(){let $C=0;
function $THREAD_99_123() {
			let $RAND = 4294967296 * Math.random() | 0;
			let $CLK = 0;
			try {
				$CLK = performance.now() | 0;
			} catch (err) {}
			let $DATE = Number(new Date()) | 0;
			let $CRY = 0;
			try {
				$CRY = crypto.getRandomValues(new Uint32Array(1))[0];
			} catch (err_2) {}
			$RAND = 1103515245 ^ $RAND;
			$CLK = $CLK === $CLK ? $RAND ^ $CLK : $RAND;
			$DATE = $DATE === $DATE ? $CLK ^ $DATE : $CLK;
			++$C;
			return ($DATE ^ $CRY ^ $C) >>> 0;
		}
return $THREAD_99_123;}main();"#;

fn claims(source: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.candidates.iter().any(|candidate| candidate == NAME)
                && role.evaluators.iter().any(|evaluator| evaluator == EVALUATOR)
        })
        .count()
}

/// Every applied copy of the canonical role.  The duplicate-role allocator
/// shares `role_name` across copies while giving each copy a distinct concrete
/// `proposed_name`, so the applied count must be keyed on the canonical role,
/// never on one particular unsuffixed spelling.
fn applied_roles(source: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && outcome.role_name.as_deref() == Some(NAME)
                && outcome.evaluator.as_deref() == Some(EVALUATOR)
        })
        .count()
}

/// The concrete, scope-safe names actually applied to the canonical role.
fn applied_names(source: &str) -> Vec<String> {
    let mut names: Vec<String> = name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && outcome.role_name.as_deref() == Some(NAME)
                && outcome.evaluator.as_deref() == Some(EVALUATOR)
        })
        .filter_map(|outcome| outcome.proposed_name.clone())
        .collect();
    names.sort();
    names
}

#[test]
fn witness_entropy_seed_helper_is_claimed_and_renamed() {
    assert_eq!(claims(POS1), 1);
    assert_eq!(applied_roles(POS1), 1);
    assert_eq!(applied_names(POS1), vec![NAME.to_string()]);
    let renamed = name_threads(POS1, true).unwrap();
    assert!(renamed.source.contains("function deriveSignalCodecSeed("));
    assert!(!renamed.source.contains("function $THREAD_5_69705("));
    // Replaying the reported edits, sorted by byte offset, reproduces the source.
    let mut replay = POS1.to_string();
    let mut edits = renamed.report.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, renamed.source);
}

#[test]
fn renamed_and_reordered_spelling_variants_are_claimed() {
    for source in [POS2, POS3] {
        assert_eq!(claims(source), 1, "{source}");
        assert_eq!(applied_roles(source), 1, "{source}");
        assert_eq!(applied_names(source), vec![NAME.to_string()], "{source}");
    }
}

#[test]
fn meaningful_operation_receiver_and_index_negatives_are_refused() {
    for source in [
        // operation: the LCG multiplier is not the codec constant.
        POS1.replace("^ 1103515245", "^ 1103515246"),
        // operation: the random scaling modulus is not 2^32.
        POS1.replace("* 4294967296", "* 4294967295"),
        // operation: the returned mix loses the unsigned coercion.
        POS1.replace("return ($KEY_1 ^ $KEY_0 ^ $KEY_370) >>> 0;", "return ($KEY_1 ^ $KEY_0 ^ $KEY_370) | 0;"),
        // index: the crypto draw reads a different word of the array.
        POS1.replace("new Uint32Array(1))[0]", "new Uint32Array(1))[1]"),
        // index: the crypto draw allocates more than one word.
        POS1.replace("new Uint32Array(1)", "new Uint32Array(2)"),
        // receiver: the crypto source is a captured local, not the ambient global.
        POS1.replace("function main(){", "function main(){let $KEY_88=crypto;")
            .replace("crypto.getRandomValues", "$KEY_88.getRandomValues"),
        // receiver: the clock source is a captured local, not the ambient global.
        POS1.replace("function main(){", "function main(){let $KEY_77={now:function(){return 0;}};")
            .replace("performance.now()", "$KEY_77.now()"),
    ] {
        assert_eq!(claims(&source), 0, "accepted negative:\n{source}");
    }
}

#[test]
fn counter_stream_shadow_and_nested_donor_negatives_are_refused() {
    for source in [
        // counter: the incremented binding is a helper local, not captured.
        POS1.replace(
            "function $THREAD_5_69705() {",
            "function $THREAD_5_69705() {let $KEY_370=0;",
        )
        .replace("function main(){let $KEY_370=0;", "function main(){"),
        // counter: the captured binding is decremented, never pre-incremented.
        POS1.replace("++$KEY_370;", "--$KEY_370;"),
        // counter: the increment is missing entirely.
        POS1.replace("++$KEY_370;", "$KEY_370 = $KEY_370;"),
        // stream: the timestamp merge folds the random word, not the clock.
        POS1.replace("$KEY_2 ^ $KEY_1", "$KEY_3 ^ $KEY_1"),
        // stream: the returned mix drops the counter leaf.
        POS1.replace(
            "return ($KEY_1 ^ $KEY_0 ^ $KEY_370) >>> 0;",
            "return ($KEY_1 ^ $KEY_0) >>> 0;",
        ),
        // shadow: a captured local named `performance` hides the ambient global.
        POS1.replace("function main(){", "function main(){let performance={};"),
        // nested donor: the clock read is only performed inside a nested arrow.
        POS1.replace(
            "try {\n\t\t\t\t$KEY_2 = performance.now() >>> 0;\n\t\t\t} catch (e) {}",
            "try {\n\t\t\t\tlet $read = () => performance.now() >>> 0;\n\t\t\t\t$KEY_2 = $read();\n\t\t\t} catch (e) {}",
        ),
    ] {
        assert_eq!(claims(&source), 0, "accepted negative:\n{source}");
    }
}

#[test]
fn two_instances_are_both_claimed() {
    let helper = POS1
        .split_once("function main(){let $KEY_370=0;\n")
        .unwrap()
        .1
        .split_once("\nreturn $THREAD_5_69705;}main();")
        .unwrap()
        .0;
    let second = helper.replace("$THREAD_5_69705", "$THREAD_8_74386");
    let source = format!(
        "function main(){{let $KEY_370=0;{helper}{second}return [$THREAD_5_69705,$THREAD_8_74386];}}main();"
    );
    assert_eq!(claims(&source), 2, "{source}");
    // Both proven copies stay claimed and applied.  They share the canonical
    // role name, and the allocator gives them distinct concrete names instead
    // of suppressing the second copy (or expecting two identical unsuffixed
    // proposals).
    assert_eq!(applied_roles(&source), 2, "{source}");
    assert_eq!(
        applied_names(&source),
        vec![NAME.to_string(), format!("{NAME}2")],
        "{source}"
    );

    let renamed = name_threads(&source, true).unwrap();
    let first = renamed
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == "$THREAD_5_69705")
        .expect("first helper outcome");
    let second = renamed
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == "$THREAD_8_74386")
        .expect("second helper outcome");
    assert!(first.applied);
    assert!(second.applied);
    assert_eq!(first.role_name.as_deref(), Some(NAME));
    assert_eq!(second.role_name.as_deref(), Some(NAME));
    assert_eq!(first.proposed_name.as_deref(), Some(NAME));
    let second_name = format!("{NAME}2");
    assert_eq!(second.proposed_name.as_deref(), Some(second_name.as_str()));

    assert!(renamed.source.contains("function deriveSignalCodecSeed("));
    assert!(renamed.source.contains("function deriveSignalCodecSeed2("));
    assert!(!renamed.source.contains("function $THREAD_5_69705("));
    assert!(!renamed.source.contains("function $THREAD_8_74386("));
    assert!(renamed
        .source
        .contains("return [deriveSignalCodecSeed,deriveSignalCodecSeed2];"));
}

#[test]
fn prior_readable_name_is_retained_and_a_local_collision_is_not_applied() {
    let prior = POS1.replace("$THREAD_5_69705", "deriveSignalCodecSeed");
    let report = name_threads(&prior, true).unwrap();
    assert_eq!(report.source, prior);
    assert!(report.report.function_roles.iter().any(|role| {
        role.current_name.as_deref() == Some("deriveSignalCodecSeed")
            && role.evaluators.iter().any(|evaluator| evaluator == EVALUATOR)
    }));

    let collided = POS1.replace(
        "function main(){",
        "function main(){let deriveSignalCodecSeed=42;consume(deriveSignalCodecSeed);",
    );
    let report = name_threads(&collided, true).unwrap();
    let outcome = report
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == "$THREAD_5_69705")
        .unwrap();
    assert_eq!(outcome.proposed_name.as_deref(), Some(NAME));
    assert!(!outcome.applied);
    assert_eq!(outcome.reason.as_deref(), Some("NameCollision"));
}

#[test]
fn class_methods_empty_stubs_and_arrows_never_gain_the_role() {
    let source = r#"function main(){
class Holder { $THREAD_5_69705() { return $KEY_2; } }
function $THREAD_9_999() {}
let $arrow = () => 42;
return [Holder, $arrow];}main();"#;
    let result = name_threads(source, true).unwrap();
    assert!(!result.report.function_roles.iter().any(|role| role
        .evaluators
        .iter()
        .any(|evaluator| evaluator == EVALUATOR)));
    assert!(result.source.contains("class Holder"));
}
