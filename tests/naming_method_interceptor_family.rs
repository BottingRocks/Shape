use shape::naming::name_threads;
const METHOD_SCOPE: &str = include_str!("fixtures/method_interception/full_scope.js");
fn method_change(source: &str, from: &str, to: &str) -> String {
    let start = source.find("function $THREAD_8_111768(").unwrap();
    let end = source[start..].find("\n\t\tfunction ").unwrap() + start;
    let own = &source[start..end];
    assert!(own.contains(from), "missing {from}");
    let mut result = source.to_string();
    result.replace_range(start..end, &own.replacen(from, to, 1));
    result
}
fn method_single() -> String {
    let s = method_change(METHOD_SCOPE, "let $AX, $AY;", "let $AX;");
    let s = method_change(&s, "\n\t\t\t\t\t$AY = null;", "");
    let s = method_change(&s, "\n\t\t\t\t\t$AY = $AX;", "");
    // These four reads belong to this installer and now consume its sole cache.
    let start = s.find("function $THREAD_8_111768(").unwrap();
    let end = s[start..].find("\n\t\tfunction ").unwrap() + start;
    let own = s[start..end].replace("$AY", "$AX");
    let mut result = s;
    result.replace_range(start..end, &own);
    result
}
fn lowered_methods(s: &str) -> usize {
    name_threads(s, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|o| {
            o.evaluators
                .iter()
                .any(|x| x == "owned_method_interceptor_lowered_external_helpers")
        })
        .count()
}
#[test]
fn method_single_cache_closed_protocol_and_negative_frontiers() {
    let s = method_single();
    assert_eq!(lowered_methods(&s), 1);
    assert_eq!(
        lowered_methods(
            &s.replace("$KEY_10", "originalValue")
                .replace("$AX", "cachedRecord")
        ),
        1
    );
    for (from, to) in [
        ("if ($AT !== $AX.object)", "if ($AN !== $AX.object)"),
        ("$AX = void 0;", "$AX = sideEffect();"),
        ("$AZ >= $KEY_241[$AO].length", "$AZ > $KEY_241[$AO].length"),
        ("++$AZ;", "--$AZ;"),
        ("$KEY_10 = $AX.original;", "$KEY_10 = $AX.result;"),
        ("$AX.result != null", "$AX.original != null"),
        ("$KEY_7 = false;", "$KEY_7 = true;"),
        ("let $KEY_7 = $AP;", "const $KEY_7 = $AP;"),
        (
            "$KEY_127($KEY_10, this, $BH)",
            "$KEY_127($KEY_10, $AN, $BH)",
        ),
        ("onBeforeInvoke: $KEY_9", "onBeforeInvoke: $KEY_4"),
        (
            "Object.defineProperty($AT, $AO, $AR)",
            "Object.defineProperty($AN, $AO, $AR)",
        ),
        ("return $AW;", "effect(); return $AW;"),
    ] {
        assert_eq!(
            lowered_methods(&method_change(&s, from, to)),
            0,
            "accepted {from}"
        );
    }
    for (from, to) in [
        (
            "$KEY_373 = $H(Object.call);",
            "$KEY_373 = $H(Object.call); $KEY_373 = $H(Object.apply);",
        ),
        ("$KEY_373 = $H(Object.call);", "/*missing*/"),
        (
            "let $KEY_129 = $THREAD_8_111768;",
            "let $KEY_129 = $THREAD_8_111768; publish($KEY_129);",
        ),
    ] {
        assert!(s.contains(from));
        assert_eq!(
            lowered_methods(&s.replace(from, to)),
            0,
            "accepted origin/exposure {from}"
        );
    }
    for global in ["Object", "Array", "Function"] {
        assert_eq!(
            lowered_methods(&method_change(
                &s,
                "let $KEY_7 = $AP;",
                &format!("let {global}=foreign;let $KEY_7=$AP;")
            )),
            0
        );
    }
}
#[test]
fn method_boolean_cache_is_a_complete_branch_not_a_guessed_alias() {
    let s = method_single();
    let s = method_change(&s, "let $AX;", "let cacheMatched;let $AX;");
    let s = method_change(&s, "$AX = null;", "cacheMatched=false;$AX = null;");
    let s = method_change(
        &s,
        "\n\t\t\t\t}\n\t\t\t\tif ($AX != null)",
        "\n cacheMatched=$AX!=null;\n\t\t\t\t}\n\t\t\t\tif (cacheMatched)",
    );
    assert_eq!(lowered_methods(&s), 1);
    for (from, to) in [
        ("cacheMatched=false;", "cacheMatched=true;"),
        ("cacheMatched=$AX!=null", "cacheMatched=$AX==null"),
        ("if (cacheMatched)", "if (!cacheMatched)"),
        (
            "cacheMatched=$AX!=null;",
            "sideEffect();cacheMatched=$AX!=null;",
        ),
    ] {
        assert_eq!(
            lowered_methods(&method_change(&s, from, to)),
            0,
            "accepted {from}"
        );
    }
}
#[test]
fn method_copied_descriptor_and_reset_free_cache_are_fully_owned() {
    let s = method_single();
    let s = method_change(&s, "let $AR;", "let $AR, descriptorCopy;");
    let s = method_change(&s, "$AR = void 0;", "$AR = void 0;descriptorCopy=void 0;");
    let s = method_change(&s, "$AR = null;", "$AR = null;descriptorCopy=null;");
    let s = method_change(
        &s,
        "desc: $AS\n\t\t\t\t};",
        "desc: $AS\n\t\t\t\t};descriptorCopy=$AR;",
    );
    let start = s.find("let $AT = $AR.containingObj;").unwrap();
    let end = s[start..].find("\n\t\tfunction ").unwrap() + start;
    let tail = s[start..end].replace("$AR", "descriptorCopy");
    let mut copied = s;
    copied.replace_range(start..end, &tail);
    assert_eq!(lowered_methods(&copied), 1);
    let no_reset = method_change(&copied, "$AX = void 0;", "");
    assert_eq!(lowered_methods(&no_reset), 1);
    for (from, to) in [
        ("descriptorCopy=$AR;", "descriptorCopy=$AN;"),
        ("descriptorCopy=null;", "descriptorCopy={};"),
        (
            "$AR = null;descriptorCopy=null;",
            "$AR = null;descriptorCopy=$AR.desc;",
        ),
        ("if ($AR == null)", "if (descriptorCopy == null)"),
        ("descriptorCopy=void 0;", "descriptorCopy=sideEffect();"),
    ] {
        assert_eq!(
            lowered_methods(&method_change(&copied, from, to)),
            0,
            "accepted descriptor {from}"
        );
    }
}

#[test]
fn method_loop_boolean_flag_is_local_and_total_on_every_exit() {
    let s = method_single();
    let s = method_change(&s, "let $AX;", "let cacheMatched;let $AX;");
    let s = method_change(&s, "$AX = null;", "cacheMatched=false;$AX = null;");
    let s = method_change(
        &s,
        "$AX = null;\n\t\t\t\t\t\t\tbreak;",
        "cacheMatched=false;$AX = null;\n\t\t\t\t\t\t\tbreak;",
    );
    let s = method_change(
        &s,
        "\n\t\t\t\t\t\tbreak;\n\t\t\t\t\t}",
        "\ncacheMatched=$AX!=null;\n\t\t\t\t\t\tbreak;\n\t\t\t\t\t}",
    );
    let s = method_change(&s, "if ($AX != null)", "if (cacheMatched)");
    assert_eq!(lowered_methods(&s), 1);
    assert_eq!(
        lowered_methods(
            &s.replace("cacheMatched", "foundRecord")
                .replace("$AX", "record")
        ),
        1
    );
    for (from, to) in [
        ("cacheMatched=false;", "cacheMatched=true;"),
        ("cacheMatched=$AX!=null;", "cacheMatched=$AX==null;"),
        ("if (cacheMatched)", "if (!cacheMatched)"),
        (
            "cacheMatched=$AX!=null;",
            "capture($AX);cacheMatched=$AX!=null;",
        ),
    ] {
        assert_eq!(
            lowered_methods(&method_change(&s, from, to)),
            0,
            "accepted {from}"
        );
    }
    let captured = method_change(&s, "let cacheMatched;", "").replacen(
        "function $THREAD_8_111768(",
        "let cacheMatched;function $THREAD_8_111768(",
        1,
    );
    assert_eq!(
        lowered_methods(&captured),
        0,
        "cannot erase a captured flag writer"
    );
}

fn payload_variant(split: bool) -> String {
    let s = method_single();
    let start="\t\t\t\t\t$KEY_5 = arguments;\n\t\t\t\t\tlet $BI;\n\t\t\t\t\tlet $BJ = {\n\t\t\t\t\t\targs: $KEY_5,\n\t\t\t\t\t\tthisObj: this,\n\t\t\t\t\t\tthrew: true,\n\t\t\t\t\t\tresult: null\n\t\t\t\t\t};";
    let replacement = if split {
        "let payloadRecord={args:void 0,thisObj:void 0};$KEY_5=arguments;payloadRecord.args=$KEY_5;payloadRecord.thisObj=this;payloadRecord.threw=true;payloadRecord.result=null;let $BJ=payloadRecord;let $BI;"
    } else {
        "$KEY_5={args:arguments,thisObj:this,threw:true,result:null};let $BJ=$KEY_5;let $BI;"
    };
    method_change(&s, start, replacement)
}

#[test]
fn method_fresh_failure_payload_transport_has_no_escape_or_other_origin() {
    for split in [false, true] {
        let s = payload_variant(split);
        assert_eq!(lowered_methods(&s), 1, "split={split}");
        assert_eq!(
            lowered_methods(
                &s.replace("$BJ", "failurePayload")
                    .replace("payloadRecord", "temporaryRecord")
            ),
            1
        );
        for (from, to) in [
            (
                if split {
                    "payloadRecord.threw=true"
                } else {
                    "threw:true"
                },
                if split {
                    "payloadRecord.threw=false"
                } else {
                    "threw:false"
                },
            ),
            (
                if split {
                    "payloadRecord.thisObj=this"
                } else {
                    "thisObj:this"
                },
                if split {
                    "payloadRecord.thisObj=$AN"
                } else {
                    "thisObj:$AN"
                },
            ),
            (
                if split {
                    "payloadRecord.result=null"
                } else {
                    "result:null"
                },
                if split {
                    "payloadRecord.result=$AN"
                } else {
                    "result:$AN"
                },
            ),
            ("$KEY_213($KEY_10, $BH)", "$KEY_213($BH, $KEY_10)"),
            ("this instanceof $THREAD_0_69749", "this instanceof Object"),
            (
                "$KEY_127($KEY_10, this, $BH)",
                "$KEY_127($KEY_10, $AN, $BH)",
            ),
            ("$KEY_4.notify($BJ)", "$KEY_9.notify($BJ)"),
            ("$BG.bypassResult.throw", "$BG.bypassResult.value"),
            ("let $BJ=", "effect();let $BJ="),
        ] {
            assert_eq!(
                lowered_methods(&method_change(&s, from, to)),
                0,
                "accepted {from}, split={split}"
            );
        }
        if split {
            for (from, to) in [
                ("payloadRecord.args=$KEY_5", "payloadRecord.args=$BH"),
                ("payloadRecord.threw=true", "payloadRecord.threw=false"),
                (
                    "let $BJ=payloadRecord",
                    "publish(payloadRecord);let $BJ=payloadRecord",
                ),
                ("let $BJ=payloadRecord", "let $BJ={}"),
                ("payloadRecord.thisObj=this", "payloadRecord.thisObj=$AN"),
            ] {
                assert_eq!(
                    lowered_methods(&method_change(&s, from, to)),
                    0,
                    "accepted {from}"
                );
            }
            let captured = method_change(&s, "let payloadRecord=", "payloadRecord=").replacen(
                "function $THREAD_8_111768(",
                "let payloadRecord;function $THREAD_8_111768(",
                1,
            );
            assert_eq!(
                lowered_methods(&captured),
                0,
                "cannot collapse an external payload"
            );
        }
    }
}

#[path = "naming_posthost_function_hooks.rs"]
mod accepted_method_controls;

#[test]
fn method_direct_originals_return_and_loop_record_copy_keep_all_exits() {
    let s = method_change(
        &method_single(),
        "$AW = { originals: $AW };\n\t\t\t\treturn $AW;",
        "return {originals:$AW};",
    );
    assert_eq!(lowered_methods(&s), 1);
    assert_eq!(
        lowered_methods(&method_change(
            &s,
            "return {originals:$AW}",
            "return {originals:$AN}"
        )),
        0
    );
    let s = method_change(&s, "let $AX;", "let $AX, cachedCopy;");
    let s = method_change(&s, "$AX = null;", "$AX = null;cachedCopy=null;");
    let s = method_change(
        &s,
        "$AX = null;\n\t\t\t\t\t\t\tbreak;",
        "$AX = null;cachedCopy=null;\n\t\t\t\t\t\t\tbreak;",
    );
    let s = method_change(
        &s,
        "\n\t\t\t\t\t\tbreak;\n\t\t\t\t\t}",
        "\ncachedCopy=$AX;\n\t\t\t\t\t\tbreak;\n\t\t\t\t\t}",
    );
    let s = method_change(&s, "$AX = void 0;", "");
    let s = method_change(&s, "$AX.result != null", "cachedCopy.result != null");
    let s = method_change(&s, "return $AX.result", "return cachedCopy.result");
    let s = method_change(
        &s,
        "$KEY_10 = $AX.original",
        "$KEY_10 = cachedCopy.original",
    );
    assert_eq!(lowered_methods(&s), 1);
    for (from, to) in [
        ("cachedCopy=$AX", "cachedCopy=$AN"),
        ("cachedCopy=null", "cachedCopy={}"),
        ("cachedCopy=$AX", "effect();cachedCopy=$AX"),
        ("if ($AT !== $AX.object)", "if ($AN !== $AX.object)"),
    ] {
        assert_eq!(
            lowered_methods(&method_change(&s, from, to)),
            0,
            "accepted {from}"
        );
    }
    let captured = method_change(&s, "let $AX, cachedCopy;", "let $AX;").replacen(
        "function $THREAD_8_111768(",
        "let cachedCopy;function $THREAD_8_111768(",
        1,
    );
    assert_eq!(lowered_methods(&captured), 0);
}

fn method_with_cache_helper() -> String {
    let s = split_frozen_test(&method_single());
    let start = s.find("let $AX;").unwrap();
    let end = s[start..].find("if ($AX != null)").unwrap() + start;
    let mut s = s;
    s.replace_range(start..end, "let $AX=cacheLookup($AT,$AO);\n");
    let helper="let cacheLookup=ownedLookup;function ownedLookup(object,key){if($KEY_241[key]==null){return null;}let cursor=0;while(cursor<$KEY_241[key].length){if(object===$KEY_241[key][cursor].object){return $KEY_241[key][cursor];}else{++cursor;continue;}}return null;}";
    s.replacen(
        "function $THREAD_8_111768(",
        &format!("{helper}function $THREAD_8_111768("),
        1,
    )
}

#[test]
fn method_cache_helper_proves_own_body_scope_initialization_and_publication_registry() {
    let s = method_with_cache_helper();
    assert_eq!(lowered_methods(&s), 1);
    assert_eq!(
        lowered_methods(
            &s.replace("ownedLookup", "findOwnedRecord")
                .replace("cacheLookup", "findRecord")
                .replace("cursor", "scanIndex")
        ),
        1
    );
    for (from, to) in [
        (
            "cursor<$KEY_241[key].length",
            "cursor<=$KEY_241[key].length",
        ),
        (
            "object===$KEY_241[key][cursor].object",
            "object!==$KEY_241[key][cursor].object",
        ),
        ("return $KEY_241[key][cursor]", "return $KEY_241[key][0]"),
        ("++cursor", "--cursor"),
        ("++cursor;continue;", "++cursor;break;"),
        (
            "return null;}let cursor",
            "effect();return null;}let cursor",
        ),
        (
            "let cacheLookup=ownedLookup;",
            "let cacheLookup=ownedLookup;cacheLookup=foreign;",
        ),
        (
            "let cacheLookup=ownedLookup;",
            "let cacheLookup=ownedLookup;ownedLookup=foreign;",
        ),
        (
            "let cacheLookup=ownedLookup;",
            "let cacheLookup=bridge;let bridge=ownedLookup;",
        ),
        ("cacheLookup($AT,$AO)", "cacheLookup($AN,$AO)"),
        ("cacheLookup($AT,$AO)", "cacheLookup($AT,$AN)"),
        ("if($KEY_241[key]==null)", "if($KEY_241[key]!=null)"),
    ] {
        assert!(s.contains(from));
        assert_eq!(
            lowered_methods(&s.replacen(from, to, 1)),
            0,
            "accepted {from}"
        );
    }
    // An initialized alias to the same value is not the same captured binding.
    let other = s.replace("$KEY_241[key]", "otherRegistry[key]").replacen(
        "let cacheLookup=ownedLookup;",
        "let otherRegistry=$KEY_241;let cacheLookup=ownedLookup;",
        1,
    );
    assert_eq!(lowered_methods(&other), 0);
}

fn split_frozen_test(s: &str) -> String {
    method_change(
        s,
        "if ($AU === false && $AV === false ||",
        "if($AU===false){$AV=$AV===false;}else{$AV=$AU===false;}if($AV||",
    )
}

#[test]
fn method_split_frozen_test_accounts_for_exact_writable_carrier() {
    let s = split_frozen_test(&method_single());
    assert_eq!(lowered_methods(&s), 1);
    for (from, to) in [
        ("$AV=$AV===false", "$AV=$AV!==false"),
        ("$AV=$AU===false", "$AV=$AU!==false"),
        ("if($AV||", "if(!$AV||"),
        ("$AV=$AU===false;", "effect();$AV=$AU===false;"),
        ("if($AU===false)", "if($AU==false)"),
    ] {
        assert_eq!(
            lowered_methods(&method_change(&s, from, to)),
            0,
            "accepted {from}"
        );
    }
}

#[test]
fn method_descriptor_helper_is_resolved_independently_of_the_caller() {
    let mut s = method_with_cache_helper();
    let start = s.find("let $AQ = $AN;").unwrap();
    let end = s[start..].find("if ($AR == null)").unwrap() + start;
    s.replace_range(start..end, "let $AR=descriptorLookup($AN,$AO);\n");
    let helper="let descriptorLookup=lookupDescriptor;function lookupDescriptor(object,key){let cursor=object;while(cursor!=null){let descriptor=Object.getOwnPropertyDescriptor(cursor,key);if(descriptor!=null){cursor={containingObj:cursor,desc:descriptor};return cursor;}else{cursor=Object.getPrototypeOf(cursor);continue;}}return null;}";
    let s = s.replacen(
        "function $THREAD_8_111768(",
        &format!("{helper}function $THREAD_8_111768("),
        1,
    );
    assert_eq!(lowered_methods(&s), 1);
    assert_eq!(
        lowered_methods(
            &s.replace("descriptorLookup", "readDescriptor")
                .replace("lookupDescriptor", "findDescriptor")
        ),
        1
    );
    for (from, to) in [
        (
            "Object.getOwnPropertyDescriptor(cursor,key)",
            "Object.getOwnPropertyDescriptor(object,key)",
        ),
        (
            "cursor=Object.getPrototypeOf(cursor)",
            "cursor=Object.getPrototypeOf(object)",
        ),
        ("if(descriptor!=null)", "if(descriptor==null)"),
        ("containingObj:cursor", "containingObj:object"),
        ("desc:descriptor", "desc:object"),
        ("return cursor;", "effect();return cursor;"),
        (
            "let descriptorLookup=lookupDescriptor;",
            "let descriptorLookup=lookupDescriptor;descriptorLookup=foreign;",
        ),
        (
            "let descriptorLookup=lookupDescriptor;",
            "let descriptorLookup=lookupDescriptor;lookupDescriptor=foreign;",
        ),
    ] {
        assert!(s.contains(from));
        assert_eq!(
            lowered_methods(&s.replacen(from, to, 1)),
            0,
            "accepted {from}"
        );
    }
}

#[test]
fn method_cursor_rebound_descriptor_record_has_no_later_cursor_read() {
    let s = method_change(&method_single(), "let $AR;", "let $AR, descriptorCopy;");
    let s = method_change(&s, "$AR = void 0;", "$AR = void 0;descriptorCopy=void 0;");
    let s = method_change(&s, "$AR = null;", "$AR = null;descriptorCopy=null;");
    let s = method_change(&s, "$AR = {\n", "$AQ = {\n");
    let s = method_change(
        &s,
        "desc: $AS\n\t\t\t\t};",
        "desc: $AS\n\t\t\t\t};$AR=$AQ;descriptorCopy=$AR;",
    );
    let start = s.find("let $AT = $AR.containingObj;").unwrap();
    let end = s[start..].find("\n\t\tfunction ").unwrap() + start;
    let tail = s[start..end].replace("$AR", "descriptorCopy");
    let mut s = s;
    s.replace_range(start..end, &tail);
    assert_eq!(lowered_methods(&s), 1);
    for (from, to) in [
        ("$AR=$AQ", "$AR=$AN"),
        ("descriptorCopy=$AR", "descriptorCopy=$AN"),
        ("$AR=$AQ", "effect();$AR=$AQ"),
        ("desc: $AS", "desc: $AQ"),
        (
            "let $AT = descriptorCopy.containingObj;",
            "let $AT = $AQ.containingObj;",
        ),
    ] {
        assert_eq!(
            lowered_methods(&method_change(&s, from, to)),
            0,
            "accepted {from}"
        );
    }
}

#[test]
fn method_failure_payload_preserves_exposed_property_order() {
    for (split, from, to) in [
        (
            false,
            "args:arguments,thisObj:this",
            "thisObj:this,args:arguments",
        ),
        (
            true,
            "args:void 0,thisObj:void 0",
            "thisObj:void 0,args:void 0",
        ),
    ] {
        let s = payload_variant(split);
        assert_eq!(
            lowered_methods(&method_change(&s, from, to)),
            0,
            "payload property order is observable"
        );
    }
}
