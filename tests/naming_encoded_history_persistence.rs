//! Portable fixtures only: parse/name statically; never run captured JavaScript.
//! Central integration must compile this target and verify pinned seed witnesses.
use shape::naming::name_threads;

const ROLE: &str = "persistEncodedHistorySnapshot";
const EVALUATOR: &str = "observables_encoded_history_persistence";
const BODY: &str = r#"function $THREAD_1_100() {
    let current = record.a.c;
    if (record.a.ci.length >= 8641) {
        record.a.ci.splice(0, record.a.ci.length - 8641);
    }
    record.a.ci.push(current);
    let pending = encode(JSON.stringify(record.a));
    if (pending != null) {
        pending.then(bytes => new Uint8Array(bytes).toString()).then(text => {
            return host.localStorage.setItem("storage-key", JSON.stringify({a: text, b: record.a.c}));
        }).catch(error => {
            return report({message: "diagnostic", stack: "" + error});
        });
    }
}"#;

fn fixture(body: &str) -> String {
    format!("function main() {{ let record, host, foreign, other; function originEncoder(value) {{ return Promise.resolve(value); }} function originReporter(value) {{ return foreign(value); }} let encode = originEncoder; let report = originReporter; {body} return $THREAD_1_100; }} main();")
}

fn check(source: &str, accepted: bool) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        !parsed.panicked && parsed.errors.is_empty(),
        "{:?}",
        parsed.errors
    );
    let result = name_threads(source, true).expect("fixture analyzes");
    let target = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_1_100")
        .expect("original callable exists");
    let claimed = result.report.function_roles.iter().any(|f| {
        f.current_name.as_deref() == Some("$THREAD_1_100")
            && f.evaluators.iter().any(|e| e == EVALUATOR)
    });
    assert_eq!(claimed, accepted, "{source}");
    if accepted {
        assert!(target.applied, "{target:?}");
        assert_eq!(target.proposed_name.as_deref(), Some(ROLE));
        assert_eq!(target.evaluator.as_deref(), Some(EVALUATOR));
    } else {
        assert_ne!(target.evaluator.as_deref(), Some(EVALUATOR), "{target:?}");
        assert_ne!(target.proposed_name.as_deref(), Some(ROLE), "{target:?}");
    }
}

fn change(source: &str, from: &str, to: &str) -> String {
    assert_eq!(source.matches(from).count(), 1, "mutation anchor {from}");
    source.replacen(from, to, 1)
}

#[test]
fn history_inline_and_local_error_record_variants() {
    let source = fixture(BODY);
    check(&source, true);
    let split = change(&source, "return report({message: \"diagnostic\", stack: \"\" + error});", "let diagnostic = {message: \"another-message\"}; diagnostic.stack = \"\" + error; return report(diagnostic);");
    check(&split, true);
    let renamed = source
        .replace("record", "state")
        .replace("pending", "promise")
        .replace("bytes", "buffer")
        .replace("text", "valueString")
        .replace("8641", "19")
        .replace("storage-key", "different-key")
        .replace("diagnostic", "unrelated-message");
    check(&renamed, true);
    let explicit = change(
        &source,
        "bytes => new Uint8Array(bytes).toString()",
        "bytes => { return new Uint8Array(bytes).toString(); }",
    );
    check(&explicit, true);
}

#[test]
fn history_rejects_wrong_owner_and_disconnected_flow() {
    let source = fixture(BODY);
    for (from, to) in [
        ("let current = record.a.c", "let current = foreign.a.c"),
        ("record.a.ci.length >= 8641", "foreign.a.ci.length >= 8641"),
        ("record.a.ci.splice", "foreign.a.ci.splice"),
        ("record.a.ci.length - 8641", "foreign.a.ci.length - 8641"),
        ("record.a.ci.length - 8641", "record.a.ci.length - 8640"),
        ("splice(0,", "splice(1,"),
        ("record.a.ci.push(current)", "record.a.ci.push(other)"),
        ("record.a.ci.push(current)", "foreign.a.ci.push(current)"),
        ("JSON.stringify(record.a)", "JSON.stringify(foreign.a)"),
        ("if (pending != null)", "if (other != null)"),
        ("pending.then", "other.then"),
        ("new Uint8Array(bytes)", "new Uint8Array(other)"),
        ("a: text, b: record.a.c", "a: other, b: record.a.c"),
        ("a: text, b: record.a.c", "a: text, b: foreign.a.c"),
        ("stack: \"\" + error", "stack: \"\" + other"),
        (
            "let current = record.a.c;",
            "let current = record.a.c; current = other;",
        ),
        (
            "let pending = encode(JSON.stringify(record.a));",
            "let pending = encode(JSON.stringify(record.a)); pending = other;",
        ),
        (
            "new Uint8Array(bytes).toString()",
            "new Uint8Array(bytes).toString(1)",
        ),
        ("host.localStorage.setItem", "record.localStorage.setItem"),
        ("return report({message:", "return encode({message:"),
        ("pending.then", "pending?.then"),
        (
            "record.a.ci.push(current);",
            "record.a.ci.push(current); foreign();",
        ),
    ] {
        check(&change(&source, from, to), false);
    }
}

#[test]
fn history_rejects_nested_callable_donations_and_shadowed_globals() {
    let source = fixture(BODY);
    for (from, to) in [
        ("bytes => new Uint8Array(bytes).toString()", "bytes => { function donor() { return new Uint8Array(bytes).toString(); } return other; }"),
        ("return host.localStorage.setItem(\"storage-key\", JSON.stringify({a: text, b: record.a.c}));", "function donor() { return host.localStorage.setItem(\"storage-key\", JSON.stringify({a: text, b: record.a.c})); } return other;"),
        ("return report({message: \"diagnostic\", stack: \"\" + error});", "function donor() { return report({message: \"diagnostic\", stack: \"\" + error}); } return other;"),
        ("bytes => new Uint8Array(bytes).toString()", "other => new Uint8Array(bytes).toString()"),
        ("text => {", "other => {"),
        ("error => {", "other => {"),
        ("function main() {", "function main() { let JSON = foreign;"),
        ("function main() {", "function main() { let Uint8Array = foreign;"),
        ("let current = record.a.c;", "let encode = foreign; let current = record.a.c;"),
        ("let current = record.a.c;", "let report = foreign; let current = record.a.c;"),
        ("bytes => new Uint8Array(bytes).toString()", "async bytes => new Uint8Array(bytes).toString()"),
        ("function $THREAD_1_100()", "async function $THREAD_1_100()"),
        ("function $THREAD_1_100()", "function $THREAD_1_100(unused)"),
    ] {
        check(&change(&source, from, to), false);
    }
    let split = change(&source, "return report({message: \"diagnostic\", stack: \"\" + error});", "let diagnostic = {message: \"diagnostic\"}; diagnostic.stack = \"\" + error; return report(diagnostic);");
    for (from, to) in [
        (
            "diagnostic.stack = \"\" + error",
            "foreign.stack = \"\" + error",
        ),
        (
            "diagnostic.stack = \"\" + error",
            "diagnostic.stack = \"\" + foreign",
        ),
        ("return report(diagnostic)", "return report(foreign)"),
        (
            "diagnostic.stack =",
            "diagnostic = foreign; diagnostic.stack =",
        ),
        (
            "return report(diagnostic)",
            "function donor() { return report(diagnostic); } return other",
        ),
    ] {
        check(&change(&split, from, to), false);
    }
}

// Verbatim history-persistence body with target function spelling normalized. Synthetic
// outer declarations supply resolved callable and data slots.
const REAL_0: &str = r#"function $THREAD_1_100() {
			let $KEY_0 = $KEY_214.a.c;
			if ($KEY_214.a.ci.length >= 8641) {
				$KEY_214.a.ci.splice(0, $KEY_214.a.ci.length - 8641);
			}
			$KEY_214.a.ci.push($KEY_0);
			let $BH = $KEY_343(JSON.stringify($KEY_214.a));
			if ($BH != null) {
				$BH.then($BI => new Uint8Array($BI).toString()).then($BI => {
					return $KEY_143.localStorage.setItem("uwi3863hme@34", JSON.stringify({
						a: $BI,
						b: $KEY_214.a.c
					}));
				}).catch($BI => {
					let $BJ = { message: "IM6" };
					$BJ.stack = "" + $BI;
					return $KEY_209($BJ);
				});
			}
		}"#;
// History-persistence spelling with a split diagnostic record.
const REAL_1: &str = r#"function $THREAD_1_100() {
					let $KEY_0 = $KEY_366.a.c;
					if ($KEY_366.a.ci.length >= 8641) {
						$KEY_366.a.ci.splice(0, $KEY_366.a.ci.length - 8641);
					}
					$KEY_366.a.ci.push($KEY_0);
					let $BO = $KEY_315(JSON.stringify($KEY_366.a));
					if ($BO != null) {
						$BO.then($BP => new Uint8Array($BP).toString()).then($BP => {
							return $KEY_407.localStorage.setItem("uwi3863hme@34", JSON.stringify({
								a: $BP,
								b: $KEY_366.a.c
							}));
						}).catch($BP => {
							return $KEY_396({
								message: "IM6",
								stack: "" + $BP
							});
						});
					}
				}"#;
// History-persistence spelling with assigned promise stages.
const REAL_2: &str = r#"function $THREAD_1_100() {
					let $KEY_0 = $KEY_387.a.c;
					if ($KEY_387.a.ci.length >= 8641) {
						$KEY_387.a.ci.splice(0, $KEY_387.a.ci.length - 8641);
					}
					$KEY_387.a.ci.push($KEY_0);
					let $BM = $KEY_115(JSON.stringify($KEY_387.a));
					if ($BM != null) {
						$BM.then($BN => new Uint8Array($BN).toString()).then($BN => {
							return $KEY_359.localStorage.setItem("uwi3863hme@34", JSON.stringify({
								a: $BN,
								b: $KEY_387.a.c
							}));
						}).catch($BN => {
							return $KEY_127({
								message: "IM6",
								stack: "" + $BN
							});
						});
					}
				}"#;
// History-persistence spelling with renamed captured bindings.
const REAL_3: &str = r#"function $THREAD_1_100() {
					let $KEY_0 = $KEY_289.a.c;
					if ($KEY_289.a.ci.length >= 8641) {
						$KEY_289.a.ci.splice(0, $KEY_289.a.ci.length - 8641);
					}
					$KEY_289.a.ci.push($KEY_0);
					let $BQ = $KEY_454(JSON.stringify($KEY_289.a));
					if ($BQ != null) {
						$BQ.then($BR => new Uint8Array($BR).toString()).then($BR => {
							return $KEY_404.localStorage.setItem("uwi3863hme@34", JSON.stringify({
								a: $BR,
								b: $KEY_289.a.c
							}));
						}).catch($BR => {
							return $KEY_156({
								message: "IM6",
								stack: "" + $BR
							});
						});
					}
				}"#;

#[test]
fn history_current_map_witness_variants() {
    let source = format!("{}{}{}", "function main() { let $KEY_214, $KEY_143; function sourceEncoder(value) { return Promise.resolve(value); } function sourceReporter(value) { return value; } let $KEY_343 = sourceEncoder; let $KEY_209 = sourceReporter; ", REAL_0, " return $THREAD_1_100; } main();");
    check(&source, true);
    let source = format!("{}{}{}", "function main() { let $KEY_366, $KEY_407; function sourceEncoder(value) { return Promise.resolve(value); } function sourceReporter(value) { return value; } let $KEY_315 = sourceEncoder; let $KEY_396 = sourceReporter; ", REAL_1, " return $THREAD_1_100; } main();");
    check(&source, true);
    let source = format!("{}{}{}", "function main() { let $KEY_387, $KEY_359; function sourceEncoder(value) { return Promise.resolve(value); } function sourceReporter(value) { return value; } let $KEY_115 = sourceEncoder; let $KEY_127 = sourceReporter; ", REAL_2, " return $THREAD_1_100; } main();");
    check(&source, true);
    let source = format!("{}{}{}", "function main() { let $KEY_289, $KEY_404; function sourceEncoder(value) { return Promise.resolve(value); } function sourceReporter(value) { return value; } let $KEY_454 = sourceEncoder; let $KEY_156 = sourceReporter; ", REAL_3, " return $THREAD_1_100; } main();");
    check(&source, true);
}
