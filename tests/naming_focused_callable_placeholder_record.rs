use shape::naming::name_threads;

const ROLE: &str = "inspectAndDeleteFunctionPrototype";

fn applied(source: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| o.applied && o.proposed_name.as_deref() == Some(ROLE))
}

fn refused(source: &str) {
    assert!(!applied(source));
    assert_eq!(name_threads(source, true).unwrap().source, source);
}

const INLINE_BASE: &str = r#"(function(){
  const nativePattern = /native code/;
  const fnToString = Function.prototype.toString;
  const hasOwn = Object.prototype.hasOwnProperty;
  function $THREAD_2_2($CF){
    let copy = $CF;
    let isFunction = typeof copy === "function";
    let nativeText;
    if (isFunction) { copy = $CF; nativeText = nativePattern.test(fnToString.call(copy)); } else { nativeText = false; }
    let hasProto;
    if (isFunction) { copy = $CF; hasProto = hasOwn.call(copy, "prototype"); } else { hasProto = false; }
    if (isFunction) { copy = fnToString.call($CF); } else { copy = ""; }
    let deleted;
    if (isFunction && !hasProto) { deleted = false; try { delete $CF.prototype; deleted = true; } catch (e) {} } else { deleted = false; }
    return { a: isFunction, b: nativeText, c: !hasProto, d: deleted, e: copy };
  }
  return [$THREAD_2_2];
})();
"#;

const PLACEHOLDER_RECORD: &str = r#"(function(){
  const nativePattern = /native code/;
  const fnToString = Function.prototype.toString;
  const hasOwn = Object.prototype.hasOwnProperty;
  function $THREAD_2_2($CF){
    let copy = $CF;
    let isFunction = typeof copy === "function";
    let nativeText;
    if (isFunction) { copy = $CF; nativeText = nativePattern.test(fnToString.call(copy)); } else { nativeText = false; }
    let hasProto;
    if (isFunction) { copy = $CF; hasProto = hasOwn.call(copy, "prototype"); } else { hasProto = false; }
    if (isFunction) { copy = fnToString.call($CF); } else { copy = ""; }
    let deleted;
    if (isFunction && !hasProto) { deleted = false; try { delete $CF.prototype; deleted = true; } catch (e) {} } else { deleted = false; }
    let record;
    record = { a: isFunction };
    record.b = nativeText;
    record.c = !hasProto;
    record.d = deleted;
    record.e = copy;
    return record;
  }
  return [$THREAD_2_2];
})();
"#;

const ALPHA: &str = r#"(function(){
  const pattern = /native code/g;
  const stringify = Function.prototype.toString;
  const owns = Object.prototype.hasOwnProperty;
  function $THREAD_2_2(target){
    let text = target;
    let flagged = typeof text == "function";
    let nativeResult;
    if (flagged) { text = target; nativeResult = pattern.test(stringify.call(text)); } else { nativeResult = false; }
    let protoPresent;
    if (flagged) { text = target; protoPresent = owns.call(text, "prototype"); } else { protoPresent = false; }
    if (flagged) { text = stringify.call(target); } else { text = ""; }
    let removed;
    if (flagged && !protoPresent) { removed = false; try { delete target.prototype; removed = true; } catch (e) {} } else { removed = false; }
    let out;
    out = { a: flagged };
    out.b = nativeResult;
    out.c = !protoPresent;
    out.d = removed;
    out.e = text;
    return out;
  }
  return [$THREAD_2_2];
})();
"#;

/// The callable-placeholder probe body is wrapped with the captured pattern, source, and
/// presence helpers it references. The function bytes are unmodified.
const PLACEHOLDER_PROBE: &str = r#"(function(){
	const $KEY_340 = /native code/;
	const $KEY_39 = Function.prototype.toString;
	const $KEY_81 = Object.prototype.hasOwnProperty;
function $THREAD_6_1434($CF) {
						let $KEY_2 = $CF;
						let $KEY_5 = typeof $KEY_2 === "function";
						let $CG;
						if ($KEY_5) {
							$KEY_2 = $CF;
							$CG = $KEY_340.test($KEY_39.call($KEY_2));
						} else {
							$CG = false;
						}
						let $CH;
						if ($KEY_5) {
							$KEY_2 = $CF;
							$CH = $KEY_81($KEY_2, "prototype");
						} else {
							$CH = false;
						}
						if ($KEY_5) {
							$KEY_2 = $KEY_39.call($CF);
						} else {
							$KEY_2 = "";
						}
						let $CI;
						if ($KEY_5 && !$CH) {
							$CI = false;
							try {
								delete $CF.prototype;
								$CI = true;
							} catch (e) {}
						} else {
							$CI = false;
						}
						$KEY_5 = { a: $KEY_5 };
						$KEY_5.b = $CG;
						$KEY_5.c = !$CH;
						$KEY_5.d = $CI;
						$KEY_5.e = $KEY_2;
						return $KEY_5;
					}
	return [$THREAD_6_1434];
})();
"#;

#[test]
fn inline_object_literal_base_still_named() {
    let result = name_threads(INLINE_BASE, true).unwrap();
    assert!(applied(INLINE_BASE));
    assert!(result
        .source
        .contains("function inspectAndDeleteFunctionPrototype($CF)"));
}

#[test]
fn callable_placeholder_probe_is_named() {
    let result = name_threads(PLACEHOLDER_PROBE, true).unwrap();
    assert!(applied(PLACEHOLDER_PROBE));
    assert_eq!(result.source, PLACEHOLDER_PROBE.replace("$THREAD_6_1434", ROLE));
}

#[test]
fn real_placeholder_placeholder_record_is_named() {
    let result = name_threads(PLACEHOLDER_RECORD, true).unwrap();
    assert!(applied(PLACEHOLDER_RECORD));
    assert!(result
        .source
        .contains("function inspectAndDeleteFunctionPrototype($CF)"));
    // The body is otherwise preserved byte-for-byte.
    assert_eq!(result.source, PLACEHOLDER_RECORD.replace("$THREAD_2_2", ROLE));
}

#[test]
fn readable_alpha_placeholder_record_is_named() {
    let result = name_threads(ALPHA, true).unwrap();
    assert!(applied(ALPHA));
    assert!(result
        .source
        .contains("function inspectAndDeleteFunctionPrototype(target)"));
}

#[test]
fn mutated_return_record_is_not_the_canonical_record() {
    // An extra placeholder field breaks the single `{ a: ... }` record.
    let source = PLACEHOLDER_RECORD.replace(
        "record = { a: isFunction };",
        "record = { a: isFunction, extra: 1 };",
    );
    refused(&source);
}

#[test]
fn foreign_input_value_refuses() {
    // `record.a` is a captured outer value, not this body's typeof test.
    let source = PLACEHOLDER_RECORD
        .replace(
            "const hasOwn = Object.prototype.hasOwnProperty;",
            "const hasOwn = Object.prototype.hasOwnProperty; const outerFlag = true;",
        )
        .replace("record = { a: isFunction };", "record = { a: outerFlag };");
    refused(&source);
}

#[test]
fn overwritten_field_value_refuses() {
    // Writing a field twice no longer produces the ordered `b/.c/.d/.e` set.
    let source = PLACEHOLDER_RECORD.replace(
        "record.b = nativeText;",
        "record.b = nativeText; record.b = nativeText;",
    );
    refused(&source);
}

#[test]
fn reassigned_record_refuses() {
    // A second placeholder write rebuilds the record, so it is not assembled
    // in exactly one declaration-local write.
    let source = PLACEHOLDER_RECORD.replace(
        "record = { a: isFunction };",
        "record = { a: isFunction }; record = { a: isFunction };",
    );
    refused(&source);
}

#[test]
fn nested_donor_probe_evidence_does_not_leak() {
    // The test/call/delete evidence lives in a nested donor; the outer body
    // only returns a record, so the function refutes.
    let source = r#"(function(){
  const nativePattern = /native code/;
  const fnToString = Function.prototype.toString;
  function $THREAD_2_2($CF){
    function probe($x){ return nativePattern.test(fnToString.call($x)); }
    let flag = probe($CF);
    let record;
    record = { a: typeof $CF === "function" };
    record.b = flag;
    record.c = false;
    record.d = false;
    record.e = "";
    return record;
  }
  return [$THREAD_2_2];
})();
"#;
    refused(source);
}

#[test]
fn reviewed_copy_binding_refuses() {
    // The one-line counterexample `record.b = copy` is still a
    // body-local, but it no longer carries the native `test(toString.call())`
    // result.
    let source = PLACEHOLDER_RECORD.replace("record.b = nativeText;", "record.b = copy;");
    refused(&source);
}

#[test]
fn reviewed_document_typeof_refuses() {
    // A typeof-function test on
    // `document` is not this callable's input formal.
    let source = PLACEHOLDER_RECORD.replace(
        "let isFunction = typeof copy === \"function\";",
        "let isFunction = typeof document === \"function\";",
    );
    refused(&source);
}

#[test]
fn wrong_prototype_input_refuses() {
    // `c` must be the prototype-presence result for the probe input itself.
    let source = PLACEHOLDER_RECORD.replace(
        "hasProto = hasOwn.call(copy, \"prototype\");",
        "hasProto = hasOwn.call(nativePattern, \"prototype\");",
    );
    refused(&source);
}

#[test]
fn wrong_delete_input_refuses() {
    // `d` only proves success when it deletes the probe input's prototype.
    let source =
        PLACEHOLDER_RECORD.replace("delete $CF.prototype;", "delete nativePattern.prototype;");
    refused(&source);
}

#[test]
fn overwritten_result_field_refuses() {
    // Overwriting the computed result local before the field write is refused.
    let source = PLACEHOLDER_RECORD.replace(
        "record.b = nativeText;",
        "nativeText = copy; record.b = nativeText;",
    );
    refused(&source);
}

#[test]
fn foreign_field_value_refuses() {
    // `c` cannot be a captured outer presence value copied into the body.
    let source = PLACEHOLDER_RECORD
        .replace(
            "const hasOwn = Object.prototype.hasOwnProperty;",
            "const hasOwn = Object.prototype.hasOwnProperty; const outerProto = false;",
        )
        .replace("record.c = !hasProto;", "record.c = !outerProto;");
    refused(&source);
}

#[test]
fn wrong_function_source_field_refuses() {
    // `e` must be the guarded `fnToString.call(input)` result, not another
    // body-local field.
    let source = PLACEHOLDER_RECORD.replace("record.e = copy;", "record.e = nativeText;");
    refused(&source);
}

#[test]
fn unrelated_body_local_field_refuses() {
    // Any body-local is not enough: `record.b = noise` has no guarded native
    // test computation behind it.
    let source = PLACEHOLDER_RECORD
        .replace("    let record;", "    let noise = 0;\n    let record;")
        .replace("record.b = nativeText;", "record.b = noise;");
    refused(&source);
}
