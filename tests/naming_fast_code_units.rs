use shape::naming::name_threads;

// Fast-loop family nullable-character-code-array-to-string.
// Canonical existing role: `codeUnitsToString`. The widening over PASS6 is the
// opaque `<opaque-root>.String.fromCharCode` provider root plus the bounded
// `.join.call` return; the canonical `String.fromCharCode` arm is accepted only
// for the unshadowed global identifier `String`.
const EVALUATOR: &str = "fast_code_units_opaque_provider";

const UNITS: &str = r#"function main(){
function $THREAD_4_44100($CB){
let $KEY_0=[];
if($CB!=null){$CB.forEach($CC=>{$KEY_0.push(b.String.fromCharCode($CC));});}
return $KEY_0.join("");
}
return $THREAD_4_44100;
}
main();"#;

fn named(source: &str, expected: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .outcomes
            .iter()
            .any(|o| o.applied && o.proposed_name.as_deref() == Some(expected))
    })
}

fn our_claim(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .function_roles
            .iter()
            .any(|role| role.evaluators.iter().any(|e| e == EVALUATOR))
    })
}

fn parses(source: &str) -> bool {
    name_threads(source, true).is_ok()
}

fn denied(source: &str) {
    assert!(parses(source), "fixture did not parse: {source}");
    assert!(
        !named(source, "codeUnitsToString"),
        "unexpected codeUnitsToString for: {source}"
    );
    assert!(!our_claim(source), "unexpected fast claim for: {source}");
}

#[test]
fn code_units_accepts_opaque_namespace_provider_and_portable_alpha() {
    let ours = [
        UNITS.to_string(),
        UNITS
            .replace("$THREAD_4_44100", "$THREAD_900_777_2")
            .replace("$CB", "$KEY_400")
            .replace("$CC", "$KEY_900")
            .replace("$KEY_0", "buffer")
            .replace("b.String", "host.String"),
        UNITS.replace("b.String", "$KEY_17.String"),
        UNITS.replace("b.String", "namespace.String"),
        // A plain immutable single-origin alias of the opaque host root.
        UNITS
            .replace("function $THREAD_4_44100", "const host=b;function $THREAD_4_44100")
            .replace("b.String", "host.String"),
        // Both arrow forms are legal for forEach (the result is ignored).
        UNITS.replace(
            "{$KEY_0.push(b.String.fromCharCode($CC));}",
            "$KEY_0.push(b.String.fromCharCode($CC))",
        ),
        // A plain `var` array binding is still one immutable local slot.
        UNITS.replace("let $KEY_0=[];", "var $KEY_0=[];"),
    ];
    for source in ours {
        assert!(named(&source, "codeUnitsToString"), "{source}");
        assert!(our_claim(&source), "not the fast claim: {source}");
    }
}

#[test]
fn code_units_lets_the_pass6_plain_string_provider_win() {
    // `String.fromCharCode` is already proved by the PASS6 shared rules. This
    // module must never be the producer for that shape: older generations win.
    let plain = UNITS.replace("b.String.fromCharCode", "String.fromCharCode");
    assert!(named(&plain, "codeUnitsToString"));
    assert!(!our_claim(&plain), "fast rule overrode PASS6: {plain}");
}

#[test]
fn code_units_refuses_changed_provider_operation_and_path() {
    for (from, to) in [
        ("fromCharCode", "fromCodePoint"),
        ("b.String.fromCharCode", "b.Text.fromCharCode"),
        ("b.String.fromCharCode", "b.String.fromCharCodeAt"),
        ("String.fromCharCode($CC)", "String.fromCharCode($CC,0)"),
        ("fromCharCode($CC)", "fromCharCode(other)"),
        ("b.String.fromCharCode($CC)", "b.String.fromCharCode($KEY_0)"),
        ("b.String.fromCharCode", "b.String?.fromCharCode"),
        ("b.String.fromCharCode", "b?.String.fromCharCode"),
    ] {
        denied(&UNITS.replace(from, to));
    }
}

#[test]
fn code_units_refuses_unproved_or_mutated_provider_roots() {
    for source in [
        // The alias is written after initialization.
        UNITS
            .replace("function $THREAD_4_44100", "let host=b;function $THREAD_4_44100")
            .replace("b.String", "host.String")
            .replace("return $THREAD_4_44100;", "host=other;return $THREAD_4_44100;"),
        // The root is a runtime formal of the enclosing function, not a plain
        // single-origin variable alias.
        UNITS
            .replace("function main(){", "function main(host){")
            .replace("b.String", "host.String"),
        // The root alias comes from a mutable/opaque data expression, not the
        // unbound host namespace.
        UNITS
            .replace("function $THREAD_4_44100", "const host=provider.value;function $THREAD_4_44100")
            .replace("b.String", "host.String"),
        // A destructured alias is not a single-origin plain binding.
        UNITS
            .replace("function $THREAD_4_44100", "const {host}=provider;function $THREAD_4_44100")
            .replace("b.String", "host.String"),
        // A for-of binding is never an alias origin.
        UNITS
            .replace("function $THREAD_4_44100", "let host;for(host of hosts){}function $THREAD_4_44100")
            .replace("b.String", "host.String"),
    ] {
        denied(&source);
    }
}

#[test]
fn code_units_refuses_changed_owned_operations() {
    for (from, to) in [
        ("$CB!=null", "$CB==null"),
        ("if($CB!=null)", "if($CB!==null)"),
        ("$CB.forEach", "$CB.map"),
        ("$CB.forEach", "other.forEach"),
        ("$KEY_0.push", "other.push"),
        ("$KEY_0.join", "other.join"),
        ("$KEY_0.join", "$CB.join"),
        ("join(\"\")", "join(\",\")"),
        ("join(\"\")", "join()"),
        ("let $KEY_0=[];", "let $KEY_0=[0];"),
        ("let $KEY_0=[];", "var $KEY_0=[];var $KEY_0;"),
    ] {
        denied(&UNITS.replace(from, to));
    }
    // No alternate branch is allowed.
    denied(&UNITS.replace(
        "if($CB!=null){$CB.forEach($CC=>{$KEY_0.push(b.String.fromCharCode($CC));});}",
        "if($CB!=null){$CB.forEach($CC=>{$KEY_0.push(b.String.fromCharCode($CC));});}else{other();}",
    ));
    // Exactly one callback push statement.
    denied(&UNITS.replace(
        "{$KEY_0.push(b.String.fromCharCode($CC));}",
        "{$KEY_0.push(b.String.fromCharCode($CC));other();}",
    ));
    // Exactly one forEach statement in the guard block.
    denied(&UNITS.replace(
        "$CB.forEach($CC=>{$KEY_0.push(b.String.fromCharCode($CC));});",
        "$CB.forEach($CC=>{$KEY_0.push(b.String.fromCharCode($CC));});other();",
    ));
    // No array write through a second callback statement.
    denied(&UNITS.replace(
        "{$KEY_0.push(b.String.fromCharCode($CC));}",
        "{$KEY_0=other;$KEY_0.push(b.String.fromCharCode($CC));}",
    ));
}

#[test]
fn code_units_refuses_changed_returns() {
    for (from, to) in [
        ("return $KEY_0.join(\"\");", "return $KEY_0;"),
        ("return $KEY_0.join(\"\");", "return $KEY_0.join(\"\");other();"),
        ("return $KEY_0.join(\"\");", "return $KEY_0.join(\"\")+other;"),
    ] {
        denied(&UNITS.replace(from, to));
    }
}

#[test]
fn code_units_refuses_changed_formals_and_callbacks() {
    for (from, to) in [
        ("function $THREAD_4_44100($CB){", "function $THREAD_4_44100($CB,other){"),
        ("function $THREAD_4_44100($CB){", "function $THREAD_4_44100($CB=other){"),
        ("function $THREAD_4_44100($CB){", "function $THREAD_4_44100(...$CB){"),
        ("function $THREAD_4_44100($CB){", "function $THREAD_4_44100({$CB}){"),
        ("function $THREAD_4_44100($CB){", "async function $THREAD_4_44100($CB){"),
        ("function $THREAD_4_44100($CB){", "function* $THREAD_4_44100($CB){"),
        ("$CC=>", "($CC=0)=>"),
        ("$CC=>", "(...$CC)=>"),
        ("$CC=>", "({$CC})=>"),
        ("$CC=>", "async $CC=>"),
    ] {
        denied(&UNITS.replace(from, to));
    }
}

#[test]
fn code_units_preserves_human_names_and_collision_guards() {
    let human = UNITS.replace("$THREAD_4_44100", "humanDecoder");
    assert_eq!(name_threads(&human, true).unwrap().source, human);

    for collision in [
        UNITS.replace(
            "function main(){",
            "function main(){let codeUnitsToString=17;useValue(codeUnitsToString);",
        ),
        UNITS.replace(
            "function main(){",
            "function main(){function codeUnitsToString(){}useValue(codeUnitsToString);",
        ),
    ] {
        assert!(parses(&collision), "{collision}");
        assert!(!named(&collision, "codeUnitsToString"), "{collision}");
    }
}

#[test]
fn code_units_edits_are_exact_identifier_only_replacements() {
    let r = name_threads(UNITS, true).unwrap();
    let applied: Vec<_> = r
        .report
        .outcomes
        .iter()
        .filter(|o| o.applied && o.proposed_name.as_deref() == Some("codeUnitsToString"))
        .collect();
    assert_eq!(applied.len(), 1, "expected one applied target");
    let mut replay = UNITS.to_string();
    for e in r.report.edits.iter().rev() {
        replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
    }
    assert_eq!(replay, r.source);
}

// The null-guarded char-code array builder has a sole bound return: `(output ? output :
// void 0).join.call(output, "")`.
const BOUND_BODY: &str = "function $THREAD_3_99318($BP) {\n\t\t\t\t\tlet $KEY_1 = [];\n\t\t\t\t\tif ($BP != null) {\n\t\t\t\t\t\t$BP.forEach($BQ => {\n\t\t\t\t\t\t\t$KEY_1.push(String.fromCharCode($BQ));\n\t\t\t\t\t\t});\n\t\t\t\t\t}\n\t\t\t\t\treturn ($KEY_1 ? $KEY_1 : void 0).join.call($KEY_1, \"\");\n\t\t\t\t}";

// The direct `codeUnitsToString` donor uses the same body with `return output.join("")`.
const DIRECT_BODY: &str = "function $THREAD_5_6691($BP) {\n\t\t\t\t\tlet $KEY_1 = [];\n\t\t\t\t\tif ($BP != null) {\n\t\t\t\t\t\t$BP.forEach($BQ => {\n\t\t\t\t\t\t\t$KEY_1.push(String.fromCharCode($BQ));\n\t\t\t\t\t\t});\n\t\t\t\t\t}\n\t\t\t\t\treturn $KEY_1.join(\"\");\n\t\t\t\t}";

fn wrap(body: &str, name: &str) -> String {
    format!("function main(){{\n{body}\nreturn {name};\n}}\nmain();")
}

// Keep the same unique-main carrier: the body function is returned, never
// called, so the added prelude cannot create a second broadest callable.
fn wrap_with_prelude(prelude: &str, body: &str, name: &str) -> String {
    format!("function main(){{\n{prelude}{body}\nreturn {name};\n}}\nmain();")
}

fn bound() -> String {
    wrap(BOUND_BODY, "$THREAD_3_99318")
}

#[test]
fn code_units_bound_fixtures_are_the_exact_witnesses() {
    assert_eq!(
        BOUND_BODY.len(),
        235,
        "byte callback witness must be 235 bytes"
    );
    assert_eq!(
        DIRECT_BODY.len(),
        201,
        "packed-pair donor must be 201 bytes"
    );
}

#[test]
fn code_units_accepts_bound_join_call_receiver_this_and_alpha() {
    let source = bound();
    assert!(named(&source, "codeUnitsToString"), "{source}");
    assert!(our_claim(&source), "not the fast claim: {source}");
    let alpha = wrap(
        &BOUND_BODY
            .replace("$THREAD_3_99318", "$THREAD_900_777_2")
            .replace("$BP", "$KEY_400")
            .replace("$BQ", "$KEY_900")
            .replace("$KEY_1", "buffer"),
        "$THREAD_900_777_2",
    );
    assert!(named(&alpha, "codeUnitsToString"), "{alpha}");
    assert!(our_claim(&alpha), "not the fast claim: {alpha}");
    // The same bounded return over the opaque host-namespace provider.
    let opaque = UNITS.replace(
        "return $KEY_0.join(\"\");",
        "return ($KEY_0 ? $KEY_0 : void 0).join.call($KEY_0, \"\");",
    );
    assert!(named(&opaque, "codeUnitsToString"), "{opaque}");
    assert!(our_claim(&opaque), "not the fast claim: {opaque}");
}

#[test]
fn code_units_retains_old_arm_owner_and_direct_donor() {
    // The direct donor keeps the PASS6 shared-runtime owner: older generation wins.
    let direct = wrap(DIRECT_BODY, "$THREAD_5_6691");
    assert!(named(&direct, "codeUnitsToString"), "{direct}");
    assert!(
        !our_claim(&direct),
        "fast rule overrode the direct arm: {direct}"
    );
    // The opaque direct arm is still the fast module's own.
    assert!(named(UNITS, "codeUnitsToString"));
    assert!(our_claim(UNITS));
}

#[test]
fn code_units_refuses_changed_bound_receiver_this_separator_and_effects() {
    for changed in [
        // Foreign explicit `this`.
        bound().replace(".join.call($KEY_1, \"\")", ".join.call(other, \"\")"),
        // Nonempty separator.
        bound().replace(".join.call($KEY_1, \"\")", ".join.call($KEY_1, \",\")"),
        // Wrong arity.
        bound().replace(".join.call($KEY_1, \"\")", ".join.call($KEY_1)"),
        bound().replace(
            ".join.call($KEY_1, \"\")",
            ".join.call($KEY_1, \"\", other)",
        ),
        // Not a `.join` method read.
        bound().replace("void 0).join.call", "void 0).call.call"),
        // Foreign method receiver.
        bound().replace("($KEY_1 ? $KEY_1 : void 0).join", "other.join"),
        bound().replace("($KEY_1 ? $KEY_1 : void 0)", "other"),
        // The truthy guard must be over the same proven output array.
        bound().replace("$KEY_1 ? $KEY_1 : void 0", "$KEY_0 ? $KEY_0 : void 0"),
        bound().replace("$KEY_1 ? $KEY_1 : void 0", "$KEY_1 ? other : void 0"),
        bound().replace("$KEY_1 ? $KEY_1 : void 0", "$KEY_1 ? $KEY_1 : void 1"),
        // Changed builder.
        bound().replace("$KEY_1.push", "other.push"),
        // Additional effects.
        bound().replace(
            "return ($KEY_1 ? $KEY_1 : void 0).join.call($KEY_1, \"\");",
            "return ($KEY_1 ? $KEY_1 : void 0).join.call($KEY_1, \"\");other();",
        ),
        bound().replace(
            "return ($KEY_1 ? $KEY_1 : void 0).join.call($KEY_1, \"\");",
            "return ($KEY_1 ? $KEY_1 : void 0).join.call($KEY_1, \"\")+other;",
        ),
    ] {
        denied(&changed);
    }
}

#[test]
fn code_units_refuses_noncanonical_canonical_provider_roots() {
    // An unrelated unbound root is not the canonical namespace. Being unbound alone must not
    // make `other` acceptable.
    denied(&bound().replace("String.fromCharCode", "other.fromCharCode"));
    // A resolved foreign provider alias is not canonical either, even though its
    // terminal origin is unbound.
    let foreign = wrap_with_prelude(
        "const foreign=provider;\n",
        &BOUND_BODY.replace("String.fromCharCode", "foreign.fromCharCode"),
        "$THREAD_3_99318",
    );
    denied(&foreign);
    // A local `String` shadow is resolved, so the scoped path proof has no
    // unshadowed global root to accept.
    denied(&wrap_with_prelude(
        "const String=foreign;\n",
        BOUND_BODY,
        "$THREAD_3_99318",
    ));
    // A parameter `String` shadow is equally non-canonical.
    let parameter =
        format!("function main(String){{\n{BOUND_BODY}\nreturn $THREAD_3_99318;\n}}\nmain();");
    denied(&parameter);
    // The canonical arm proves the literal `fromCharCode` operation only.
    denied(&bound().replace("String.fromCharCode", "String.fromCodePoint"));
    denied(&bound().replace("String.fromCharCode", "String.fromCharCodeAt"));
}

#[test]
fn code_units_refuses_resolved_distinct_array_aliases_and_output_writes() {
    // `other` is a real second array symbol, not an unresolved spelling.
    let with_other = wrap_with_prelude("let other=[];\n", BOUND_BODY, "$THREAD_3_99318");
    assert!(named(&with_other, "codeUnitsToString"), "{with_other}");
    assert!(our_claim(&with_other), "not the fast claim: {with_other}");
    for changed in [
        // A resolved distinct array as the method receiver.
        with_other.replace("($KEY_1 ? $KEY_1 : void 0).join", "other.join"),
        // A resolved distinct array as the guard consequent.
        with_other.replace("$KEY_1 ? $KEY_1 : void 0", "$KEY_1 ? other : void 0"),
        // A resolved distinct array as the explicit `this`.
        with_other.replace(".join.call($KEY_1, \"\")", ".join.call(other, \"\")"),
        // An explicit write to the output local still preserves the three
        // top-level statements, so the immutable-local guard is what refuses it.
        with_other.replace("if ($BP != null)", "if (($KEY_1 = other) != null)"),
    ] {
        denied(&changed);
    }
}
