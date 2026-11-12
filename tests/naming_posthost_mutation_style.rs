use shape::naming::name_threads;

// The owner witness shape, reduced to an inert source with the same scoped
// operations. The consumer digest and the payload sink are never executed.
// Only the two generated function bindings carry a renamable identity.
const POSITIVE: &str = r#"function bootstrap(){
let measurementDiv = false;
let animationStyle = false;
let sink = { next(value){ return value; } };
function $THREAD_1_81587(records){
let record, index;
let outer = 0;
while (outer < records.length) {
record = records[outer];
function $THREAD_6_100057(){
let node = record.addedNodes[index];
if (node.nodeType !== 1) {
return 1;
} else {
if (node.tagName === "DIV") {
let names = Array.from(node.style);
names.sort();
if (digest(names.join(";")) === -1422589240) {
let text = names.map((name) => name + ":" + node.style.getPropertyValue(name)).join(";");
if (digest(text) === -1819846925) {
measurementDiv = true;
sink.next({ selectMeta: false, animationStyle: animationStyle, measurementDiv: true });
}
}
} else if (node.tagName === "STYLE") {
let lines = node.textContent.split("\n").map((line) => line.trim()).filter((line) => line !== "");
if (digest(lines.join("\n")) === -1626966941) {
animationStyle = true;
sink.next({ selectMeta: false, animationStyle: true, measurementDiv: measurementDiv });
}
}
return void 0;
}
}
let i = 0;
for (;; i = Number(index) + 1) {
index = i;
if (!(i < record.addedNodes.length)) break;
if ($THREAD_6_100057()) {}
}
++outer;
}
}
return $THREAD_1_81587;
}
bootstrap();"#;

// Alpha renaming: every scoped capture, counter, generated function id and flag
// spelling changes. Nothing about the rule may depend on the seed spellings.
const ALPHA: &str = r#"function $THREAD_90_1(){
let $KEY_90 = false;
let $KEY_91 = false;
let $KEY_92 = { next($KEY_93){ return $KEY_93; } };
function $THREAD_91_2($KEY_94){
let $KEY_95, $KEY_96;
let $KEY_97 = 0;
while ($KEY_97 < $KEY_94.length) {
$KEY_95 = $KEY_94[$KEY_97];
function $THREAD_92_3(){
let $KEY_98 = $KEY_95.addedNodes[$KEY_96];
if ($KEY_98.nodeType !== 1) {
return 1;
} else {
if ($KEY_98.tagName === "DIV") {
let $KEY_99 = Array.from($KEY_98.style);
$KEY_99.sort();
if ($KEY_100($KEY_99.join(";")) === 5) {
$KEY_90 = true;
$KEY_92.next({ measurementDiv: true, animationStyle: $KEY_91 });
}
} else if ($KEY_98.tagName === "STYLE") {
let $KEY_101 = $KEY_98.textContent.split("\n").map($KEY_102 => $KEY_102.trim()).filter($KEY_102 => $KEY_102 !== "");
if ($KEY_100($KEY_101.join("\n")) === 6) {
$KEY_91 = true;
$KEY_92.next({ measurementDiv: $KEY_90, animationStyle: true });
}
}
return void 0;
}
}
let $KEY_103 = 0;
for (;; $KEY_103 = Number($KEY_96) + 1) {
$KEY_96 = $KEY_103;
if (!($KEY_103 < $KEY_95.addedNodes.length)) break;
if ($THREAD_92_3()) {}
}
++$KEY_97;
}
}
return $THREAD_91_2;
}
$THREAD_90_1();"#;

// Readable local spellings prove the rule is matching scoped operations, not
// `$KEY_` lexemes, and a classic `for`/`while` pair proves the loop shape is
// not fixed. The STYLE probe is first, so the else-if order is not evidence.
const SHAPES: &str = r#"function bootstrap(){
let probeDiv = false;
let probeStyle = false;
let sink = { next(value){ return value; } };
function $THREAD_3_7001(records){
let record, cursorIndex;
for (let p = 0; p < records.length; p++) {
const current = records[p];
function $THREAD_4_7002(){
let node = current.addedNodes[cursorIndex];
if (node.nodeType === 1) {
if (node.tagName === "STYLE") {
let lines = node.textContent.split("\n").map((line) => line.trim()).filter((line) => line !== "");
if (digest(lines.join("\n")) === 7) {
probeStyle = true;
sink.next({ probeDiv: probeDiv, probeStyle: true });
}
} else if (node.tagName === "DIV") {
let names = [...node.style];
names.sort();
if (digest(names.join(";")) === 8) {
probeDiv = true;
sink.next({ probeDiv: true, probeStyle: probeStyle });
}
}
}
}
let k = 0;
while (k < current.addedNodes.length) {
cursorIndex = k;
$THREAD_4_7002();
k = k + 1;
}
}
}
return $THREAD_3_7001;
}
bootstrap();"#;

const SIGNED_COMPARATOR_WITNESS: &str = r#"function bootstrap(){
let $KEY_5 = false;
let $KEY_10 = false;
let $KEY_6 = false;
let $KEY_14 = false;
let $KEY_4 = { next(value){ return value; } };
function $KEY_359(value){ return value; }
function $THREAD_1_81587($BO) {
							let $KEY_1, $KEY_2;
							let $BP = 0;
							while ($BP < $BO.length) {
								$KEY_2 = $BO[$BP];
								function $THREAD_6_100057() {
									let $KEY_3 = $KEY_2.addedNodes[$KEY_1];
									if ($KEY_3.nodeType !== 1) {
										return 1;
									} else {
										if ($KEY_3.tagName === "DIV") {
											let $BQ = Array.from($KEY_3.style);
											$BQ.sort();
											if ($KEY_359($BQ.join(";")) === -1422589240) {
												let $BR = $BQ.map($BT => $BT + ":" + $KEY_3.style.getPropertyValue($BT)).join(";").replace(RegExp(":0\\.001\\d{0,3};"), ":0.001000;");
												if ($KEY_359($BR.replace(RegExp(":-214748[23]\\d\\d\\d$"), ":-2147483647")) === -1819846925) {
													$KEY_6 = true;
													$KEY_4.next({
														selectMeta: $KEY_5,
														animationStart: $KEY_10,
														measurementDiv: true,
														animationStyle: $KEY_14
													});
												}
											}
										} else if ($KEY_3.tagName === "STYLE") {
											let $BS = $KEY_3.textContent.split("\n").map($BT => $BT.trim()).filter($BT => $BT !== "");
											if ($KEY_359($BS.join("\n")) === -1626966941) {
												$KEY_14 = true;
												$KEY_4.next({
													selectMeta: $KEY_5,
													animationStart: $KEY_10,
													measurementDiv: $KEY_6,
													animationStyle: true
												});
											}
										}
										return void 0;
									}
								}
								let i = 0;
								for (;; i = Number($KEY_1) + 1) {
									$KEY_1 = i;
									if (!(i < $KEY_2.addedNodes.length)) break;
									if ($THREAD_6_100057()) {}
								}
								++$BP;
							}
						}
$THREAD_1_81587([]);
}
bootstrap();"#;

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|report| {
        report.report.outcomes.iter().any(|outcome| {
            outcome.applied
                && outcome.binding.name == target
                && outcome.proposed_name.as_deref() == Some(name)
        })
    })
}

/// Compact outcome trace used only in assertion messages so a refusal shows
/// whether the family declined or another evaluator claimed the binding.
fn outcome_trace(source: &str, target: &str) -> String {
    let Ok(report) = name_threads(source, true) else {
        return format!("{target}: analysis error");
    };
    let rows: Vec<String> = report
        .report
        .outcomes
        .iter()
        .filter(|outcome| outcome.binding.name == target)
        .map(|outcome| {
            format!(
                "applied={} proposed={:?} evaluator={:?} reason={:?}",
                outcome.applied, outcome.proposed_name, outcome.evaluator, outcome.reason
            )
        })
        .collect();
    format!("{target}: [{}]", rows.join("; "))
}

/// A negative source must still parse and analyze; only the naming proof is
/// expected to fail. This keeps refusal tests from passing vacuously.
fn refused(source: &str, target: &str, name: &str) {
    let result = name_threads(source, true)
        .unwrap_or_else(|error| panic!("negative source must parse and analyze: {error}\n{source}"));
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes:\n{source}"));
    assert!(
        !(outcome.applied && outcome.proposed_name.as_deref() == Some(name)),
        "accepted {target} as {name}:\n{source}"
    );
}

fn refuses(template: &str, target: &str, name: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        refused(&template.replace(from, to), target, name);
    }
}

/// Apply several source edits before one refusal check. Each edit must find its
/// anchor, so a stale fixture cannot silently become a vacuous negative.
fn with_changes(template: &str, changes: &[(&str, &str)]) -> String {
    let mut source = template.to_string();
    for &(from, to) in changes {
        assert!(source.contains(from), "missing replacement {from}");
        source = source.replace(from, to);
    }
    source
}

#[test]
fn mutation_iterator_and_owned_style_inspector_are_named() {
    for source in [POSITIVE, ALPHA] {
        assert!(
            named(source, "$THREAD_1_81587", "iterateMutationAddedNodes")
                || named(source, "$THREAD_91_2", "iterateMutationAddedNodes"),
            "did not name the mutation added-node iterator:\n{}\n{source}",
            outcome_trace(source, "$THREAD_1_81587")
        );
        assert!(
            named(source, "$THREAD_6_100057", "inspectMutationStyleSignature")
                || named(source, "$THREAD_92_3", "inspectMutationStyleSignature"),
            "did not name the owned style inspector:\n{}\n{}\n{source}",
            outcome_trace(source, "$THREAD_6_100057"),
            outcome_trace(source, "$THREAD_92_3")
        );
    }
}

#[test]
fn mutation_iterator_accepts_classic_loops_and_swapped_branch_order() {
    assert!(named(SHAPES, "$THREAD_3_7001", "iterateMutationAddedNodes"));
    assert!(named(
        SHAPES,
        "$THREAD_4_7002",
        "inspectMutationStyleSignature"
    ));
}

#[test]
fn actual_signed_comparator_mutation_witness_is_named() {
    assert!(
        named(&SIGNED_COMPARATOR_WITNESS, "$THREAD_1_81587", "iterateMutationAddedNodes"),
        "actual witness iterator was not named:\n{}",
        outcome_trace(&SIGNED_COMPARATOR_WITNESS, "$THREAD_1_81587")
    );
    assert!(
        named(
            &SIGNED_COMPARATOR_WITNESS,
            "$THREAD_6_100057",
            "inspectMutationStyleSignature"
        ),
        "actual witness inspector was not named:\n{}",
        outcome_trace(&SIGNED_COMPARATOR_WITNESS, "$THREAD_6_100057")
    );
}

#[test]
fn mutation_iterator_accepts_array_from_style_provider() {
    let spread = with_changes(POSITIVE, &[("Array.from(node.style)", "[...node.style]")]);
    assert!(
        named(
            &spread,
            "$THREAD_6_100057",
            "inspectMutationStyleSignature"
        ),
        "spread style provider was not named:\n{}",
        outcome_trace(&spread, "$THREAD_6_100057")
    );
}

#[test]
fn mutation_iterator_accepts_simple_div_signature() {
    let simple = with_changes(
        POSITIVE,
        &[(
            "let text = names.map((name) => name + \":\" + node.style.getPropertyValue(name)).join(\";\");\nif (digest(text) === -1819846925) {\nmeasurementDiv = true;\nsink.next({ selectMeta: false, animationStyle: animationStyle, measurementDiv: true });\n}\n",
            "measurementDiv = true;\nsink.next({ selectMeta: false, animationStyle: animationStyle, measurementDiv: true });\n",
        )],
    );
    assert!(
        named(
            &simple,
            "$THREAD_6_100057",
            "inspectMutationStyleSignature"
        ),
        "simple DIV signature was not named:\n{}",
        outcome_trace(&simple, "$THREAD_6_100057")
    );
}

#[test]
fn mutation_iterator_accepts_inequality_guard() {
    let inequality = with_changes(
        SHAPES,
        &[(
            "if (node.nodeType === 1) {",
            "if (node.nodeType !== 1) { return 1; } else {",
        )],
    );
    assert!(
        named(
            &inequality,
            "$THREAD_4_7002",
            "inspectMutationStyleSignature"
        ),
        "inequality guard was not named:\n{}",
        outcome_trace(&inequality, "$THREAD_4_7002")
    );
}

#[test]
fn mutation_iterator_accepts_div_first_branch_order() {
    let chain = "if (node.tagName === \"STYLE\") {\nlet lines = node.textContent.split(\"\\n\").map((line) => line.trim()).filter((line) => line !== \"\");\nif (digest(lines.join(\"\\n\")) === 7) {\nprobeStyle = true;\nsink.next({ probeDiv: probeDiv, probeStyle: true });\n}\n} else if (node.tagName === \"DIV\") {\nlet names = [...node.style];\nnames.sort();\nif (digest(names.join(\";\")) === 8) {\nprobeDiv = true;\nsink.next({ probeDiv: true, probeStyle: probeStyle });\n}\n}";
    let swapped = "if (node.tagName === \"DIV\") {\nlet names = [...node.style];\nnames.sort();\nif (digest(names.join(\";\")) === 8) {\nprobeDiv = true;\nsink.next({ probeDiv: true, probeStyle: probeStyle });\n}\n} else if (node.tagName === \"STYLE\") {\nlet lines = node.textContent.split(\"\\n\").map((line) => line.trim()).filter((line) => line !== \"\");\nif (digest(lines.join(\"\\n\")) === 7) {\nprobeStyle = true;\nsink.next({ probeDiv: probeDiv, probeStyle: true });\n}\n}";
    let source = with_changes(SHAPES, &[(chain, swapped)]);
    assert!(
        named(&source, "$THREAD_4_7002", "inspectMutationStyleSignature"),
        "DIV-first branch order was not named:\n{}",
        outcome_trace(&source, "$THREAD_4_7002")
    );
}

#[test]
fn mutation_iterator_accepts_signed_digest_literals() {
    // The comparator uses a signed digest literal (`=== -1422589240`), parsed by Oxc as a unary
    // expression over a numeric literal. Reuse SHAPES unchanged except for comparator signs to
    // pin that guard. A numeric-literal-only check would refuse the signed form while accepting
    // unsigned constants.
    let signed = with_changes(
        SHAPES,
        &[("=== 7", "=== -7"), ("=== 8", "=== +8")],
    );
    assert!(signed.contains("=== -7"));
    assert!(signed.contains("=== +8"));
    assert!(
        named(&signed, "$THREAD_4_7002", "inspectMutationStyleSignature"),
        "signed digest literals were not named:\n{}",
        outcome_trace(&signed, "$THREAD_4_7002")
    );
}

#[test]
fn mutation_iterator_accepts_while_inner_loop() {
    let source = with_changes(
        POSITIVE,
        &[(
            "let i = 0;\nfor (;; i = Number(index) + 1) {\nindex = i;\nif (!(i < record.addedNodes.length)) break;\nif ($THREAD_6_100057()) {}\n}",
            "let i = 0;\nwhile (i < record.addedNodes.length) {\nindex = i;\nif ($THREAD_6_100057()) {}\ni = Number(index) + 1;\n}",
        )],
    );
    assert!(
        named(&source, "$THREAD_6_100057", "inspectMutationStyleSignature"),
        "while inner loop was not named:\n{}",
        outcome_trace(&source, "$THREAD_6_100057")
    );
}

#[test]
fn mutation_iterator_accepts_for_outer_loop() {
    let source = with_changes(
        POSITIVE,
        &[
            (
                "let outer = 0;\nwhile (outer < records.length) {\nrecord = records[outer];",
                "for (let outer = 0; outer < records.length; outer++) {\nrecord = records[outer];",
            ),
            ("++outer;\n", ""),
        ],
    );
    assert!(
        named(&source, "$THREAD_6_100057", "inspectMutationStyleSignature"),
        "for outer loop was not named:\n{}",
        outcome_trace(&source, "$THREAD_6_100057")
    );
}

#[test]
fn mutation_iterator_accepts_declared_record_form() {
    // The record is declared inside the outer loop as a const instead of being
    // assigned to a function-scope local. This is the SHAPES record form with
    // every POSITIVE feature retained. The bounded inner loop must name the
    // same declared record, otherwise the bound belongs to the stale outer
    // local and the iterator proof legitimately declines.
    let source = with_changes(
        POSITIVE,
        &[
            ("record = records[outer];", "const current = records[outer];"),
            ("record.addedNodes[index]", "current.addedNodes[index]"),
            (
                "if (!(i < record.addedNodes.length)) break;",
                "if (!(i < current.addedNodes.length)) break;",
            ),
        ],
    );
    assert!(
        named(&source, "$THREAD_6_100057", "inspectMutationStyleSignature"),
        "declared record form was not named:\n{}",
        outcome_trace(&source, "$THREAD_6_100057")
    );
}

#[test]
fn mutation_iterator_requires_its_own_added_node_bound() {
    // Only the nested callback still mentions `addedNodes`; the outer loop is
    // bounded by a different collection. Nested-decoy-only evidence is refused.
    refuses(
        POSITIVE,
        "$THREAD_1_81587",
        "iterateMutationAddedNodes",
        &[(
            "if (!(i < record.addedNodes.length)) break;",
            "if (!(i < record.childNodes.length)) break;",
        )],
    );
    refuses(
        POSITIVE,
        "$THREAD_6_100057",
        "inspectMutationStyleSignature",
        &[(
            "if (!(i < record.addedNodes.length)) break;",
            "if (!(i < record.childNodes.length)) break;",
        )],
    );
}

#[test]
fn mutation_iterator_requires_an_owned_callback_invocation() {
    refuses(
        POSITIVE,
        "$THREAD_1_81587",
        "iterateMutationAddedNodes",
        &[("if ($THREAD_6_100057()) {}", "void 0;")],
    );
}

#[test]
fn mutation_iterator_requires_exactly_one_owned_callback() {
    // A second function that the iterator directly owns is also invoked from
    // the same bounded loop. The pair is a different body, not this family.
    let extra = with_changes(
        POSITIVE,
        &[
            (
                "function $THREAD_6_100057(){\nlet node",
                "function $THREAD_7_9001(){ return 0; }\nfunction $THREAD_6_100057(){\nlet node",
            ),
            (
                "if ($THREAD_6_100057()) {}",
                "if ($THREAD_6_100057()) {}\nif ($THREAD_7_9001()) {}",
            ),
        ],
    );
    refused(&extra, "$THREAD_1_81587", "iterateMutationAddedNodes");
    refused(&extra, "$THREAD_6_100057", "inspectMutationStyleSignature");
}

#[test]
fn style_inspector_requires_the_node_type_guard_to_contain_the_tag_chain() {
    // The nodeType check is still present and still reads the proved node, but
    // it no longer protects the paired DIV/STYLE chain; a disconnected check
    // cannot donate the element guard.
    let disconnected = with_changes(
        POSITIVE,
        &[
            (
                "if (node.nodeType !== 1) {",
                "if (node.childNodes.length !== 1) {",
            ),
            (
                "if (node.tagName === \"DIV\") {",
                "if (node.nodeType === 1) { void 0; }\nif (node.tagName === \"DIV\") {",
            ),
        ],
    );
    refused(
        &disconnected,
        "$THREAD_6_100057",
        "inspectMutationStyleSignature",
    );
}

#[test]
fn style_inspector_requires_flags_on_the_probe_payload_path() {
    // The probe flags are still read by object literals, but those literals are
    // no longer handed to the sink call in their branch.
    let unrelated = with_changes(
        POSITIVE,
        &[
            (
                "sink.next({ selectMeta: false, animationStyle: animationStyle, measurementDiv: true });",
                "sink.next({ selectMeta: false, measurementDiv: true });\nlet decoyDiv = { animationStyle: animationStyle };",
            ),
            (
                "sink.next({ selectMeta: false, animationStyle: true, measurementDiv: measurementDiv });",
                "sink.next({ selectMeta: false, animationStyle: true });\nlet decoyStyle = { measurementDiv: measurementDiv };",
            ),
        ],
    );
    refused(
        &unrelated,
        "$THREAD_6_100057",
        "inspectMutationStyleSignature",
    );
}

#[test]
fn style_inspector_requires_the_iterator_record_and_cursor() {
    refuses(
        POSITIVE,
        "$THREAD_6_100057",
        "inspectMutationStyleSignature",
        &[
            (
                "let node = record.addedNodes[index];",
                "let node = other.addedNodes[index];",
            ),
            (
                "let node = record.addedNodes[index];",
                "let node = record.addedNodes[outer];",
            ),
        ],
    );
}

#[test]
fn style_inspector_requires_the_proved_node_receiver() {
    refuses(
        POSITIVE,
        "$THREAD_6_100057",
        "inspectMutationStyleSignature",
        &[
            (
                "if (node.tagName === \"DIV\")",
                "if (other.tagName === \"DIV\")",
            ),
            (
                "let names = Array.from(node.style);",
                "let names = Array.from(other.style);",
            ),
        ],
    );
}

#[test]
fn style_inspector_requires_the_unshadowed_array_provider() {
    // A local `Array` is a different provider, not the host namespace.
    refuses(
        POSITIVE,
        "$THREAD_6_100057",
        "inspectMutationStyleSignature",
        &[(
            "function $THREAD_6_100057(){\nlet node",
            "function $THREAD_6_100057(){\nlet Array = { from: function(){ return []; } };\nlet node",
        )],
    );
}

#[test]
fn style_inspector_requires_element_guard_and_both_signatures() {
    refuses(
        POSITIVE,
        "$THREAD_6_100057",
        "inspectMutationStyleSignature",
        &[
            ("if (node.nodeType !== 1)", "if (node.nodeType !== 2)"),
            ("names.sort();", "names.slice();"),
            (
                "if (digest(names.join(\";\")) === -1422589240)",
                "if (digest(names.join(\",\")) === -1422589240)",
            ),
            (
                "let lines = node.textContent.split(\"\\n\")",
                "let lines = node.innerHTML.split(\"\\n\")",
            ),
            ("lines.join(\"\\n\")", "lines.join(\"|\")"),
        ],
    );
}

#[test]
fn style_inspector_requires_the_split_line_array_join_receiver() {
    // The node text is still split, but an unrelated array donates both the join
    // and the digest input. The STYLE signature is not the node's line dataflow.
    let unrelated = POSITIVE
        .replace(
            "} else if (node.tagName === \"STYLE\") {\nlet lines",
            "} else if (node.tagName === \"STYLE\") {\nlet decoyLines = [\"a\", \"b\"];\nlet lines",
        )
        .replace(
            "digest(lines.join(\"\\n\"))",
            "digest(decoyLines.join(\"\\n\"))",
        );
    assert!(unrelated.contains("let decoyLines = [\"a\", \"b\"];"));
    assert!(unrelated.contains("digest(decoyLines.join(\"\\n\"))"));
    assert!(unrelated.contains("let lines = node.textContent.split(\"\\n\")"));
    refused(
        &unrelated,
        "$THREAD_6_100057",
        "inspectMutationStyleSignature",
    );
}

#[test]
fn style_inspector_requires_every_line_array_definition_to_flow_from_the_split() {
    // The join still consumes the split-derived symbol, but that symbol is also
    // assigned an unrelated array before the digest. The line identity is mixed
    // and the STYLE signature is refused.
    let reassigned = with_changes(
        POSITIVE,
        &[(
            "if (digest(lines.join(\"\\n\")) === -1626966941) {",
            "lines = [\"a\", \"b\"];\nif (digest(lines.join(\"\\n\")) === -1626966941) {",
        )],
    );
    assert!(reassigned.contains("lines = [\"a\", \"b\"];"));
    refused(
        &reassigned,
        "$THREAD_6_100057",
        "inspectMutationStyleSignature",
    );
}

#[test]
fn style_inspector_requires_cross_reported_probe_flags() {
    // Each branch reports only its own flag through the sink and leaves the
    // other probe's state out. That split arrangement is not the witness body.
    let split = with_changes(
        POSITIVE,
        &[
            (
                "sink.next({ selectMeta: false, animationStyle: animationStyle, measurementDiv: true });",
                "sink.next({ selectMeta: false, measurementDiv: measurementDiv });",
            ),
            (
                "sink.next({ selectMeta: false, animationStyle: true, measurementDiv: measurementDiv });",
                "sink.next({ selectMeta: false, animationStyle: animationStyle });",
            ),
        ],
    );
    refused(
        &split,
        "$THREAD_6_100057",
        "inspectMutationStyleSignature",
    );
}

#[test]
fn style_inspector_requires_distinct_captured_probe_flags() {
    refuses(
        POSITIVE,
        "$THREAD_6_100057",
        "inspectMutationStyleSignature",
        &[
            ("animationStyle = true;", "measurementDiv = true;"),
            ("measurementDiv = true;", "void 0;"),
            (
                "function $THREAD_6_100057(){\nlet node",
                "function $THREAD_6_100057(){\nlet measurementDiv = false;\nlet node",
            ),
        ],
    );
}

#[test]
fn readable_bindings_are_not_renamed() {
    let human = POSITIVE
        .replace("$THREAD_1_81587", "iterateAddedNodes")
        .replace("$THREAD_6_100057", "inspectStyleSignature");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
}
