//! Focused, portable coverage for the `hashStringOrBuffer` loop-node variants.
//!
//! The representative body is wrapped in a minimal carrier alongside source-portable
//! spelling variants. Negatives change operations, providers, aliases, order, refusal
//! conditions, or collisions and must remain unclaimed by `thread_fast_hash_variants`.
//! Other classifiers provide a stable integration environment.
//!
//! The full-input static harness is separate from this portable target. This target
//! contains no absolute paths or cached expected counts.
use shape::naming::name_threads;

const WITNESS: &str = r#"function main(){
let $KEY_150=function($unit){return $unit.charCodeAt();};
let $KEY_286=false;
function $THREAD_6_66551($BO) {
			if (typeof $BO === "string") {
				let $BP = $BO.length >> 1 << 1;
				let i = 0;
				let $BQ = 0;
				for (; i < $BP; i = i + 2) {
					$BQ = ($BQ << 5) - $BQ + ($KEY_150($BO[i]) << 16 | $KEY_150($BO[i + 1])) | 0;
				}
				if ($BP !== $BO.length) {
					return ($BQ << 5) - $BQ + ($KEY_150($BO[$BP]) << 16) | 0;
				} else {
					return $BQ;
				}
			}
			let $BR = $KEY_286;
			if ($BR) {
				$BR = ArrayBuffer.isView($BO);
			}
			if (!$BR) {
				throw new TypeError();
			}
			let $BS;
			if ($BO.byteLength % 4 === 0) {
				$BS = $BO.buffer;
			} else {
				$BS = new ArrayBuffer($BO.byteLength + (4 - $BO.byteLength % 4));
				new Int8Array($BS).set(new Int8Array($BO.buffer));
			}
			let $KEY_2 = new Int32Array($BS);
			let $BT = 0;
			let $BU = 0;
			while ($BU < $KEY_2.length) {
				$BT = ($BT << 5) - $BT + $KEY_2[$BU] | 0;
				++$BU;
			}
			return $BT;
		}
return $THREAD_6_66551;}main();"#;

fn accepted(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.function_roles.iter().any(|role| {
            role.candidates
                .iter()
                .any(|candidate| candidate == "hashStringOrBuffer")
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| evaluator == "thread_fast_hash_variants")
        })
    })
}

fn claims_with(source: &str, evaluator: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|role| role.evaluators.iter().any(|e| e == evaluator))
        .count()
}

fn replay(source: &str) -> String {
    let report = name_threads(source, true).unwrap();
    let mut replay = source.to_owned();
    let mut edits = report.report.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    replay
}

#[test]
fn witness_generation_is_claimed_and_the_old_classifier_still_declines_it() {
    // The corpus representative belongs to this gap: the older anchor matcher
    // requires a `for` word loop, the witness uses `while`.
    assert!(accepted(WITNESS), "{WITNESS}");
    assert_eq!(claims_with(WITNESS, "thread_fast_hash_variants"), 1);
    let old = name_threads(WITNESS, true).unwrap();
    let witness_role = old
        .report
        .function_roles
        .iter()
        .find(|role| role.current_name.as_deref() == Some("$THREAD_6_66551"))
        .unwrap();
    assert!(!witness_role
        .evaluators
        .iter()
        .any(|e| e == "signals_hash_string_or_buffer"));
    assert_eq!(replay(WITNESS), name_threads(WITNESS, true).unwrap().source);
}

#[test]
fn portable_spelling_variants_are_claimed() {
    let renamed = WITNESS
        .replace("$THREAD_6_66551", "$THREAD_71_887766_2")
        .replace("$KEY_150", "$KEY_150x")
        .replace("$KEY_286", "$KEY_286x")
        .replace("$BO", "$KEY_900")
        .replace("$BP", "$KEY_901")
        .replace("$BQ", "$KEY_902")
        .replace("$BR", "$KEY_903")
        .replace("$BS", "$KEY_904")
        .replace("$KEY_2", "$KEY_905")
        .replace("$BT", "$KEY_906")
        .replace("$BU", "$KEY_907");
    let string_while = WITNESS.replace(
        "for (; i < $BP; i = i + 2) {\n\t\t\t\t\t$BQ = ($BQ << 5) - $BQ + ($KEY_150($BO[i]) << 16 | $KEY_150($BO[i + 1])) | 0;\n\t\t\t\t}",
        "while (i < $BP) {\n\t\t\t\t\t$BQ = ($BQ << 5) - $BQ + ($KEY_150($BO[i]) << 16 | $KEY_150($BO[i + 1])) | 0;\n\t\t\t\t\ti += 2;\n\t\t\t\t}",
    );
    let buffer_plus_one = WITNESS.replace("++$BU;", "$BU += 1;");
    let buffer_for = WITNESS.replace(
        "while ($BU < $KEY_2.length) {\n\t\t\t\t$BT = ($BT << 5) - $BT + $KEY_2[$BU] | 0;\n\t\t\t\t++$BU;\n\t\t\t}",
        "for (; $BU < $KEY_2.length; $BU++) {\n\t\t\t\t$BT = ($BT << 5) - $BT + $KEY_2[$BU] | 0;\n\t\t\t}",
    );
    let split_coercion = WITNESS.replace(
        "$BT = ($BT << 5) - $BT + $KEY_2[$BU] | 0;\n\t\t\t\t++$BU;",
        "$BT = ($BT << 5) - $BT + $KEY_2[$BU];\n\t\t\t\t++$BU;\n\t\t\t\t$BT = $BT | 0;",
    );
    let aliases = WITNESS
        .replace(
            "let $KEY_286=false;",
            "let $KEY_286=false;let AB=ArrayBuffer;let I32=Int32Array;let I8=Int8Array;",
        )
        .replace("ArrayBuffer.isView", "AB.isView")
        .replace("new Int32Array", "new I32")
        .replace("new ArrayBuffer", "new AB")
        .replace("new Int8Array", "new I8");
    let reader_alias = WITNESS
        .replace(
            "let $KEY_286=false;",
            "let $KEY_286=false;let $KEY_150b=$KEY_150;",
        )
        .replace("$KEY_150(", "$KEY_150b(");
    let arrow_reader = WITNESS.replace(
        "let $KEY_150=function($unit){return $unit.charCodeAt();};",
        "let $KEY_150=($unit)=>$unit.charCodeAt();",
    );
    for source in [
        renamed,
        string_while,
        buffer_plus_one,
        buffer_for,
        split_coercion,
        aliases,
        reader_alias,
        arrow_reader,
    ] {
        assert!(accepted(&source), "{source}");
        assert_eq!(replay(&source), name_threads(&source, true).unwrap().source);
    }
}

#[test]
fn operation_provider_alias_order_refusal_and_collision_negatives_are_refused() {
    for source in [
        // operation
        WITNESS.replace("($BQ << 5)", "($BQ << 6)"),
        WITNESS.replace("<< 16)", "<< 8)"),
        WITNESS.replace("$KEY_2[$BU] | 0", "$KEY_2[$BU] << 1 | 0"),
        WITNESS.replace("$BO.byteLength % 4", "$BO.byteLength % 8"),
        WITNESS.replace("$BP !== $BO.length", "$BP === $BO.length"),
        // provider
        WITNESS.replace("charCodeAt()", "codePointAt()"),
        WITNESS.replace("ArrayBuffer.isView($BO)", "Array.isArray($BO)"),
        WITNESS.replace("throw new TypeError()", "throw new Error()"),
        WITNESS.replace("new Int32Array", "new Float32Array"),
        // alias
        WITNESS
            .replace(
                "let $KEY_286=false;",
                "let $KEY_286=false;let AB=ArrayBuffer;",
            )
            .replace("ArrayBuffer.isView", "AB.isView")
            .replace("let AB=ArrayBuffer;", "let AB=ArrayBuffer;AB=other;"),
        WITNESS
            .replace(
                "let $KEY_286=false;",
                "let $KEY_286=false;let AB=NotArrayBuffer;",
            )
            .replace("ArrayBuffer.isView", "AB.isView"),
        WITNESS.replace(
            "let $KEY_150=function($unit){return $unit.charCodeAt();};",
            "let $KEY_150=function($unit){return $unit.charCodeAt();};$KEY_150=function($unit){return $unit.codePointAt();};",
        ),
        // order: the index step must follow the fold update
        WITNESS.replace(
            "$BT = ($BT << 5) - $BT + $KEY_2[$BU] | 0;\n\t\t\t\t++$BU;",
            "++$BU;\n\t\t\t\t$BT = ($BT << 5) - $BT + $KEY_2[$BU] | 0;",
        ),
        // refusal: guard polarity / missing throw / loose typeof
        WITNESS.replace("if (!$BR)", "if ($BR)"),
        WITNESS.replace(
            "if (!$BR) {\n\t\t\t\tthrow new TypeError();\n\t\t\t}",
            "if (!$BR) {\n\t\t\t}",
        ),
        WITNESS.replace("typeof $BO === \"string\"", "typeof $BO == \"string\""),
        // partial body: only the string half is not the family
        WITNESS.replace(
            "let $BR = $KEY_286;\n\t\t\tif ($BR) {\n\t\t\t\t$BR = ArrayBuffer.isView($BO);\n\t\t\t}\n\t\t\tif (!$BR) {\n\t\t\t\tthrow new TypeError();\n\t\t\t}\n\t\t\tlet $BS;\n\t\t\tif ($BO.byteLength % 4 === 0) {\n\t\t\t\t$BS = $BO.buffer;\n\t\t\t} else {\n\t\t\t\t$BS = new ArrayBuffer($BO.byteLength + (4 - $BO.byteLength % 4));\n\t\t\t\tnew Int8Array($BS).set(new Int8Array($BO.buffer));\n\t\t\t}\n\t\t\tlet $KEY_2 = new Int32Array($BS);\n\t\t\tlet $BT = 0;\n\t\t\tlet $BU = 0;\n\t\t\twhile ($BU < $KEY_2.length) {\n\t\t\t\t$BT = ($BT << 5) - $BT + $KEY_2[$BU] | 0;\n\t\t\t\t++$BU;\n\t\t\t}\n\t\t\treturn $BT;",
            "return $BO;",
        ),
    ] {
        assert!(!accepted(&source), "accepted negative:\n{source}");
    }
}

#[test]
fn collision_with_an_existing_unsuffixed_role_is_not_applied() {
    let source = WITNESS.replace(
        "function main(){",
        "function main(){let hashStringOrBuffer=42;consume(hashStringOrBuffer);",
    );
    let result = name_threads(&source, true).unwrap();
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == "$THREAD_6_66551")
        .unwrap();
    assert_eq!(outcome.proposed_name.as_deref(), Some("hashStringOrBuffer"));
    assert!(!outcome.applied);
    assert_eq!(outcome.reason.as_deref(), Some("NameCollision"));
}

#[test]
fn typed_census_prior_name_and_replay_preserve_contracts() {
    let source = WITNESS.replace(
        "return $THREAD_6_66551;}main();",
        "function $THREAD_9_999(a){return a+1;}return [$THREAD_6_66551,$THREAD_9_999];}main();",
    );
    let report = name_threads(&source, true).unwrap();
    assert_eq!(report.report.generated_thread_count, 2);
    assert!(report
        .report
        .function_roles
        .iter()
        .any(
            |role| role.current_name.as_deref() == Some("$THREAD_6_66551")
                && role
                    .evaluators
                    .iter()
                    .any(|e| e == "thread_fast_hash_variants")
        ));
    assert!(!report
        .report
        .function_roles
        .iter()
        .any(|role| role.current_name.as_deref() == Some("$THREAD_9_999")
            && role
                .evaluators
                .iter()
                .any(|e| e == "thread_fast_hash_variants")));
    assert_eq!(replay(&source), report.source);

    // A prior readable name is retained and the role is still proven.
    let prior = source.replace("$THREAD_6_66551", "hashStringOrBuffer");
    let prior_report = name_threads(&prior, true).unwrap();
    assert_eq!(prior_report.source, prior);
    assert!(prior_report.report.function_roles.iter().any(|role| {
        role.current_name.as_deref() == Some("hashStringOrBuffer")
            && role
                .evaluators
                .iter()
                .any(|e| e == "thread_fast_hash_variants")
    }));

    // A byte edit that only renames captured locals is still the same proof.
    let edited = source
        .replace("$KEY_150", "$KEY_150_edited")
        .replace("$KEY_286", "$KEY_286_edited")
        .replace("$BO", "$KEY_900");
    assert!(accepted(&edited));
}

#[test]
fn class_methods_and_empty_stubs_never_gain_the_role() {
    let class = r#"function main(){
let reader=function($unit){return $unit.charCodeAt();};
let supported=false;
class Holder { $THREAD_6_66551($BO) { return $BO; } }
function $THREAD_9_999() {}
return Holder;}main();"#;
    let result = name_threads(class, true).unwrap();
    assert!(!result.report.function_roles.iter().any(|role| role
        .evaluators
        .iter()
        .any(|e| e == "thread_fast_hash_variants")));
    assert!(result.source.contains("class Holder"));
}

fn iife_source(arrow: &str) -> String {
    WITNESS
        .replace("($BQ << 5) - $BQ", &format!("({arrow})($BQ, $BQ)"))
        .replace("($BT << 5) - $BT", &format!("({arrow})($BT, $BT)"))
}

#[test]
fn pure_hash_iife_actual_alpha_and_donor_shapes_are_claimed() {
    let actual = iife_source("($PARAMA, $PARAMB) => { return ($PARAMA << 5) - $PARAMB; }");
    let alpha = actual.replace("$PARAMA", "$A").replace("$PARAMB", "$B");
    assert!(actual.contains("$PARAMA") && actual.contains("$PARAMB"));
    for source in [actual.as_str(), alpha.as_str(), WITNESS] {
        assert!(accepted(source), "{source}");
        assert_eq!(claims_with(source, "thread_fast_hash_variants"), 1);
        assert_eq!(replay(source), name_threads(source, true).unwrap().source);
    }

    let expression = iife_source("($A, $B) => ($A << 5) - $B");
    assert!(accepted(&expression), "{expression}");
    assert_eq!(claims_with(&expression, "thread_fast_hash_variants"), 1);
}

#[test]
fn pure_hash_iife_near_misses_are_refused() {
    let actual = iife_source("($PARAMA, $PARAMB) => { return ($PARAMA << 5) - $PARAMB; }");
    let negatives = [
        // The call must pass the existing accumulator twice.
        actual.replace("($BQ, $BQ)", "($BQ, $BQ + 1)"),
        actual.replace("($BQ, $BQ)", "($BQ, $BT)"),
        // Formal writes, extra statements, and nested calls are not pure views.
        iife_source("($A, $B) => { $A = $A << 5; return $A - $B; }"),
        iife_source("($A, $B) => { $A; return ($A << 5) - $B; }"),
        iife_source("($A, $B) => helper(($A << 5) - $B)"),
        // A named pure helper is a call, not the requested IIFE view.
        WITNESS
            .replace(
                "let $KEY_286=false;",
                "let $KEY_286=false;function $F($A,$B){return ($A << 5) - $B;}",
            )
            .replace("($BQ << 5) - $BQ", "$F($BQ, $BQ)")
            .replace("($BT << 5) - $BT", "$F($BT, $BT)"),
        // The exact operators and shift literal are part of the old proof.
        iife_source("($A, $B) => ($A << 6) - $B"),
        iife_source("($A, $B) => ($A << 5) + $B"),
        // A captured value cannot stand in for either formal.
        WITNESS
            .replace("let $KEY_286=false;", "let $KEY_286=false;let $CAPTURED=0;")
            .replace(
                "($BQ << 5) - $BQ",
                "(($A, $B) => ($CAPTURED << 5) - $B)($BQ, $BQ)",
            )
            .replace(
                "($BT << 5) - $BT",
                "(($A, $B) => ($CAPTURED << 5) - $B)($BT, $BT)",
            ),
    ];
    for source in negatives {
        assert!(!accepted(&source), "accepted negative:\n{source}");
    }
}

// The declared order of the two word-loop locals is not consulted: the index is
// the one the loop test compares with `words.length`, the accumulator is the
// other, and the fold/return proofs pin it.
#[test]
fn word_loop_locals_are_bound_by_use_not_declaration_order() {
    let index_before_hash = WITNESS.replace(
        "\t\t\tlet $BT = 0;\n\t\t\tlet $BU = 0;",
        "\t\t\tlet $BU = 0;\n\t\t\tlet $BT = 0;",
    );
    assert!(accepted(&index_before_hash), "{index_before_hash}");
    assert_eq!(
        claims_with(&index_before_hash, "thread_fast_hash_variants"),
        1
    );
    assert_eq!(
        replay(&index_before_hash),
        name_threads(&index_before_hash, true).unwrap().source
    );
}

// The split memo fold `h = [h << 5, h]; h = h[0] - h[1] + operand | 0`, both
// with two hoisted unit locals and with the units inline, plus a tail that
// reuses the even bound local for its one leftover unit.
#[test]
fn memo_fold_and_hoisted_unit_locals_are_claimed() {
    let memo_hoist_tail = WITNESS
        .replace(
            "\t\t\t\t\t$BQ = ($BQ << 5) - $BQ + ($KEY_150($BO[i]) << 16 | $KEY_150($BO[i + 1])) | 0;\n\t\t\t\t}",
            "\t\t\t\t\tlet $U1 = $KEY_150($BO[i]);\n\t\t\t\t\tlet $U2 = $KEY_150($BO[i + 1]);\n\t\t\t\t\t$BQ = [$BQ << 5, $BQ];\n\t\t\t\t\t$BQ = $BQ[0] - $BQ[1] + ($U1 << 16 | $U2) | 0;\n\t\t\t\t}",
        )
        .replace(
            "\t\t\t\t\treturn ($BQ << 5) - $BQ + ($KEY_150($BO[$BP]) << 16) | 0;",
            "\t\t\t\t\t$BP = $KEY_150($BO[$BP]);\n\t\t\t\t\t$BQ = [$BQ << 5, $BQ];\n\t\t\t\t\treturn $BQ[0] - $BQ[1] + ($BP << 16) | 0;",
        );
    let memo_inline = WITNESS
        .replace(
            "\t\t\t\t\t$BQ = ($BQ << 5) - $BQ + ($KEY_150($BO[i]) << 16 | $KEY_150($BO[i + 1])) | 0;",
            "\t\t\t\t\t$BQ = [$BQ << 5, $BQ];\n\t\t\t\t\t$BQ = $BQ[0] - $BQ[1] + ($KEY_150($BO[i]) << 16 | $KEY_150($BO[i + 1])) | 0;",
        )
        .replace(
            "\t\t\t\t\treturn ($BQ << 5) - $BQ + ($KEY_150($BO[$BP]) << 16) | 0;",
            "\t\t\t\t\t$BQ = [$BQ << 5, $BQ];\n\t\t\t\t\treturn $BQ[0] - $BQ[1] + ($KEY_150($BO[$BP]) << 16) | 0;",
        );
    for source in [memo_hoist_tail, memo_inline] {
        assert!(accepted(&source), "{source}");
        assert_eq!(claims_with(&source, "thread_fast_hash_variants"), 1);
        assert_eq!(replay(&source), name_threads(&source, true).unwrap().source);
    }
}

fn forward_reader() -> String {
    WITNESS
        .replace(
            "let $KEY_150=function($unit){return $unit.charCodeAt();};\nlet $KEY_286=false;",
            "let $KEY_286=false;",
        )
        .replace(
            "return $THREAD_6_66551;}main();",
            "let $KEY_150=function($unit){return $unit.charCodeAt();};\nreturn $THREAD_6_66551;}main();",
        )
}

// A captured unit reader declared after the candidate is a deferred forward
// invocation: it stays claimed, while a forward to a non-reader, a multi-write
// alias, or a value inside the candidate body is refused.
#[test]
fn deferred_forward_reader_is_claimed_and_unsafe_forwards_are_refused() {
    let forward = forward_reader();
    assert!(accepted(&forward), "{forward}");
    assert_eq!(claims_with(&forward, "thread_fast_hash_variants"), 1);
    assert_eq!(
        replay(&forward),
        name_threads(&forward, true).unwrap().source
    );

    let forward_non_reader = forward.replace("charCodeAt()", "codePointAt()");
    let forward_multiwrite = forward.replace(
        "let $KEY_150=function($unit){return $unit.charCodeAt();};\nreturn $THREAD_6_66551;}main();",
        "let $KEY_150;\n$KEY_150=function($unit){return $unit.charCodeAt();};\n$KEY_150=function($unit){return $unit.charCodeAt();};\nreturn $THREAD_6_66551;}main();",
    );
    let forward_into_body = WITNESS.replace(
        "\t\t\tlet $BR = $KEY_286;",
        "\t\t\tlet $KEY_150=function($unit){return $unit.charCodeAt();};\n\t\t\tlet $BR = $KEY_286;",
    );
    for source in [forward_non_reader, forward_multiwrite, forward_into_body] {
        assert!(!accepted(&source), "accepted negative:\n{source}");
    }
}

// Wrong memo shapes, a non-unit or off-index hoist, and a wrong unit receiver
// must all stay unclaimed by this evaluator.
#[test]
fn memo_and_hoist_negatives_are_refused() {
    let memo = WITNESS.replace(
        "\t\t\t\t\t$BQ = ($BQ << 5) - $BQ + ($KEY_150($BO[i]) << 16 | $KEY_150($BO[i + 1])) | 0;",
        "\t\t\t\t\t$BQ = [$BQ << 5, $BQ];\n\t\t\t\t\t$BQ = $BQ[0] - $BQ[1] + ($KEY_150($BO[i]) << 16 | $KEY_150($BO[i + 1])) | 0;",
    );
    let hoist = memo
        .replace(
            "\t\t\t\t\t$BQ = [$BQ << 5, $BQ];\n\t\t\t\t\t$BQ = $BQ[0] - $BQ[1] + ($KEY_150($BO[i]) << 16 | $KEY_150($BO[i + 1])) | 0;",
            "\t\t\t\t\tlet $U1 = $KEY_150($BO[i]);\n\t\t\t\t\tlet $U2 = $KEY_150($BO[i + 1]);\n\t\t\t\t\t$BQ = [$BQ << 5, $BQ];\n\t\t\t\t\t$BQ = $BQ[0] - $BQ[1] + ($U1 << 16 | $U2) | 0;",
        )
        .replace(
            "\t\t\t\t\treturn ($BQ << 5) - $BQ + ($KEY_150($BO[$BP]) << 16) | 0;",
            "\t\t\t\t\t$BP = $KEY_150($BO[$BP]);\n\t\t\t\t\t$BQ = [$BQ << 5, $BQ];\n\t\t\t\t\treturn $BQ[0] - $BQ[1] + ($BP << 16) | 0;",
        );
    for source in [
        // wrong memo element order / wrong memo core order
        memo.replace("$BQ = [$BQ << 5, $BQ];", "$BQ = [$BQ, $BQ << 5];"),
        memo.replace("$BQ[0] - $BQ[1]", "$BQ[1] - $BQ[0]"),
        // memo core without its array
        memo.replace("$BQ = [$BQ << 5, $BQ];\n", ""),
        // a non-unit statement smuggled into the loop body
        hoist.replace(
            "\t\t\t\t\tlet $U2 = $KEY_150($BO[i + 1]);",
            "\t\t\t\t\tlet $U2 = $KEY_150($BO[i + 1]);\n\t\t\t\t\tlet $U3 = $BO;",
        ),
        // a hoist that does not read the pair's own units
        hoist.replace(
            "\t\t\t\t\tlet $U1 = $KEY_150($BO[i]);",
            "\t\t\t\t\tlet $U1 = $KEY_150($BO[i + 2]);",
        ),
        // a pair read through another receiver
        memo.replace("($KEY_150($BO[i]) << 16", "($KEY_150($BR[i]) << 16"),
    ] {
        assert!(!accepted(&source), "accepted negative:\n{source}");
    }
}

// The hoisted-unit witness reads each pair through separate char-code locals. Its 31-fold
// core is a block-bodied two-formal arrow IIFE: `(($PARAMA, $PARAMB) => { return ($PARAMA
// << 5) - $PARAMB; })($BJ, $BJ)`. The carrier supplies the captured charCodeAt reader and
// support flag; the candidate body is unchanged. Recognition must follow the dataflow
// rather than the source path, hash, span, or spelling.
const HOISTED_UNIT_ACTUAL: &str = r#"function $THREAD_6_75544($BH) {
					if (typeof $BH === "string") {
						let $BI = $BH.length >> 1 << 1;
						let i = 0;
						let $BJ = 0;
						for (; i < $BI; i = i + 2) {
							let $BK = $KEY_408($BH[i]);
							let $BL = $KEY_408($BH[i + 1]);
							$BJ = (($PARAMA, $PARAMB) => {
						return ($PARAMA << 5) - $PARAMB;
					})($BJ, $BJ) + ($BK << 16 | $BL) | 0;
						}
						if ($BI !== $BH.length) {
							$BI = $KEY_408($BH[$BI]);
							return (($PARAMA, $PARAMB) => {
						return ($PARAMA << 5) - $PARAMB;
					})($BJ, $BJ) + ($BI << 16) | 0;
						} else {
							return $BJ;
						}
					}
					let $BM = $KEY_344;
					if ($BM) {
						$BM = ArrayBuffer.isView($BH);
					}
					if (!$BM) {
						throw new TypeError();
					}
					let $BN;
					if ($BH.byteLength % 4 === 0) {
						$BN = $BH.buffer;
					} else {
						$BN = new ArrayBuffer($BH.byteLength + (4 - $BH.byteLength % 4));
						new Int8Array($BN).set(new Int8Array($BH.buffer));
					}
					let $KEY_0 = new Int32Array($BN);
					let $BO = 0;
					let $BP = 0;
					while ($BO < $KEY_0.length) {
						$BP = (($PARAMA, $PARAMB) => {
						return ($PARAMA << 5) - $PARAMB;
					})($BP, $BP) + $KEY_0[$BO] | 0;
						++$BO;
					}
					return $BP;
				}"#;
const HOISTED_UNIT_ACTUAL_SHA256: &str = "51e7dfad31bb0a4b89650b0364e97f0b97e596836419e2899e864e1966a45ca0";

fn hoisted_unit_actual_source() -> String {
    format!(
        "function main(){{\nlet $KEY_408=function($unit){{return $unit.charCodeAt();}};\nlet $KEY_344=false;\n{body}\nreturn $THREAD_6_75544;}}main();",
        body = HOISTED_UNIT_ACTUAL
    )
}

#[test]
fn real_hoisted_unit_hoisted_unit_iife_witness_is_claimed() {
    assert_eq!(HOISTED_UNIT_ACTUAL.len(), 1236);
    assert_eq!(
        shape::source::sha256_hex(HOISTED_UNIT_ACTUAL.as_bytes()),
        HOISTED_UNIT_ACTUAL_SHA256
    );

    // Non-vacuous base: the retained direct donor and the existing synthetic
    // IIFE positive both claim before the new real witness is exercised.
    assert!(accepted(WITNESS));
    assert!(accepted(&iife_source(
        "($PARAMA, $PARAMB) => { return ($PARAMA << 5) - $PARAMB; }"
    )));

    let actual = hoisted_unit_actual_source();
    assert!(actual.contains(HOISTED_UNIT_ACTUAL));
    assert!(accepted(&actual), "{actual}");
    assert_eq!(claims_with(&actual, "thread_fast_hash_variants"), 1);
    assert_eq!(replay(&actual), name_threads(&actual, true).unwrap().source);

    // Alpha rename of every local/thread/helper in the real capture: the proof is
    // scope identity, never spelling.
    let alpha = actual
        .replace("$THREAD_6_75544", "$THREAD_6_75544a")
        .replace("$PARAMA", "$A")
        .replace("$PARAMB", "$B")
        .replace("$KEY_408", "$READER")
        .replace("$KEY_344", "$SUPPORT")
        .replace("$BH", "$INPUT")
        .replace("$BI", "$EVEN")
        .replace("$BJ", "$HASH")
        .replace("$BK", "$UNIT1")
        .replace("$BL", "$UNIT2")
        .replace("$BM", "$FLAG")
        .replace("$BN", "$BUFFER")
        .replace("$KEY_0", "$WORDS")
        .replace("$BO", "$INDEX")
        .replace("$BP", "$ACC");
    assert!(accepted(&alpha), "{alpha}");
    assert_eq!(claims_with(&alpha, "thread_fast_hash_variants"), 1);
    assert_eq!(replay(&alpha), name_threads(&alpha, true).unwrap().source);

    // The direct donor is still accepted unchanged.
    assert!(accepted(WITNESS));
    assert_eq!(claims_with(WITNESS, "thread_fast_hash_variants"), 1);
}

// The IIFE fold supports both an arrow body and the equivalent
// `function (...) { ... }` expression.  The function-expression spelling has the
// same exact two-immutable-formal proof; async/generator values, defaults, rest,
// destructuring, formal writes, extra statements and the wrong operators fail.
#[test]
fn function_expression_iife_fold_is_claimed_and_refused() {
    let positive = iife_source("function ($A, $B) { return ($A << 5) - $B; }");
    assert!(accepted(&positive), "{positive}");
    assert_eq!(claims_with(&positive, "thread_fast_hash_variants"), 1);
    assert_eq!(
        replay(&positive),
        name_threads(&positive, true).unwrap().source
    );

    let negatives = [
        // async / generator functions are not pure views.
        iife_source("async function ($A, $B) { return ($A << 5) - $B; }"),
        iife_source("function* ($A, $B) { return ($A << 5) - $B; }"),
        // defaults, rest and destructuring formals are refused.
        iife_source("function ($A = 0, $B) { return ($A << 5) - $B; }"),
        iife_source("function (...$A) { return ($A << 5) - $A; }"),
        iife_source("function ([$A], $B) { return ($A << 5) - $B; }"),
        // formal writes and extra statements stay outside the pure view.
        iife_source("function ($A, $B) { $A = $A << 5; return $A - $B; }"),
        iife_source("function ($A, $B) { $A; return ($A << 5) - $B; }"),
        // the exact operators and shift literal are part of the old proof.
        iife_source("function ($A, $B) { return ($A << 6) - $B; }"),
        iife_source("function ($A, $B) { return ($A << 5) + $B; }"),
    ];
    for source in negatives {
        assert!(!accepted(&source), "accepted negative:\n{source}");
    }
}
