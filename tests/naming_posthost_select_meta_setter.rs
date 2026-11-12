use shape::naming::name_threads;

// The setter body is retained verbatim inside its following descriptor registration. Only
// the generated function bindings and enclosing bootstrap are named. The digest consumer is
// unbound and the sink never executes. The body mentions no addedNodes, MutationObserver,
// DIV, or STYLE, proving independence from style-inspector and iterator families.
const POSITIVE: &str = r#"function bootstrap(){
let $KEY_14 = void 0;
let $KEY_3 = false;
let $KEY_4 = false;
let $KEY_9 = false;
let $KEY_0 = false;
let $KEY_1 = { next(value){ return value; } };
let $KEY_11 = {};
function $THREAD_1_8379() {
	return $KEY_14;
}
function $THREAD_0_150330($BY) {
						$KEY_14 = $BY;
						if ($BY == null) {
							return void 0;
						} else {
							let $KEY_2 = Object.keys($BY);
							$KEY_2.sort();
							$KEY_2 = [$KEY_2, $KEY_2.join];
							if ($KEY_22($KEY_2[1].call($KEY_2[0], ";")) === -1460593664) {
								$KEY_3 = true;
								$KEY_1.next({
									selectMeta: true,
									animationStart: $KEY_4,
									measurementDiv: $KEY_9,
									animationStyle: $KEY_0
								});
							}
							return void 0;
						}
					}
let $BX = { get: void 0, set: void 0 };
$BX.get = $THREAD_1_8379;
$BX.set = $THREAD_0_150330;
Object.defineProperty($KEY_11, "_selectMeta", $BX);
}
bootstrap();"#;

// Alpha renaming: every scoped slot, formal, generated function id and captured
// flag spelling changes. Nothing about the rule may depend on the seed
// spellings; only the protocol method and literal names are fixed.
const ALPHA: &str = r#"function $THREAD_90_1(){
let $KEY_90 = void 0;
let $KEY_91 = false;
let $KEY_92 = false;
let $KEY_93 = false;
let $KEY_94 = false;
let $KEY_95 = { next($KEY_96){ return $KEY_96; } };
let $KEY_99 = {};
function $THREAD_90_2() {
return $KEY_90;
}
function $THREAD_91_2($KEY_100) {
$KEY_90 = $KEY_100;
if ($KEY_100 == null) {
return void 0;
} else {
let $KEY_101 = Object.keys($KEY_100);
$KEY_101.sort();
$KEY_101 = [$KEY_101, $KEY_101.join];
if ($KEY_97($KEY_101[1].call($KEY_101[0], ";")) === 5) {
$KEY_91 = true;
$KEY_95.next({
selectMeta: true,
animationStart: $KEY_92,
measurementDiv: $KEY_93,
animationStyle: $KEY_94
});
}
return void 0;
}
}
let $KEY_102 = { get: void 0, set: void 0 };
$KEY_102.get = $THREAD_90_2;
$KEY_102.set = $THREAD_91_2;
Object.defineProperty($KEY_99, "_selectMeta", $KEY_102);
}
$THREAD_90_1();"#;

// Readable local spellings prove the rule matches scoped operations, not
// `$KEY_` lexemes. The four payload fields and the captured state slots line
// up by scope identity alone.
const READABLE: &str = r#"function bootstrap(){
let storedSlot = void 0;
let probeFlag = false;
let animationStart = false;
let measurementDiv = false;
let animationStyle = false;
let observer = { next(value){ return value; } };
let target = {};
function $THREAD_1_8379() {
return storedSlot;
}
function $THREAD_0_150330(input) {
storedSlot = input;
if (input == null) {
return void 0;
} else {
let keys = Object.keys(input);
keys.sort();
keys = [keys, keys.join];
if (digest(keys[1].call(keys[0], ";")) === -1460593664) {
probeFlag = true;
observer.next({
selectMeta: true,
animationStart: animationStart,
measurementDiv: measurementDiv,
animationStyle: animationStyle
});
}
return void 0;
}
}
let descriptor = { get: void 0, set: void 0 };
descriptor.get = $THREAD_1_8379;
descriptor.set = $THREAD_0_150330;
Object.defineProperty(target, "_selectMeta", descriptor);
}
bootstrap();"#;

// In the direct-join variant the sorted keys array is never overwritten into a holder; the
// hash consumes its join(";") directly. Captured slots, conditional registration, getter,
// setter assignment, and _selectMeta defineProperty remain, so this is not body-only donor
// evidence.
const DIRECT_JOIN_VARIANT: &str = r#"function bootstrap(){
let $KEY_2 = void 0;
let $KEY_5 = false;
let $KEY_10 = false;
let $KEY_6 = false;
let $KEY_14 = false;
let $KEY_4 = { next(value){ return value; } };
let $KEY_7 = {};
let $KEY_1 = null;
if ($KEY_1 == null || $KEY_1.configurable) {
function $THREAD_9_166746($BO) {
								$KEY_2 = $BO;
								if ($BO == null) {
									return void 0;
								} else {
									let $KEY_1 = Object.keys($BO);
									$KEY_1.sort();
									if ($KEY_359($KEY_1.join(";")) === -1460593664) {
										$KEY_5 = true;
										$KEY_4.next({
											selectMeta: true,
											animationStart: $KEY_10,
											measurementDiv: $KEY_6,
											animationStyle: $KEY_14
										});
									}
									return void 0;
								}
							}let $BN = { get: void 0, set: void 0 };
function $THREAD_4_82634() { return $KEY_2; }
$BN.get = $THREAD_4_82634;
$BN.set = $THREAD_9_166746;
Object.defineProperty($KEY_7, "_selectMeta", $BN);
}
}
bootstrap();"#;

// Alpha spelling of the direct-join variant. The negative hash remains a signed numeric
// literal rather than a string or dictionary key.
const DIRECT_JOIN_ALPHA: &str = r#"function $THREAD_90_1(){
let $KEY_90 = void 0;
let $KEY_91 = false;
let $KEY_92 = false;
let $KEY_93 = false;
let $KEY_94 = false;
let $KEY_95 = { next(value){ return value; } };
let $KEY_99 = {};
function $THREAD_90_2($KEY_100) {
$KEY_90 = $KEY_100;
if ($KEY_100 == null) {
return void 0;
} else {
let $KEY_101 = Object.keys($KEY_100);
$KEY_101.sort();
if ($KEY_97($KEY_101.join(";")) === -5) {
$KEY_91 = true;
$KEY_95.next({
selectMeta: true,
animationStart: $KEY_92,
measurementDiv: $KEY_93,
animationStyle: $KEY_94
});
}
return void 0;
}
}
let $KEY_102 = { get: void 0, set: void 0 };
function $THREAD_90_3() { return $KEY_90; }
$KEY_102.get = $THREAD_90_3;
$KEY_102.set = $THREAD_90_2;
Object.defineProperty($KEY_99, "_selectMeta", $KEY_102);
}
$THREAD_90_1();"#;

// The direct body exists, but registration points at a different function. The proof must
// not borrow registration or sink evidence from a same-shaped sibling.
const FOREIGN_REGISTRATION_DONOR: &str = r#"function bootstrap(){
let stored = void 0;
let flag = false;
let start = false;
let div = false;
let style = false;
let observer = { next(value){ return value; } };
let target = {};
function $THREAD_8_123(input) {
stored = input;
if (input == null) { return void 0; } else {
let keys = Object.keys(input);
keys.sort();
if (hash(keys.join(";")) === -5) {
flag = true;
observer.next({ selectMeta: true, animationStart: start, measurementDiv: div, animationStyle: style });
}
return void 0;
}
}
function $THREAD_1_234() { return stored; }
let descriptor = { get: void 0, set: void 0 };
descriptor.get = $THREAD_1_234;
descriptor.set = $THREAD_1_234;
Object.defineProperty(target, "_selectMeta", descriptor);
}
bootstrap();"#;

// An unused nested donor: its whole body is the protocol shape, but the only
// registration in the scope binds the outer function, so neither the donor nor
// the outer function is claimed. A body-only match must never borrow the sink.
const DONOR: &str = r#"function bootstrap(){
let $KEY_14 = void 0;
let $KEY_3 = false;
let $KEY_4 = false;
let $KEY_9 = false;
let $KEY_0 = false;
let $KEY_1 = { next(value){ return value; } };
let $KEY_11 = {};
function $THREAD_0_150330($BY) {
function $THREAD_9_9999($BZ) {
$KEY_14 = $BZ;
if ($BZ == null) {
return void 0;
} else {
let $KEY_2 = Object.keys($BZ);
$KEY_2.sort();
$KEY_2 = [$KEY_2, $KEY_2.join];
if ($KEY_22($KEY_2[1].call($KEY_2[0], ";")) === -1460593664) {
$KEY_3 = true;
$KEY_1.next({
selectMeta: true,
animationStart: $KEY_4,
measurementDiv: $KEY_9,
animationStyle: $KEY_0
});
}
return void 0;
}
}
return void 0;
}
function $THREAD_1_8379() {
return $KEY_14;
}
let $BX = { get: void 0, set: void 0 };
$BX.get = $THREAD_1_8379;
$BX.set = $THREAD_0_150330;
Object.defineProperty($KEY_11, "_selectMeta", $BX);
}
bootstrap();"#;

// A one-formal helper that mentions `addedNodes` but has no `_selectMeta`
// descriptor. The old iterator family may own it; this family must not.
const ADDED_NODES: &str = r#"function bootstrap(){
let $KEY_9 = false;
function $THREAD_4_1(records){
let $KEY_5;
let $BZ = 0;
while ($BZ < records.length) {
$KEY_5 = records[$BZ];
$KEY_9 = $KEY_5.addedNodes.length;
++$BZ;
}
}
$THREAD_4_1([]);
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
    let result = name_threads(source, true).unwrap_or_else(|error| {
        panic!("negative source must parse and analyze: {error}\n{source}")
    });
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

#[test]
fn actual_holder_join_select_meta_setter_is_named() {
    assert!(
        named(POSITIVE, "$THREAD_0_150330", "setSelectMetaProbeValue"),
        "actual setter was not named:\n{}",
        outcome_trace(POSITIVE, "$THREAD_0_150330")
    );
}

#[test]
fn alpha_renamed_spelling_keeps_the_proof() {
    assert!(
        named(ALPHA, "$THREAD_91_2", "setSelectMetaProbeValue"),
        "alpha spelling was not named:\n{}",
        outcome_trace(ALPHA, "$THREAD_91_2")
    );
}

#[test]
fn direct_join_direct_join_variant_and_signed_hash_are_named() {
    assert!(
        named(DIRECT_JOIN_VARIANT, "$THREAD_9_166746", "setSelectMetaProbeValue"),
        "direct-join variant was not named:\n{}",
        outcome_trace(DIRECT_JOIN_VARIANT, "$THREAD_9_166746")
    );
    assert!(
        named(DIRECT_JOIN_ALPHA, "$THREAD_90_2", "setSelectMetaProbeValue"),
        "alpha direct-join variant was not named:\n{}",
        outcome_trace(DIRECT_JOIN_ALPHA, "$THREAD_90_2")
    );
    // The digest literal is opaque numeric protocol data. Swapping it for another signed
    // numeric constant must not change the structural numeric gate; observed values are not an
    // allowlist.
    for constant in ["-1460593665", "-5", "0"] {
        let source = DIRECT_JOIN_VARIANT.replace("-1460593664", constant);
        assert_ne!(
            source, DIRECT_JOIN_VARIANT,
            "constant {constant} was not substituted"
        );
        assert!(
            named(&source, "$THREAD_9_166746", "setSelectMetaProbeValue"),
            "direct-join variant with constant {constant} was not named:\n{}",
            outcome_trace(&source, "$THREAD_9_166746")
        );
    }
}

#[test]
fn direct_join_direct_join_keeps_all_source_backed_identities() {
    refuses(
        DIRECT_JOIN_VARIANT,
        "$THREAD_9_166746",
        "setSelectMetaProbeValue",
        &[
            ("$KEY_2 = $BO;", "$KEY_2 = $KEY_5;"),
            // The compared digest value is opaque numeric protocol data, not a
            // captured constant: every signed numeric literal is accepted, so
            // changing only the value is not a negative. A nonliteral operand
            // does fail the structural numeric gate without overfitting a hash.
            (
                "$KEY_359($KEY_1.join(\";\")) === -1460593664",
                "$KEY_359($KEY_1.join(\";\")) === $KEY_10",
            ),
            ("$KEY_4.next({", "$KEY_10.next({"),
            ("animationStart: $KEY_10,", "animationStart: $KEY_6,"),
            (
                "Object.defineProperty($KEY_7, \"_selectMeta\", $BN);",
                "Object.defineProperty($KEY_7, \"_other\", $BN);",
            ),
            ("if ($BO == null) {", "if ($KEY_5 == null) {"),
            (
                "let $KEY_1 = Object.keys($BO);",
                "let $KEY_1 = Object.keys($BO);\n$KEY_5 = true;",
            ),
            (
                "return void 0;\n\t\t\t\t\t\t\t\t}\n\t\t\t\t\t\t\t}let $BN",
                "return 0;\n\t\t\t\t\t\t\t\t}\n\t\t\t\t\t\t\t}let $BN",
            ),
        ],
    );
    refused(FOREIGN_REGISTRATION_DONOR, "$THREAD_8_123", "setSelectMetaProbeValue");
}

#[test]
fn readable_spellings_keep_the_proof() {
    assert!(
        named(READABLE, "$THREAD_0_150330", "setSelectMetaProbeValue"),
        "readable spelling was not named:\n{}",
        outcome_trace(READABLE, "$THREAD_0_150330")
    );
}

#[test]
fn the_rule_needs_no_added_nodes_or_mutation_observer() {
    // The positive body is the real setter: the old style-inspector dependency
    // is absent, so naming it cannot have come from that family.
    for needle in ["addedNodes", "MutationObserver", "nodeType", "tagName"] {
        assert!(
            !POSITIVE.contains(needle),
            "POSITIVE unexpectedly contains {needle}"
        );
    }
    refused(ADDED_NODES, "$THREAD_4_1", "setSelectMetaProbeValue");
}

#[test]
fn null_guard_and_defuse_are_required() {
    refuses(
        POSITIVE,
        "$THREAD_0_150330",
        "setSelectMetaProbeValue",
        &[
            ("if ($BY == null)", "if ($BY != null)"),
            ("if ($BY == null)", "if ($KEY_3 == null)"),
            ("$KEY_14 = $BY;", "$KEY_14 = $KEY_3;"),
        ],
    );
}

#[test]
fn sorted_keys_pair_and_separator_are_required() {
    refuses(
        POSITIVE,
        "$THREAD_0_150330",
        "setSelectMetaProbeValue",
        &[
            ("$KEY_2.sort();", "$KEY_2.slice();"),
            (
                "$KEY_2 = [$KEY_2, $KEY_2.join];",
                "$KEY_2 = [$KEY_2, $KEY_2.slice];",
            ),
            (
                r#"$KEY_2[1].call($KEY_2[0], ";")"#,
                r#"$KEY_2[1].call($KEY_2[0], ",")"#,
            ),
        ],
    );
}

#[test]
fn digest_input_must_come_from_the_own_keys_join_call() {
    refuses(
        POSITIVE,
        "$THREAD_0_150330",
        "setSelectMetaProbeValue",
        &[
            (r#"$KEY_2[1].call($KEY_2[0], ";")"#, r#"$KEY_2.join(";")"#),
            (
                r#"$KEY_2[1].call($KEY_2[0], ";")"#,
                r#"$KEY_2[1].call($KEY_5[0], ";")"#,
            ),
        ],
    );
}

#[test]
fn flag_observer_and_payload_are_required() {
    refuses(
        POSITIVE,
        "$THREAD_0_150330",
        "setSelectMetaProbeValue",
        &[
            ("$KEY_3 = true;", "$KEY_3 = false;"),
            ("$KEY_1.next({", "$KEY_2.next({"),
            ("selectMeta: true,", "selectMeta: false,"),
            ("animationStart: $KEY_4,", "animationStart: 0,"),
            ("measurementDiv: $KEY_9,", "measurementDiv: $KEY_4,"),
        ],
    );
}

#[test]
fn descriptor_registration_is_required() {
    refuses(
        POSITIVE,
        "$THREAD_0_150330",
        "setSelectMetaProbeValue",
        &[
            (
                r#"Object.defineProperty($KEY_11, "_selectMeta", $BX);"#,
                r#"Object.defineProperty($KEY_11, "_other", $BX);"#,
            ),
            (
                r#"Object.defineProperty($KEY_11, "_selectMeta", $BX);"#,
                r#"Object.defineProperty($KEY_11, "_selectMeta", $KEY_1);"#,
            ),
            ("$BX.set = $THREAD_0_150330;", "$BX.set = $KEY_22;"),
            ("$BX.get = $THREAD_1_8379;", "$BX.get = void 0;"),
            ("return $KEY_14;", "return $KEY_3;"),
        ],
    );
}

#[test]
fn extra_writes_and_structure_are_refused() {
    refuses(
        POSITIVE,
        "$THREAD_0_150330",
        "setSelectMetaProbeValue",
        &[
            ("$KEY_3 = true;", "$KEY_3 = true;\n$KEY_9 = true;"),
            (
                "let $KEY_2 = Object.keys($BY);",
                "let $KEY_2 = Object.keys($BY);\n$KEY_0 = true;",
            ),
        ],
    );
}

#[test]
fn unused_nested_donor_is_not_claimed() {
    refused(DONOR, "$THREAD_9_9999", "setSelectMetaProbeValue");
    refused(DONOR, "$THREAD_0_150330", "setSelectMetaProbeValue");
}

// ISO refusals must reach this exact candidate with a clean parser result and
// must decline both the family evaluator and the applied semantic role.
fn iso_refused(source: &str, target: &str) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::script()).parse();
    assert!(!parsed.panicked && parsed.errors.is_empty(), "{:?}\n{source}", parsed.errors);
    let result = name_threads(source, true).expect("ISO fixture must analyze");
    let outcomes: Vec<_> = result.report.outcomes.iter()
        .filter(|outcome| outcome.binding.name == target).collect();
    assert_eq!(outcomes.len(), 1, "exact ISO target must be present: {target}");
    for outcome in outcomes {
        assert_ne!(outcome.evaluator.as_deref(), Some("thread_posthost_select_meta_setter"),
            "ISO family accepted {target}:\n{source}");
        assert!(!(outcome.applied && outcome.proposed_name.as_deref() == Some("setSelectMetaProbeValue")),
            "ISO role applied to {target}:\n{source}");
    }
}

fn iso_readable_mutant(from: &str, to: &str) {
    assert_eq!(READABLE.matches(from).count(), 1, "mutation must hit exactly once: {from}");
    iso_refused(&READABLE.replacen(from, to, 1), "$THREAD_0_150330");
}

#[test]
fn descriptor_origin_correct_symbol_nested_and_sibling_registration_refused() {
    for replacement in [
        "function neverCalled(){ descriptor.set = $THREAD_0_150330; }",
        "const neverCalled = () => { descriptor.set = $THREAD_0_150330; };",
        "if (false) { descriptor.set = $THREAD_0_150330; }",
        "{ descriptor.set = $THREAD_0_150330; }",
    ] {
        iso_readable_mutant("descriptor.set = $THREAD_0_150330;", replacement);
    }
    iso_readable_mutant("descriptor.get = $THREAD_1_8379;",
        "function neverCalled(){ descriptor.get = $THREAD_1_8379; }");
    iso_readable_mutant("Object.defineProperty(target, \"_selectMeta\", descriptor);",
        "function neverCalled(){ Object.defineProperty(target, \"_selectMeta\", descriptor); }");
}

#[test]
fn descriptor_origin_descriptor_rebind_and_property_overwrite_refused() {
    // Includes the exact reported descriptor-rebinding mutant.
    for prefix in [
        "descriptor = {}; ",
        "descriptor.set = void 0; ",
        "descriptor['set'] = void 0; ",
        "delete descriptor.set; ",
        "let alias = descriptor; alias.set = void 0; ",
        "mutate(descriptor); ",
        "function neverCalled(){ descriptor.set = void 0; } ",
    ] {
        iso_readable_mutant("Object.defineProperty(target, \"_selectMeta\", descriptor);",
            &format!("{prefix}Object.defineProperty(target, \"_selectMeta\", descriptor);"));
    }
    iso_readable_mutant("let descriptor = { get: void 0, set: void 0 };",
        "let descriptor = foreignDescriptor;");
}

#[test]
fn descriptor_origin_installation_must_follow_both_property_writes() {
    iso_readable_mutant(
        "descriptor.set = $THREAD_0_150330;\nObject.defineProperty(target, \"_selectMeta\", descriptor);",
        "Object.defineProperty(target, \"_selectMeta\", descriptor);\ndescriptor.set = $THREAD_0_150330;");
    iso_readable_mutant(
        "descriptor.get = $THREAD_1_8379;\ndescriptor.set = $THREAD_0_150330;\nObject.defineProperty(target, \"_selectMeta\", descriptor);",
        "descriptor.set = $THREAD_0_150330;\nObject.defineProperty(target, \"_selectMeta\", descriptor);\ndescriptor.get = $THREAD_1_8379;");
}

#[test]
fn descriptor_origin_stale_late_or_foreign_getter_origins_refused() {
    for prefix in [
        "$THREAD_1_8379 = function(){ return probeFlag; }; ",
        "if (false) { $THREAD_1_8379 = function(){ return probeFlag; }; } ",
    ] {
        iso_readable_mutant("descriptor.get = $THREAD_1_8379;",
            &format!("{prefix}descriptor.get = $THREAD_1_8379;"));
    }
    let getter = "function $THREAD_1_8379() {\nreturn storedSlot;\n}";
    iso_readable_mutant(getter,
        "let $THREAD_1_8379; if (false) { $THREAD_1_8379 = function(){ return storedSlot; }; }");
    let late = READABLE.replace(getter, "let $THREAD_1_8379;").replace(
        "descriptor.get = $THREAD_1_8379;",
        "descriptor.get = $THREAD_1_8379;\n$THREAD_1_8379 = function(){ return storedSlot; };");
    iso_refused(&late, "$THREAD_0_150330");
    iso_readable_mutant(getter,
        "let $THREAD_1_8379; function neverCalled(){ $THREAD_1_8379 = function(){ return storedSlot; }; }");
    iso_readable_mutant("descriptor.set = $THREAD_0_150330;",
        "$THREAD_0_150330 = function(input){}; descriptor.set = $THREAD_0_150330;");
}

#[test]
fn descriptor_origin_supported_registration_regions_and_getter_origins_remain_named() {
    let reversed = READABLE.replace(
        "descriptor.get = $THREAD_1_8379;\ndescriptor.set = $THREAD_0_150330;",
        "descriptor.set = $THREAD_0_150330;\ndescriptor.get = $THREAD_1_8379;");
    let expression_getter = READABLE.replace(
        "function $THREAD_1_8379() {\nreturn storedSlot;\n}",
        "const $THREAD_1_8379 = function(){ return storedSlot; };");
    let registered_donor = FOREIGN_REGISTRATION_DONOR.replace(
        "descriptor.set = $THREAD_1_234;", "descriptor.set = $THREAD_8_123;");
    for (source, target) in [
        (READABLE, "$THREAD_0_150330"),
        (POSITIVE, "$THREAD_0_150330"),
        (ALPHA, "$THREAD_91_2"),
        (DIRECT_JOIN_VARIANT, "$THREAD_9_166746"),
        (DIRECT_JOIN_ALPHA, "$THREAD_90_2"),
        (reversed.as_str(), "$THREAD_0_150330"),
        (expression_getter.as_str(), "$THREAD_0_150330"),
        (registered_donor.as_str(), "$THREAD_8_123"),
    ] {
        assert!(named(source, target, "setSelectMetaProbeValue"), "{}", outcome_trace(source, target));
    }
}

// Additional candidate, separate from the original ten findings: the probe's
// false arm previously escaped the exact owned-effect grammar entirely.
#[test]
fn probe_effect_probe_alternate_effects_refused() {
    iso_readable_mutant("});\n}\nreturn void 0;",
        "});\n} else { evil(); }\nreturn void 0;");
    iso_readable_mutant("});\n}\nreturn void 0;",
        "});\n} else { probeFlag = true; }\nreturn void 0;");
    let direct = DIRECT_JOIN_ALPHA.replace("});\n}\nreturn void 0;",
        "});\n} else { $KEY_95.next({}); }\nreturn void 0;");
    assert_ne!(direct, DIRECT_JOIN_ALPHA);
    iso_refused(&direct, "$THREAD_90_2");
}
