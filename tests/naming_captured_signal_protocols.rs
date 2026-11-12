use shape::naming::name_threads;
const TRAITS: &str = include_str!("fixtures/captured-signal-protocols/function-traits.js");
fn wrapped(s: &str) -> String {
    format!("function main(){{ let $KEY_272 = /x/; let $KEY_64 = Function.prototype.toString; let $KEY_221 = Object.hasOwn; {s} }}main();")
}
fn count(s: &str) -> usize {
    name_threads(s, true).unwrap().report.outcomes.iter().filter(|o|o.applied && o.role_name.as_deref()==Some("collectCallabilitySourceTextAndPrototypeDeletionTraits")).count()
}
#[test]
fn source_text_traits_complete_body_and_near_misses() {
    assert_eq!(count(&wrapped(TRAITS)),1);
    assert_eq!(count(&wrapped(TRAITS).replace("$CG","candidate").replace("$KEY_3","alias").replace("$KEY_272","tester")),1);
    for (from,to) in [
        ("$KEY_64.call($CG)","$KEY_221.call($CG)"),
        ("$CI = $KEY_221($KEY_3, \"prototype\")","$CI = $KEY_221($KEY_3, \"other\")"),
        ("delete $CG.prototype","delete $KEY_3.prototype"),
        ("$CJ = true","$CJ = false"),
        ("catch (e) {}","catch (e) { sideEffect(); }"),
        ("c: !$CI","c: $CI"),
        ("e: $KEY_3","e: $CG"),
        ("return $KEY_2;","sideEffect(); return $KEY_2;"),
        ("if ($KEY_2 && !$CI)","if ($KEY_2 || !$CI)"),
        ("$KEY_64.call($KEY_3)","$KEY_64.call($CG)"),
        ("$KEY_3 = $CG;","$KEY_3 = {};"),
    ] {assert!(TRAITS.contains(from),"missing mutation {from}");assert_eq!(count(&wrapped(&TRAITS.replace(from,to))),0,"accepted {from}");}

}
const VECTOR: &str = include_str!("fixtures/captured-signal-protocols/vector-history.js");
fn vectors(s: &str) -> usize {name_threads(&format!("function main(){{{s}}}main();"),true).unwrap().report.outcomes.iter().filter(|o|o.applied&&o.role_name.as_deref()==Some("recordPixelVectorSample")).count()}
#[test]
fn merged_null_length_guard_whole_protocol() {
    assert_eq!(vectors(VECTOR),1);
    assert_eq!(vectors(&VECTOR.replace("$CV","vector").replace("$CW","history").replace("$CZ","rejected")),1);
    for (from,to) in [
        ("$KEY_0 == null || $CY == null", "$KEY_0 == null && $CY == null"),
        ("$CZ = true", "$CZ = false"),
        ("$KEY_0.length !== $CY.length", "$KEY_0.length === $CY.length"),
        ("if ($CZ)", "if (!$CZ)"),
        ("$KEY_0[$DA] === $CY[$DA]", "$KEY_0[$DA] === $CY[0]"),
        ("Array.prototype.slice.call($KEY_0)", "Array.prototype.slice.call($CW)"),
        ("$CW.a.push($CX)", "$CW.a.push($KEY_0)"),
        ("return $DB;", "sideEffect(); return $DB;"),
    ] {assert!(VECTOR.contains(from));assert_eq!(vectors(&VECTOR.replace(from,to)),0,"accepted {from}");}
    assert_eq!(vectors(&format!("function outer(Array){{{VECTOR}}}")),0);
    assert_eq!(vectors(&format!("function outer(Number){{{VECTOR}}}")),0);
}
const DIGEST: &str=include_str!("fixtures/captured-signal-protocols/script-digest.js");
fn role_count(s:&str,role:&str)->usize{name_threads(s,true).unwrap().report.outcomes.iter().filter(|o|o.applied&&o.role_name.as_deref()==Some(role)).count()}
#[test]
fn digest_inline_indices_keeps_exact_receiver_and_all_fallbacks(){
    let wrap=|s:&str|format!("function main(){{let $KEY_6={{}},$KEY_3={{}},$KEY_15='', $KEY_2='';let $KEY_200=function(a,b){{return a;}};let $KEY_14=function(a,b){{}};{s}}}main();");
    let role="dispatchScriptDigestUrl";
    assert_eq!(role_count(&wrap(DIGEST),role),1);
    assert_eq!(role_count(&wrap(&DIGEST.replace("$BH","promise").replace("$BI","result")),role),1);
    for(from,to)in[("$KEY_14($KEY_15, $BI?.sidx || [])","$KEY_14($KEY_15, $BI?.sidx && [])"),("$BK.call($BJ","$BK.call($KEY_15"),("&c=","&d="),("$KEY_14($KEY_15, [])","$KEY_14($KEY_15, [1])"),("$BI?.hash","$BI.hash"),(".catch(() => {",".catch(() => { sideEffect();")]{assert!(DIGEST.contains(from));assert_eq!(role_count(&wrap(&DIGEST.replace(from,to)),role),0,"accepted {from}");}
}
fn wheel_wrap(body:&str)->String {
    let captures=format!("let $KEY_289=99;{}{}{}let $KEY_327=$THREAD_2_103652;let $KEY_245=$KEY_327();let $KEY_120=$THREAD_0_107316;let $KEY_191=$THREAD_7_39465;let $KEY_16=0,$KEY_22=0,$KEY_1=0,$KEY_5={{}},$KEY_4=null,$KEY_18={{}};",include_str!("fixtures/event-timing-and-encoding/createKeystreamByteSource.js"),include_str!("fixtures/event-timing-and-encoding/writeNumber.js"),include_str!("fixtures/event-timing-and-encoding/createElapsedTimer.js"));
    format!("function main(){{{captures}{body}return 1;}}main();")
}
#[test]
fn wheel_integer_nonce_preserves_proved_writer_and_effect_order(){
    // Adapt the established complete family witness, preserving its own native
    // factory/writer origins. These are not evidence donated to seed functions.
    let body=include_str!("fixtures/event-timing-and-encoding/wheel_counter_frame.js");
    let integer=body.replace("let $BA = Math.random() * 4294967296;","let $BA = Math.random() * 4294967296 | 0;").replace("($BA | 0)","$BA");
    let role="emitWheelCounterFrame";
    assert_ne!(integer,body);assert_eq!(role_count(&wheel_wrap(&integer),role),1);
    assert_eq!(role_count(&wheel_wrap(&integer.replace("$BA","nonce").replace("$BB","bytes")),role),1);
    for(from,to)in[("4294967296 | 0","4294967296 | 1"),("$BA ^ 4256838423","$BA + 4256838423"),("$BD($BA, $KEY_1, $BB, $BC)","$BD($BA, $KEY_22, $BB, $BC)"),("$BB, 3)","$BB, 4)"),("$KEY_5.error(e)","$KEY_18.error(e)"),("removeEventListener(\"wheel\"","removeEventListener(\"click\"")]{assert!(integer.contains(from));assert_eq!(role_count(&wheel_wrap(&integer.replace(from,to)),role),0,"accepted {from}");}
    let reordered=integer.replace("let $BC = $KEY_191($BA, $KEY_22, $BB, 3);\n\t\t\t\t\t\tlet $BD = $KEY_191;\n\t\t\t\t\t\t++$KEY_1;\n\t\t\t\t\t\t$BD($BA, $KEY_1, $BB, $BC);","let $BC = $KEY_191; ++$KEY_1; $BC = $BC($BA, $KEY_1, $BB, 3); $KEY_191($BA, $KEY_22, $BB, $BC);");
    assert_ne!(reordered,integer);assert_eq!(role_count(&wheel_wrap(&reordered),role),1);
    assert_eq!(role_count(&wheel_wrap(&reordered.replace("$BC = $BC($BA, $KEY_1","$BC = $BC($BA, $KEY_22")),role),0);
}
