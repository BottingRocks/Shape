use shape::naming::name_threads;
fn named(source: &str, name: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| o.applied && o.proposed_name.as_deref() == Some(name))
}
fn wrap(body: &str, captures: &str) -> String {
    format!("function main(){{{captures}{body}return 1;}}main();")
}
fn check(body: &str, captures: &str, name: &str, changes: &[(&str, &str)]) {
    assert!(named(&wrap(body, captures), name), "missing {name}");
    assert!(
        named(
            &wrap(
                &body
                    .replace("$DH", "stamp")
                    .replace("$DG", "input")
                    .replace("$AZ", "stamp")
                    .replace("$BA", "input"),
                captures
            ),
            name
        ),
        "alpha {name}"
    );
    for (a, b) in changes {
        assert!(body.contains(a), "missing mutation {a}");
        assert!(
            !named(&wrap(&body.replace(a, b), captures), name),
            "accepted {name} mutation {a}"
        );
    }
}
#[test]
fn orientation_exact_moments_and_error_forwarding() {
    let body = include_str!("fixtures/event-timing-and-encoding/orientation_moments.js");
    check(body,"let $KEY_190=function(){return Date.now();};let $KEY_22=0,$KEY_30=0,$KEY_29=0,$KEY_26=0,$KEY_21=0,$KEY_15=0,$KEY_10=0,$KEY_1=0,$KEY_9=0,$KEY_0=0,$KEY_11={};","recordOrientationMoments",&[
 ("$KEY_22 <= 1","$KEY_22 < 1"),("$DG.alpha * $DG.alpha","$DG.alpha * $DG.beta"),("$DH - $KEY_30","$KEY_30 - $DH"),("$KEY_22 - 2","$KEY_22 - 3"),("$KEY_11.error(e)","$KEY_11.error($DG)"),("++$KEY_22","$KEY_22++"),("$KEY_30 = $DH;","$KEY_30 = $KEY_29;")]);
}
#[test]
fn interval_order_and_extrema_contract() {
    let body = include_str!("fixtures/event-timing-and-encoding/interval_extrema.js");
    check(body,"let $KEY_2=function(){return Date.now();};let $KEY_39=0,$KEY_7={},$KEY_33,$KEY_3,$KEY_13=0;","recordIntervalAndExtrema",&[
 ("$KEY_0 - $DG","$DG - $KEY_0"),("&&","||"),("$KEY_0 - $DG > $KEY_33","$KEY_0 - $DG >= $KEY_33"),("$KEY_3 = $KEY_0 - $DG","$KEY_33 = $KEY_0 - $DG"),("++$KEY_13","$KEY_13++"),("$KEY_39 = $KEY_0;","$KEY_39 = $DG;")]);
}
#[test]
fn digest_guard_promise_and_both_fallbacks() {
    let body = include_str!("fixtures/event-timing-and-encoding/script_digest_dispatch.js");
    check(body,"let $KEY_28={},$KEY_12={},$KEY_45='', $KEY_32='';let $KEY_280=function(a,b){return a;};let $KEY_38=function(a,b){};","dispatchScriptDigestUrl",&[
 ("typeof $KEY_28.global.crypto.subtle === \"object\"","typeof $KEY_28.global.crypto.subtle === \"function\""),("$KEY_28.scrBytes.sidx.length > 0","$KEY_28.scrBytes.sidx.length >= 0"),("$DG?.hash","$DG.hash"),("$DI.call($DH","$DI.call($KEY_45"),("\"&c=\" + $DK","\"&d=\" + $DK"),("$DG?.sidx || []","$DG?.sidx && []"),("$KEY_38($KEY_45, [])","$KEY_38($KEY_45, [1])")]);
}
#[test]
fn wheel_frame_requires_owned_factory_and_writer() {
    let body = include_str!("fixtures/event-timing-and-encoding/wheel_counter_frame.js");
    let captures=format!("let $KEY_289=99;{}{}{}let $KEY_327=$THREAD_2_103652;let $KEY_245=$KEY_327();let $KEY_120=$THREAD_0_107316;let $KEY_191=$THREAD_7_39465;let $KEY_16=0,$KEY_22=0,$KEY_1=0,$KEY_5={{}},$KEY_4=null,$KEY_18={{}};",include_str!("fixtures/event-timing-and-encoding/createKeystreamByteSource.js"),include_str!("fixtures/event-timing-and-encoding/writeNumber.js"),include_str!("fixtures/event-timing-and-encoding/createElapsedTimer.js"));
    check(
        body,
        &captures,
        "emitWheelCounterFrame",
        &[
            ("> 500", ">= 500"),
            (
                "$KEY_191($BA, $KEY_22, $BB, 3)",
                "$KEY_191($BA, $KEY_1, $BB, 3)",
            ),
            ("$BD($BA, $KEY_1, $BB, $BC)", "$BD($BA, $KEY_1, $BB, 3)"),
            ("$KEY_191;", "$KEY_120;"),
            ("$BB]);", "[]]);"),
            ("\"wheel\", $KEY_4, false", "\"wheel\", $KEY_4, true"),
            ("$KEY_5.error(e)", "$KEY_5.error($BB)"),
        ],
    );
    let fake = captures.replace(
        "let $KEY_191=$THREAD_7_39465;",
        "let $KEY_191=function(a,b,c,d){return d;};",
    );
    assert!(!named(&wrap(body, &fake), "emitWheelCounterFrame"));
}

#[test]
fn interval_clock_is_initialized_in_each_exclusive_setup_arm() {
    let body = include_str!("fixtures/event-timing-and-encoding/interval_extrema.js");
    let timer = include_str!("fixtures/event-timing-and-encoding/createElapsedTimer.js");
    let prefix = format!("{timer}let factory=$THREAD_2_103652;let $KEY_2;let $KEY_39=0,$KEY_7={{}},$KEY_33,$KEY_3,$KEY_13=0;");
    let second = body.replace("$THREAD_7_126914_2", "$THREAD_9_81234");
    let source = wrap(
        &format!("if(flag){{$KEY_2=factory();{body}}}else{{$KEY_2=factory();{second}}}"),
        &prefix,
    );
    let count = |text: &str| {
        name_threads(text, true)
            .unwrap()
            .report
            .outcomes
            .iter()
            .filter(|o| {
                o.applied
                    && o.proposed_name
                        .as_deref()
                        .is_some_and(|n| n.starts_with("recordIntervalAndExtrema"))
            })
            .count()
    };
    assert_eq!(count(&source), 2);
    assert_eq!(
        count(&source.replace("let $KEY_2;", "let $KEY_2=host.clearInterval;")),
        2
    );
    assert_eq!(
        count(&source.replacen("$KEY_2=factory();", "$KEY_2=other;", 1)),
        0
    );
    assert_eq!(
        count(&source.replace(
            "let $KEY_2;",
            "let $KEY_2;function hidden(){$KEY_2=factory();}"
        )),
        0
    );
    assert_eq!(count(&source.replace("}else{", "}if(other){")), 0);
    let late = wrap(
        &format!("if(flag){{{body}$KEY_2=factory();}}else{{$KEY_2=factory();{second}}}"),
        &prefix,
    );
    assert_eq!(count(&late), 1);
    let fallthrough =
        format!("if(flag){{if(inner){{$KEY_2=factory();{body}return;}}}}$KEY_2=factory();{second}");
    assert_eq!(count(&wrap(&fallthrough, &prefix)), 2);
    assert_eq!(
        count(&wrap(&fallthrough.replace("return;", ""), &prefix)),
        0
    );
    let escape = fallthrough.replace("return;", "if(escape){break;}return;");
    assert_eq!(count(&wrap(&format!("for(;;){{{escape}}}"), &prefix)), 0);
    let finally_escape=wrap(&format!("while(flag){{try{{if(inner){{$KEY_2=factory();{body}return;}}}}finally{{break;}}}}$KEY_2=factory();{second}"),&prefix);
    assert_eq!(count(&finally_escape), 0);
    let data = "let $KEY_2;let $KEY_39=0,$KEY_7={},$KEY_33,$KEY_3,$KEY_13=0;";
    let factory = format!("{timer}let factory=$THREAD_2_103652;");
    assert_eq!(
        count(&wrap(
            &format!("let setup=()=>{{{data}{fallthrough}}};"),
            &factory
        )),
        2
    );
}
