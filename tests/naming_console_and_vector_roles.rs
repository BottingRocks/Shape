use sha2::{Digest, Sha256};
use shape::naming::name_threads;

const INIT: &str = r#"function main(){function $THREAD_1_1(){console.error("no global");}return $THREAD_1_1;}main();"#;

const REFILL: &str = r#"function main(){function $THREAD_2_2($B){
let i=1;let x=$B[0]*63+$B[1];let n=8;
while(i<=n){x=Math.abs(Math.fround(Math.sin(x+i))*2);
if(x>=1){if(n<50){++n;}else{$B.push((n-i)/n);x=(n-i)/n;}}else{$B.push(x);}
++i;}
$B[1]=Number($B[1])+1;}
return $THREAD_2_2;}main();"#;

const VISIBILITY: &str = r#"function main(){let store={},flag=false,count=0,errors={};
function clock(){return 0;}
function $THREAD_3_3($E){try{++count;let s=store;let put=store.put;
put.call(s,{visible:$E.target.visibilityState==="visible",timestamp:clock()});
flag=true;}catch(e){errors.error(e);}}
return $THREAD_3_3;}main();"#;

fn expected_evaluators(name: &str) -> &'static [&'static str] {
    match name {
        "reportErrorToConsole" => &["events_console_error_statement"],
        "refillSinPrngBuffer" => &["scheduling_sine_prng_refill"],
        "recordPixelVectorSample" => &["graphics_bounded_pixel_vector_history"],
        "teardownSubmitInstrumentation" => &["browser_submit_instrumentation_teardown"],
        "emitSeededHashEncoding" => &["encoding_seeded_hash_encoding"],
        "emitSeededSignalFrame" => &["signals_seeded_signal_frame"],
        "recordVisibilityChangeSample" => &["events_visibility_change_sample"],
        _ => &[],
    }
}

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.function_roles.iter().any(|role| {
            role.candidates.iter().any(|candidate| candidate == name)
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| expected_evaluators(name).contains(&evaluator.as_str()))
        }) && r.report.outcomes.iter().any(|o| {
            o.binding.name == target && o.applied && o.proposed_name.as_deref() == Some(name)
        })
    })
}

#[test]
fn report_error_to_console_proves_exact_console_report() {
    for s in [
        INIT.to_string(),
        INIT.replace("$THREAD_1_1", "$THREAD_912_99881_2")
            .replace("no global", "runtime missing"),
    ] {
        let target = if s.contains("$THREAD_912") {
            "$THREAD_912_99881_2"
        } else {
            "$THREAD_1_1"
        };
        assert!(named(&s, target, "reportErrorToConsole"), "{s}");
    }
}

#[test]
fn report_error_to_console_refuses_changed_api_and_shape() {
    for (from, to) in [
        ("console.error", "console.warn"),
        ("console.error", "other.error"),
        ("console.error", "console?.error"),
        ("(\"no global\")", "(1)"),
        ("(\"no global\")", "(\"no global\", 1)"),
        (
            "console.error(\"no global\");",
            "return console.error(\"no global\");",
        ),
        (
            "console.error(\"no global\");",
            "console.error(\"no global\");return 1;",
        ),
        ("function $THREAD_1_1()", "function $THREAD_1_1(x)"),
        ("function $THREAD_1_1()", "async function $THREAD_1_1()"),
    ] {
        let s = INIT.replace(from, to);
        assert!(
            !named(&s, "$THREAD_1_1", "reportErrorToConsole"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn refill_sin_prng_proves_bounded_state_loop() {
    for s in [
        REFILL.to_string(),
        REFILL
            .replace("$THREAD_2_2", "$THREAD_71_887766_2")
            .replace("$B", "$K")
            .replace("let i", "let cursor")
            .replace("i<=n", "cursor<=n")
            .replace("x+i", "x+cursor")
            .replace("++i", "++cursor")
            .replace("(n-i)", "(n-cursor)"),
    ] {
        let target = if s.contains("$THREAD_71_887766_2") {
            "$THREAD_71_887766_2"
        } else {
            "$THREAD_2_2"
        };
        assert!(named(&s, target, "refillSinPrngBuffer"), "{s}");
    }
}

#[test]
fn refill_sin_prng_refuses_changed_operations() {
    for (from, to) in [
        ("*63", "*64"),
        ("*2", "*3"),
        ("Math.sin", "Math.cos"),
        ("Math.fround", "Math.floor"),
        ("Math.abs", "Math.trunc"),
        ("(x+i)", "(x-i)"),
        ("i<=n", "i<n"),
        ("x>=1", "x>1"),
        ("n<50", "n<=50"),
        ("(n-i)/n", "(i-n)/n"),
        ("Number($B[1])", "Number($B[0])"),
        ("Number($B[1])+1", "Number($B[1])+2"),
        ("$B[1]=Number", "$B[0]=Number"),
        ("let i=1", "let i=0"),
        ("let n=8", "let n=7"),
        ("$B.push(x)", "$B.push(i)"),
        ("++n", "--n"),
    ] {
        let s = REFILL.replace(from, to);
        assert!(
            !named(&s, "$THREAD_2_2", "refillSinPrngBuffer"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn visibility_sample_proves_owned_record_and_clock() {
    for s in [
        VISIBILITY.to_string(),
        VISIBILITY.replace("$THREAD_3_3", "$THREAD_44_1234_2")
            .replace("$E", "$K")
            .replace("store", "$KEY_7")
            .replace("flag", "$KEY_8")
            .replace("count", "$KEY_9")
            .replace("errors", "$KEY_10")
            .replace("clock", "$KEY_11"),
        VISIBILITY.replace(
            "let s=store;let put=store.put;\nput.call(s,{visible:$E.target.visibilityState===\"visible\",timestamp:clock()});",
            "store.put({visible:$E.target.visibilityState===\"visible\",timestamp:clock()});",
        ),
    ] {
        let target = if s.contains("$THREAD_44_1234_2") {
            "$THREAD_44_1234_2"
        } else {
            "$THREAD_3_3"
        };
        assert!(named(&s, target, "recordVisibilityChangeSample"), "{s}");
    }
}

#[test]
fn visibility_sample_refuses_changed_record_and_capture() {
    for (from, to) in [
        ("++count", "count++"),
        ("++count", "--count"),
        ("visible:", "shown:"),
        ("timestamp:", "time:"),
        ("visibilityState", "readyState"),
        ("===\"visible\"", "===\"hidden\""),
        ("clock()", "clock(1)"),
        ("flag=true", "flag=false"),
        ("store.put", "other.put"),
        ("errors.error(e)", "errors.log(e)"),
        ("errors.error(e)", "other.error(e)"),
        ("catch(e)", "catch(e){}"),
        ("put.call(s", "put.call(other"),
        ("let s=store", "let s=other"),
        ("let put=store.put", "let put=other.put"),
        ("function $THREAD_3_3($E)", "function $THREAD_3_3($E=0)"),
        ("try{++count", "try{let count=0;++count"),
    ] {
        let s = VISIBILITY.replace(from, to);
        assert!(
            !named(&s, "$THREAD_3_3", "recordVisibilityChangeSample"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn meaningful_names_and_collisions_are_preserved() {
    for s in [
        INIT.replace("$THREAD_1_1", "reportNoGlobal"),
        REFILL.replace("$THREAD_2_2", "refillBuffer"),
        VISIBILITY.replace("$THREAD_3_3", "onVisibility"),
    ] {
        assert_eq!(name_threads(&s, true).unwrap().source, s);
    }
    for (source, name) in [
        (INIT, "reportErrorToConsole"),
        (REFILL, "refillSinPrngBuffer"),
        (VISIBILITY, "recordVisibilityChangeSample"),
    ] {
        let collision = source.replace(
            "function main(){",
            &format!("function main(){{let {name}=17;useValue({name});"),
        );
        assert!(!named(&collision, name, name));
    }
}

const EMIT_FIXED: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_4_4($S){let seed=Math.random()*4294967296;
let nonce=[(seed|0)>>>24,(seed|0)>>16&255,(seed|0)>>8&255,(seed|0)&255];
seed=hashKey((seed|0)^1234567);
let h=mix(seed,$S[3],nonce,3);h=mix(seed,$S[4],nonce,h);
h=mix(seed,$S[2],nonce,h);h=mix(seed,$S[1],nonce,h);mix(seed,$S[0],nonce,h);
observer.next([[],nonce]);observer.complete();}
return $THREAD_4_4;}main();"#;

const EMIT_LOOP: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_5_5($D){let seed=Math.random()*4294967296;
let nonce=[(seed|0)>>>24,(seed|0)>>16&255,(seed|0)>>8&255,(seed|0)&255];
let hasher=hashKey((seed|0)^7654321);
let n=$D.length;let i=0;let h=mix(hasher,n,nonce,3);
for(;i<n;i++){h=mix(hasher,$D[i],nonce,h);}
observer.next([[],nonce]);observer.complete();}
return $THREAD_5_5;}main();"#;

#[test]
fn seeded_hash_encoding_proves_fixed_and_loop_variants() {
    for (source, target) in [
        (EMIT_FIXED.to_string(), "$THREAD_4_4"),
        (
            EMIT_FIXED
                .replace("$THREAD_4_4", "$THREAD_71_887766_2")
                .replace("$S", "$K")
                .replace("seed", "$KEY_5")
                .replace("nonce", "$KEY_6")
                .replace("hashKey", "hashOf")
                .replace("mix", "blend"),
            "$THREAD_71_887766_2",
        ),
        (EMIT_LOOP.to_string(), "$THREAD_5_5"),
        (
            EMIT_LOOP
                .replace("$THREAD_5_5", "$THREAD_812_99881_3")
                .replace("$D", "$K")
                .replace("seed", "$KEY_5")
                .replace("nonce", "$KEY_6")
                .replace("hasher", "$KEY_7")
                .replace("hashKey", "hashOf")
                .replace("mix", "blend"),
            "$THREAD_812_99881_3",
        ),
    ] {
        assert!(named(&source, target, "emitSeededHashEncoding"), "{source}");
    }
}

#[test]
fn seeded_hash_encoding_refuses_changed_operations() {
    for (from, to) in [
        ("Math.random()", "Math.min()"),
        ("*4294967296", "*4294967295"),
        ("(seed|0)>>>24", "(seed|0)>>>16"),
        ("(seed|0)>>16&255", "(seed|0)>>8&255"),
        ("^1234567", "+1234567"),
        (
            "let h=mix(seed,$S[3],nonce,3)",
            "let h=mix(seed,$S[3],nonce,4)",
        ),
        ("$S[4]", "$S[3]"),
        ("$S[2]", "$S[3]"),
        ("observer.next", "other.next"),
        ("observer.complete", "other.complete"),
        ("next([[],nonce])", "next([nonce,nonce])"),
        ("mix(seed,$S[0],nonce,h);", "mix(seed,$S[0],nonce,3);"),
        ("let nonce=", "let nonce=[],nonce="),
        ("$S[3]", "$S[6]"),
        ("function $THREAD_4_4($S)", "function $THREAD_4_4($S,$T)"),
    ] {
        let source = EMIT_FIXED.replace(from, to);
        assert!(
            !named(&source, "$THREAD_4_4", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
    for (from, to) in [
        ("$D.length", "$D.size"),
        ("i<n", "i<=n"),
        ("i++)", "i--)"),
        ("$D[i]", "$D[0]"),
        ("h=mix(hasher,$D[i],nonce,h)", "h=mix(hasher,$D[i],nonce,3)"),
        ("hasher=hashKey", "hasher=otherKey"),
        ("observer.complete()", "observer.complete(1)"),
        ("next([[],nonce])", "next([[],hasher])"),
    ] {
        let source = EMIT_LOOP.replace(from, to);
        assert!(
            !named(&source, "$THREAD_5_5", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

const TEARDOWN: &str = r#"function main(){let doc={head:null},flag=true,reg1={},reg2={},reg3={},clickHandler=()=>{},submitHandler=()=>{},styleNode={};
function $THREAD_6_6(){try{doc.removeEventListener("click",clickHandler);
if(!!flag){reg1.clear();reg2.clear();reg3.clear();
window.document.removeEventListener("submit",submitHandler);flag=false;}
if(doc.head){doc.head.removeChild(styleNode);}}catch(e){}}
return $THREAD_6_6;}main();"#;

#[test]
fn submit_teardown_proves_ordered_cleanup() {
    for (source, target) in [
        (TEARDOWN.to_string(), "$THREAD_6_6"),
        (TEARDOWN.replace("$THREAD_6_6", "$THREAD_71_887766_2")
            .replace("doc={", "$KEY_1={")
            .replace("doc.remove", "$KEY_1.remove")
            .replace("doc.head", "$KEY_1.head")
            .replace("flag", "$KEY_2")
            .replace("reg1", "$KEY_3")
            .replace("reg2", "$KEY_4")
            .replace("reg3", "$KEY_5")
            .replace("clickHandler", "$KEY_6")
            .replace("submitHandler", "$KEY_7")
            .replace("styleNode", "$KEY_8"),
         "$THREAD_71_887766_2"),
        (TEARDOWN.replace(
            "window.document.removeEventListener(\"submit\",submitHandler);flag=false;",
            "window.document.removeEventListener(\"submit\",submitHandler);window.navigation.removeEventListener(\"navigate\",submitHandler);flag=false;",
        ),
         "$THREAD_6_6"),
        (TEARDOWN.replace("reg3.clear();", "reg3.clear();reg4.clear();")
            .replace("reg3={},", "reg3={},reg4={},"),
         "$THREAD_6_6"),
    ] {
        assert!(named(&source, target, "teardownSubmitInstrumentation"), "{source}");
    }
}

#[test]
fn submit_teardown_refuses_changed_operations_and_order() {
    for (from, to) in [
        (
            "removeEventListener(\"click\"",
            "removeEventListener(\"mousedown\"",
        ),
        ("doc.removeEventListener", "other.removeEventListener"),
        (
            "removeEventListener(\"click\",clickHandler)",
            "removeEventListener(\"click\",otherHandler)",
        ),
        ("!!flag", "!flag"),
        ("reg1.clear", "other.clear"),
        ("reg1.clear()", "reg1.clear(1)"),
        ("reg1.clear()", "reg1.empty()"),
        ("reg2.clear()", "reg1.clear()"),
        (
            "window.document.removeEventListener",
            "window.document.addEventListener",
        ),
        (
            "removeEventListener(\"submit\",submitHandler)",
            "removeEventListener(\"submit\",otherHandler)",
        ),
        ("flag=false", "flag=true"),
        ("if(doc.head)", "if(other.head)"),
        ("doc.head.removeChild", "doc.head.appendChild"),
        (
            "doc.head.removeChild(styleNode)",
            "doc.head.removeChild(otherNode)",
        ),
        ("catch(e){}", "catch(e){flag=false;}"),
        ("function $THREAD_6_6()", "function $THREAD_6_6(extra)"),
    ] {
        let source = TEARDOWN.replace(from, to);
        assert!(
            !named(&source, "$THREAD_6_6", "teardownSubmitInstrumentation"),
            "accepted {from} -> {to}"
        );
    }
}

const PIXEL: &str = r#"function main(){let store={a:[],b:0};
function $THREAD_7_7($V,$R){let vec=$V;
if($R.a.some($E=>{let ok;
if(vec==null||$E==null||vec.length!==$E.length){ok=false;}
else{let j=0;for(;;){if(j>=vec.length){ok=true;break;}
if(vec[j]===$E[j]){++j;continue;}ok=false;break;}}
return ok;})){return void 0;}
$R.b=Number($R.b)+1;
if($R.a.length>5){return void 0;}
else{$R.a.push(Array.prototype.slice.call(vec));return void 0;}}
return $THREAD_7_7;}main();"#;

#[test]
fn pixel_vector_sample_proves_unique_history() {
    for (source, target) in [
        (PIXEL.to_string(), "$THREAD_7_7"),
        (
            PIXEL
                .replace("$THREAD_7_7", "$THREAD_71_887766_2")
                .replace("$V", "$K")
                .replace("$R", "$M")
                .replace("$E", "$N")
                .replace("vec", "$KEY_1")
                .replace("ok", "$KEY_2")
                .replace("j", "$KEY_3"),
            "$THREAD_71_887766_2",
        ),
        (
            PIXEL.replace(
                "$R.a.push(Array.prototype.slice.call(vec));",
                "let copy=Array.prototype.slice.call(vec);$R.a.push(copy);",
            ),
            "$THREAD_7_7",
        ),
    ] {
        assert!(
            named(&source, target, "recordPixelVectorSample"),
            "{source}"
        );
    }
}

#[test]
fn pixel_vector_sample_refuses_changed_comparison_and_limits() {
    for (from, to) in [
        ("vec==null", "vec===null"),
        ("$E==null", "$E===null"),
        ("vec.length!==$E.length", "vec.length===$E.length"),
        ("j>=vec.length", "j>vec.length"),
        ("vec[j]===$E[j]", "vec[j]==$E[j]"),
        ("ok=true", "ok=1"),
        ("++j", "--j"),
        ("ok=false", "ok=0"),
        ("$R.b=Number($R.b)+1", "$R.b=Number($R.b)+2"),
        ("$R.b=Number", "$R.a=Number"),
        ("$R.a.length>5", "$R.a.length>6"),
        ("$R.a.push", "$R.b.push"),
        (
            "Array.prototype.slice.call(vec)",
            "Array.prototype.slice(vec)",
        ),
        ("Array.prototype.slice", "Array.prototype.splice"),
        ("return void 0;}}", "return null;}}"),
        ("sat", "sat"),
    ] {
        if from == "sat" {
            continue;
        }
        let source = PIXEL.replace(from, to);
        assert!(
            !named(&source, "$THREAD_7_7", "recordPixelVectorSample"),
            "accepted {from} -> {to}"
        );
    }
}

// Payload-source variants use a descending fixed-field scan or an owned audio-channel
// Array.from array. Both retain the seed, header, mixer, and observer purpose while
// preparing the field source differently.

const EMIT_DESCENDING: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_8_8($S){let seed=Math.random()*4294967296;
let nonce=[(seed|0)>>>24,(seed|0)>>16&255,(seed|0)>>8&255,(seed|0)&255];
let out=[];
seed=hashKey((seed|0)^1234567);
let n=$S.length;
let h=mix(seed,n,nonce,3);
while(n-1>=0){h=mix(seed,$S[n-1],nonce,h);--n;}
observer.next([out,nonce]);observer.complete();}
return $THREAD_8_8;}main();"#;

const EMIT_DESCENDING_DECL: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_9_9($D){let out=[];
let seed=Math.random()*4294967296;
let nonce=[(seed|0)>>>24,(seed|0)>>16&255,(seed|0)>>8&255,(seed|0)&255];
let hasher=hashKey((seed|0)^7654321);
let n=$D.length;
let h=mix(hasher,n,nonce,3);
while(n-1>=0){h=mix(hasher,$D[n-1],nonce,h);--n;}
observer.next([out,nonce]);observer.complete();}
return $THREAD_9_9;}main();"#;

const EMIT_AUDIO: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
let channels=[0];
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_10_10($A){let samples=$A.getChannelData(0);
let seed=Math.random()*4294967296|0;
let nonce=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
let hasher=hashKey(seed^7654321);
let h=mix(hasher,channels[0],nonce,3);
samples=Array.from(samples);
let n=samples.length;let i=0;
h=mix(hasher,n,nonce,h);
for(;i<n;i++){h=mix(hasher,samples[i],nonce,h);}
observer.next([[],nonce]);observer.complete();}
return $THREAD_10_10;}main();"#;

#[test]
fn seeded_hash_encoding_base_variants_remain_nonvacuous() {
    // The BASE proofs must still fire before the payload-source extension is
    // accepted. The final assertion is deliberately non-vacuous: one changed
    // operator in the base body must stop the proof.
    for (source, target) in [(EMIT_FIXED, "$THREAD_4_4"), (EMIT_LOOP, "$THREAD_5_5")] {
        assert!(named(source, target, "emitSeededHashEncoding"), "{source}");
    }
    let broken = EMIT_FIXED.replace("^1234567", "+1234567");
    assert!(
        !named(&broken, "$THREAD_4_4", "emitSeededHashEncoding"),
        "base proof accepted a changed reseed operator"
    );
}

#[test]
fn seeded_hash_encoding_proves_descending_fixed_field_payload_sources() {
    for (source, target) in [
        (EMIT_DESCENDING.to_string(), "$THREAD_8_8"),
        (EMIT_DESCENDING_DECL.to_string(), "$THREAD_9_9"),
        (
            EMIT_DESCENDING
                .replace("$THREAD_8_8", "$THREAD_71_887766_2")
                .replace("$S", "$K")
                .replace("seed", "$KEY_5")
                .replace("nonce", "$KEY_6")
                .replace("hashKey", "hashOf")
                .replace("mix", "blend"),
            "$THREAD_71_887766_2",
        ),
        (
            EMIT_DESCENDING_DECL
                .replace("$THREAD_9_9", "$THREAD_812_99881_3")
                .replace("$D", "$K")
                .replace("seed", "$KEY_5")
                .replace("nonce", "$KEY_6")
                .replace("hasher", "$KEY_7")
                .replace("hashKey", "hashOf")
                .replace("mix", "blend"),
            "$THREAD_812_99881_3",
        ),
    ] {
        assert!(named(&source, target, "emitSeededHashEncoding"), "{source}");
    }
}

#[test]
fn seeded_hash_encoding_proves_owned_audio_channel_payload_sources() {
    for (source, target) in [
        (EMIT_AUDIO.to_string(), "$THREAD_10_10"),
        (
            EMIT_AUDIO
                .replace("$THREAD_10_10", "$THREAD_44_1234_2")
                .replace("$A", "$K")
                .replace("samples", "$KEY_1")
                .replace("seed", "$KEY_5")
                .replace("nonce", "$KEY_6")
                .replace("hasher", "$KEY_7")
                .replace("channels", "$KEY_8")
                .replace("hashKey", "hashOf")
                .replace("mix", "blend"),
            "$THREAD_44_1234_2",
        ),
    ] {
        assert!(named(&source, target, "emitSeededHashEncoding"), "{source}");
    }
}

#[test]
fn descending_payload_source_refuses_changed_seed_nonce_scan_and_payload() {
    for (from, to) in [
        ("let seed=Math.random()*4294967296;", "let seed=Date.now();"),
        (
            "let seed=Math.random()*4294967296;",
            "let seed=Math.random()*4294967295;",
        ),
        ("(seed|0)>>>24", "seed>>>24"),
        ("(seed|0)>>16&255", "(seed|0)>>15&255"),
        ("n-1>=0", "n>=0"),
        ("--n;", "n--;"),
        ("$S[n-1]", "$S[n]"),
        ("$S[n-1]", "$K[n-1]"),
        ("mix(seed,n,nonce,3)", "mix(seed,n,nonce,4)"),
        ("let h=mix(seed,n,nonce,3);", "let h=mix(other,n,nonce,3);"),
        ("seed=hashKey(", "other=hashKey("),
        ("let out=[];", "let out=[0];"),
        ("observer.next([out,nonce]);", "observer.next([nonce,out]);"),
        ("observer.next([out,nonce]);", "other.next([out,nonce]);"),
        ("observer.complete();", "observer.complete(1);"),
        (
            "while(n-1>=0){h=mix(seed,$S[n-1],nonce,h);--n;}",
            "while(n-1>=0){h=mix(seed,$S[n-1],nonce,h);--n;h=mix(seed,$S[0],nonce,h);}",
        ),
        ("let n=$S.length;", "let n=$S.length;let extra=$S.length;"),
    ] {
        let source = EMIT_DESCENDING.replace(from, to);
        assert!(
            !named(&source, "$THREAD_8_8", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn descending_payload_source_stays_disjoint_from_the_integer_seed_neighbor() {
    // The already-proved target-anchor digest shape uses a `| 0` seed and a bare
    // seed nonce. The strict descending proof must not silently absorb it: the
    // two anchor evaluators stay independent and the 004 alias agreement is
    // unchanged.
    let neighbor = EMIT_DESCENDING
        .replace(
            "let seed=Math.random()*4294967296;",
            "let seed=Math.random()*4294967296|0;",
        )
        .replace("(seed|0)>>>24", "seed>>>24")
        .replace("(seed|0)>>16&255", "seed>>16&255")
        .replace("(seed|0)>>8&255", "seed>>8&255")
        .replace("(seed|0)&255", "seed&255");
    assert!(
        !named(&neighbor, "$THREAD_8_8", "emitSeededHashEncoding"),
        "vector callback claimed the integer-seed neighbor"
    );
}

fn integer_vector_source() -> String {
    EMIT_DESCENDING_DECL
        .replace("Math.random()*4294967296;", "Math.random()*4294967296|0;")
        .replace("(seed|0)", "seed")
}

#[test]
fn integer_reverse_vector_signed_unsigned_words_and_randomized_locals_salts() {
    for random in [
        "Math.random()*4294967296|0",
        "(Math.random()*4294967296)>>>0",
    ] {
        for word in ["seed", "(seed|0)", "(seed>>>0)"] {
            for salt in ["0", "638053951", "2565407423", "1947099284", "4294967295"] {
                let source = integer_vector_source()
                    .replace("Math.random()*4294967296|0", random)
                    .replace(
                        "[seed>>>24,seed>>16&255,seed>>8&255,seed&255]",
                        &format!("[{word}>>>24,{word}>>16&255,{word}>>8&255,{word}&255]"),
                    )
                    .replace("hashKey(seed^7654321)", &format!("hashKey({word}^{salt})"))
                    .replace("$THREAD_9_9", "$THREAD_619_7002")
                    .replace("seed", "$KEY_71")
                    .replace("hasher", "$KEY_72")
                    .replace("nonce", "$KEY_73")
                    .replace("$D", "$K")
                    .replace("hashKey", "factory")
                    .replace("mix", "writeNumber");
                assert!(
                    named(&source, "$THREAD_619_7002", "emitSeededHashEncoding"),
                    "{source}"
                );
            }
        }
    }
    // Masking the middle bytes makes signed and unsigned shifts equivalent.
    let unsigned_bytes = integer_vector_source()
        .replace("seed>>16", "seed>>>16")
        .replace("seed>>8", "seed>>>8");
    assert!(named(
        &unsigned_bytes,
        "$THREAD_9_9",
        "emitSeededHashEncoding"
    ));
}

#[test]
fn integer_reverse_vector_rejects_skipped_repeated_or_foreign_indices_and_cursors() {
    for (from, to) in [
        ("$D[n-1]", "$D[n-2]"),
        ("$D[n-1]", "$D[0]"),
        ("$D[n-1]", "$D[n]"),
        ("$D[n-1]", "other[n-1]"),
        ("n-1>=0", "n-1>0"),
        ("n-1>=0", "n>=0"),
        ("--n;", "n=n-2;"),
        ("--n;", "--h;"),
        ("let n=$D.length;", "let n=$D.length-1;"),
        ("mix(hasher,n,nonce,3)", "mix(hasher,n,nonce,0)"),
        ("mix(hasher,n,nonce,3)", "mix(hasher,other.length,nonce,3)"),
        ("mix(hasher,$D[n-1],nonce,h)", "mix(hasher,$D[n-1],nonce,3)"),
        ("mix(hasher,$D[n-1],nonce,h)", "mix(hasher,$D[n-1],other,h)"),
        (
            "mix(hasher,$D[n-1],nonce,h)",
            "otherMix(hasher,$D[n-1],nonce,h)",
        ),
        ("--n;", "h=mix(hasher,$D[n-1],nonce,h);--n;"),
    ] {
        let source = integer_vector_source().replace(from, to);
        assert!(
            !named(&source, "$THREAD_9_9", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn integer_reverse_vector_rejects_different_word_ignored_seed_or_factory_and_shadowed_random() {
    for (from, to) in [
        ("seed>>>24", "other>>>24"),
        ("seed>>8&255", "other>>8&255"),
        ("seed>>>24", "seed>>24"),
        ("seed>>16&255", "seed>>15&255"),
        ("seed&255", "seed&127"),
        ("Math.random()*4294967296|0", "Math.random()*4294967295|0"),
        ("Math.random()*4294967296|0", "Math.random()*4294967296>>>1"),
        ("seed^7654321", "other^7654321"),
        ("seed^7654321", "7654321"),
        ("seed^7654321", "seed+7654321"),
        ("hashKey(seed^7654321)", "unknownFactory(seed^7654321)"),
        ("hashKey(seed^7654321)", "mix(seed^7654321)"),
        ("mix(hasher,n,nonce,3)", "mix(seed,n,nonce,3)"),
        ("mix(hasher,$D[n-1],nonce,h)", "mix(seed,$D[n-1],nonce,h)"),
        (
            "function main(){",
            "function main(){let Math={random(){return 0;}};",
        ),
    ] {
        let source = integer_vector_source().replace(from, to);
        assert!(
            !named(&source, "$THREAD_9_9", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
    // Different alias spellings of one callable root are still not independent.
    let source =
        integer_vector_source().replace("function mix(h,v,b,s){return s;}", "let mix=hashKey;");
    assert!(!named(&source, "$THREAD_9_9", "emitSeededHashEncoding"));
}

#[test]
fn integer_reverse_vector_rejects_dead_or_incomplete_emission_and_cross_function_donor() {
    for (from, to) in [
        (
            "observer.next([out,nonce]);",
            "if(false){observer.next([out,nonce]);}",
        ),
        (
            "observer.next([out,nonce]);",
            "return;observer.next([out,nonce]);",
        ),
        ("observer.next([out,nonce]);", "observer.next([out,other]);"),
        ("observer.next([out,nonce]);", "observer.next([nonce,out]);"),
        ("observer.complete();", ""),
        ("observer.complete();", "other.complete();"),
        ("observer.complete();", "observer.complete(0);"),
        ("let out=[];", "let out=[0];"),
        ("while(n-1>=0){h=mix(hasher,$D[n-1],nonce,h);--n;}", ""),
    ] {
        let source = integer_vector_source().replace(from, to);
        assert!(
            !named(&source, "$THREAD_9_9", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
    let donor = integer_vector_source().replace("$THREAD_9_9", "donor");
    let source = donor.replace(
        "return donor;",
        "function $THREAD_9_9($D){observer.next([[],[]]);observer.complete();}return $THREAD_9_9;",
    );
    assert!(!named(&source, "$THREAD_9_9", "emitSeededHashEncoding"));
}

#[test]
fn integer_reverse_vector_rejects_unreached_or_reassigned_factory_and_writer_aliases() {
    for (from, to) in [
        (
            "function hashKey(v){return v;}",
            "let hashKey;function factory(v){return v;}if(false){hashKey=factory;}",
        ),
        (
            "function hashKey(v){return v;}",
            "let hashKey=factory;function factory(v){return v;}hashKey=other;",
        ),
        (
            "function mix(h,v,b,s){return s;}",
            "let mix;function writer(h,v,b,s){return s;}if(false){mix=writer;}",
        ),
        (
            "function mix(h,v,b,s){return s;}",
            "let mix=writer;function writer(h,v,b,s){return s;}mix=other;",
        ),
    ] {
        let source = integer_vector_source().replace(from, to);
        assert!(
            !named(&source, "$THREAD_9_9", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn audio_channel_payload_source_refuses_changed_source_conversion_and_scan() {
    for (from, to) in [
        (
            "let samples=$A.getChannelData(0);",
            "let samples=$B.getChannelData(0);",
        ),
        ("getChannelData(0)", "getChannelData()"),
        ("getChannelData(0)", "getChannelDataTwo(0)"),
        (
            "samples=Array.from(samples);",
            "samples=Object.from(samples);",
        ),
        ("samples=Array.from(samples);", "other=Array.from(samples);"),
        ("samples=Array.from(samples);", "samples=Array.from(other);"),
        ("let n=samples.length;", "let n=$A.length;"),
        ("i<n", "i<=n"),
        ("i++)", "i--)"),
        ("samples[i]", "samples[0]"),
        ("samples[i]", "$A[i]"),
        (
            "mix(hasher,channels[0],nonce,3)",
            "mix(hasher,channels[0],nonce,4)",
        ),
        ("channels[0]", "channels"),
        ("channels[0]", "samples[0]"),
        ("mix(hasher,n,nonce,h)", "mix(other,n,nonce,h)"),
        (
            "h=mix(hasher,samples[i],nonce,h)",
            "h=mix(hasher,samples[i],nonce,3)",
        ),
        (
            "observer.next([[],nonce]);",
            "observer.next([samples,nonce]);",
        ),
        ("observer.next([[],nonce]);", "observer.next([[],hasher]);"),
        (
            "samples=Array.from(samples);",
            "samples=Array.from(samples);samples=$A.getChannelData(0);",
        ),
    ] {
        let source = EMIT_AUDIO.replace(from, to);
        assert!(
            !named(&source, "$THREAD_10_10", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

// Bounded spellings share one seeded-emitter owner: five formal-index fields with the
// cursor folded into a nested mix on the same hash/nonce/mixer chain; a length local
// declared before the seeded hash; or an audio channel whose captured numeric field moves
// to a trailing ignored mixer update. All retain the shared header, nonce, and observer
// tail.

const EMIT_FIXED_NESTED: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_11_11($S){let seed=Math.random()*4294967296;
let nonce=[(seed|0)>>>24,(seed|0)>>16&255,(seed|0)>>8&255,(seed|0)&255];
seed=hashKey((seed|0)^1234567);
let h=mix(seed,$S[0],nonce,3);
h=mix(seed,$S[1],nonce,mix(seed,$S[4],nonce,h));
h=mix(seed,$S[2],nonce,h);
mix(seed,$S[3],nonce,h);
observer.next([[],nonce]);observer.complete();}
return $THREAD_11_11;}main();"#;

const EMIT_LOOP_LENGTH_FIRST: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_12_12($D){let seed=Math.random()*4294967296|0;
let nonce=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
let n=$D.length;
let hasher=hashKey(seed^7654321);
let i=0;let h=mix(hasher,n,nonce,3);
for(;i<n;i++){h=mix(hasher,$D[i],nonce,h);}
observer.next([[],nonce]);observer.complete();}
return $THREAD_12_12;}main();"#;

const EMIT_AUDIO_TRAILER: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
let channels=[0];
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_13_13($A){let samples=$A.getChannelData(0);
let seed=Math.random()*4294967296|0;
let nonce=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
seed=hashKey(seed^7654321);
samples=Array.from(samples);
let n=samples.length;let i=0;
let h=mix(seed,n,nonce,3);
for(;i<n;i++){h=mix(seed,samples[i],nonce,h);}
mix(seed,channels[0],nonce,h);
observer.next([[],nonce]);observer.complete();}
return $THREAD_13_13;}main();"#;

#[test]
fn seeded_hash_encoding_proves_inner_mix_length_first_and_audio_trailer_sources() {
    for (source, target) in [
        (EMIT_FIXED_NESTED.to_string(), "$THREAD_11_11"),
        (
            EMIT_FIXED_NESTED
                .replace("$THREAD_11_11", "$THREAD_71_887766_2")
                .replace("$S", "$K")
                .replace("seed", "$KEY_5")
                .replace("nonce", "$KEY_6")
                .replace("hashKey", "hashOf")
                .replace("mix", "blend"),
            "$THREAD_71_887766_2",
        ),
        (EMIT_LOOP_LENGTH_FIRST.to_string(), "$THREAD_12_12"),
        (
            EMIT_LOOP_LENGTH_FIRST
                .replace("$THREAD_12_12", "$THREAD_812_99881_3")
                .replace("$D", "$K")
                .replace("seed", "$KEY_5")
                .replace("nonce", "$KEY_6")
                .replace("hasher", "$KEY_7")
                .replace("hashKey", "hashOf")
                .replace("mix", "blend"),
            "$THREAD_812_99881_3",
        ),
        (EMIT_AUDIO_TRAILER.to_string(), "$THREAD_13_13"),
        (
            EMIT_AUDIO_TRAILER
                .replace("$THREAD_13_13", "$THREAD_44_1234_2")
                .replace("$A", "$K")
                .replace("samples", "$KEY_1")
                .replace("seed", "$KEY_5")
                .replace("nonce", "$KEY_6")
                .replace("channels", "$KEY_8")
                .replace("hashKey", "hashOf")
                .replace("mix", "blend"),
            "$THREAD_44_1234_2",
        ),
    ] {
        assert!(named(&source, target, "emitSeededHashEncoding"), "{source}");
    }
}

#[test]
fn inner_mix_payload_source_refuses_foreign_chain_and_incomplete_indices() {
    for (from, to) in [
        (
            "h=mix(seed,$S[1],nonce,mix(seed,$S[4],nonce,h));",
            "h=mix(seed,$S[1],nonce,other(seed,$S[4],nonce,h));",
        ),
        (
            "h=mix(seed,$S[1],nonce,mix(seed,$S[4],nonce,h));",
            "h=mix(seed,$S[1],nonce,mix(anchor,$S[4],nonce,h));",
        ),
        (
            "h=mix(seed,$S[1],nonce,mix(seed,$S[4],nonce,h));",
            "h=mix(seed,$S[1],nonce,mix(seed,$S[4],anchor,h));",
        ),
        (
            "h=mix(seed,$S[1],nonce,mix(seed,$S[4],nonce,h));",
            "h=mix(seed,$S[1],nonce,mix(seed,$S[6],nonce,h));",
        ),
        (
            "h=mix(seed,$S[1],nonce,mix(seed,$S[4],nonce,h));",
            "h=mix(seed,$S[1],nonce,mix(seed,$S[4],nonce,anchor));",
        ),
        (
            "h=mix(seed,$S[1],nonce,mix(seed,$S[4],nonce,h));",
            "h=mix(seed,$S[0],nonce,mix(seed,$S[4],nonce,h));",
        ),
        ("mix(seed,$S[3],nonce,h);", "mix(seed,$S[1],nonce,h);"),
    ] {
        let source = EMIT_FIXED_NESTED.replace(from, to);
        assert!(
            !named(&source, "$THREAD_11_11", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn loop_length_first_refuses_wrong_source_and_order_dependency() {
    for (from, to) in [
        ("let n=$D.length;", "let n=$D.size;"),
        ("let n=$D.length;", ""),
        ("hashKey(seed^7654321)", "hashKey(seed+7654321)"),
        ("let hasher=hashKey(", "let hasher=otherKey("),
        ("seed>>>24", "seed>>>16"),
        ("let h=mix(hasher,n,nonce,3)", "let h=mix(hasher,i,nonce,3)"),
        ("i<n", "i<=n"),
        ("i++)", "i--)"),
        ("$D[i]", "$D[0]"),
        ("h=mix(hasher,$D[i],nonce,h)", "h=mix(hasher,$D[i],nonce,3)"),
        ("observer.next", "other.next"),
        (
            "observer.next([[],nonce]);observer.complete();",
            "observer.next([[],nonce]);",
        ),
    ] {
        let source = EMIT_LOOP_LENGTH_FIRST.replace(from, to);
        assert!(
            !named(&source, "$THREAD_12_12", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn audio_trailer_payload_source_refuses_wrong_trailer_hash_nonce_cursor_and_observer() {
    for (from, to) in [
        (
            "mix(seed,channels[0],nonce,h);",
            "other(seed,channels[0],nonce,h);",
        ),
        (
            "mix(seed,channels[0],nonce,h);",
            "mix(anchor,channels[0],nonce,h);",
        ),
        (
            "mix(seed,channels[0],nonce,h);",
            "mix(seed,channels[0],anchor,h);",
        ),
        (
            "mix(seed,channels[0],nonce,h);",
            "mix(seed,samples[0],nonce,h);",
        ),
        (
            "mix(seed,channels[0],nonce,h);",
            "mix(seed,channels[0],nonce,3);",
        ),
        ("channels[0]", "channels"),
        ("let n=samples.length;", "let n=$A.length;"),
        ("samples=Array.from(samples);", "other=Array.from(samples);"),
        ("observer.next", "other.next"),
        ("observer.complete", "other.complete"),
        (
            "observer.next([[],nonce]);observer.complete();",
            "observer.next([[],nonce]);",
        ),
    ] {
        let source = EMIT_AUDIO_TRAILER.replace(from, to);
        assert!(
            !named(&source, "$THREAD_13_13", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

// Bounded variants cover compact two-update fixed fields and reverse-while audio payloads.
// The audio payload has prefix-field and trailer-field spellings; both retain the same
// seeded-emitter owner.

const EMIT_FIXED_COMPACT: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_14_14($S){let seed=Math.random()*4294967296;
let nonce=[(seed|0)>>>24,(seed|0)>>16&255,(seed|0)>>8&255,(seed|0)&255];
seed=hashKey((seed|0)^2632759746);
let h=mix(seed,$S[4],nonce,mix(seed,$S[3],nonce,3));
h=mix(seed,$S[0],nonce,h);
mix(seed,$S[1],nonce,mix(seed,$S[2],nonce,h));
observer.next([[],nonce]);observer.complete();}
return $THREAD_14_14;}main();"#;

const EMIT_AUDIO_REVERSE: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
let channels=[0];
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_15_15($A){let samples=$A.getChannelData(0);
let seed=Math.random()*4294967296|0;
let nonce=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
let out=[];let hasher=hashKey(seed^3179946036);
let h=mix(hasher,channels[0],nonce,3);
samples=Array.from(samples);
let n=samples.length;
h=mix(hasher,n,nonce,h);
while(n-1>=0){h=mix(hasher,samples[n-1],nonce,h);--n;}
observer.next([out,nonce]);observer.complete();}
return $THREAD_15_15;}main();"#;

const EMIT_AUDIO_REVERSE_TRAILER: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
let channels=[0];
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_16_16($A){let samples=$A.getChannelData(0);
let seed=Math.random()*4294967296;
let nonce=[(seed|0)>>>24,(seed|0)>>16&255,(seed|0)>>8&255,(seed|0)&255];
let out=[];seed=hashKey((seed|0)^179115447);
samples=Array.from(samples);
let n=samples.length;let h=mix(seed,n,nonce,3);
while(n-1>=0){h=mix(seed,samples[n-1],nonce,h);--n;}
mix(seed,channels[0],nonce,h);
observer.next([out,nonce]);observer.complete();}
return $THREAD_16_16;}main();"#;

#[test]
fn seeded_hash_encoding_proves_compact_and_reverse_while_variants() {
    for (source, target) in [
        (EMIT_FIXED_COMPACT.to_string(), "$THREAD_14_14"),
        (
            EMIT_FIXED_COMPACT
                .replace("$THREAD_14_14", "$THREAD_71_887766_2")
                .replace("$S", "$K")
                .replace("seed", "$KEY_5")
                .replace("nonce", "$KEY_6")
                .replace("hashKey", "hashOf")
                .replace("mix", "blend"),
            "$THREAD_71_887766_2",
        ),
        (EMIT_AUDIO_REVERSE.to_string(), "$THREAD_15_15"),
        (
            EMIT_AUDIO_REVERSE
                .replace("$THREAD_15_15", "$THREAD_44_1234_2")
                .replace("$A", "$K")
                .replace("samples", "$KEY_1")
                .replace("seed", "$KEY_5")
                .replace("nonce", "$KEY_6")
                .replace("hasher", "$KEY_7")
                .replace("channels", "$KEY_8")
                .replace("hashKey", "hashOf")
                .replace("mix", "blend"),
            "$THREAD_44_1234_2",
        ),
        (EMIT_AUDIO_REVERSE_TRAILER.to_string(), "$THREAD_16_16"),
    ] {
        assert!(named(&source, target, "emitSeededHashEncoding"), "{source}");
    }
    assert!(named(
        &EMIT_AUDIO_REVERSE_TRAILER
            .replace("$THREAD_16_16", "$THREAD_812_99881_3")
            .replace("$A", "$K")
            .replace("samples", "$KEY_1")
            .replace("seed", "$KEY_5")
            .replace("nonce", "$KEY_6")
            .replace("channels", "$KEY_8")
            .replace("hashKey", "hashOf")
            .replace("mix", "blend"),
        "$THREAD_812_99881_3",
        "emitSeededHashEncoding"
    ));
}

#[test]
fn compact_inner_mix_refuses_wrong_chain_order_and_field_coverage() {
    for (from, to) in [
        ("mix(seed,$S[3],nonce,3)", "other(seed,$S[3],nonce,3)"),
        ("mix(seed,$S[3],nonce,3)", "mix(anchor,$S[3],nonce,3)"),
        ("mix(seed,$S[3],nonce,3)", "mix(seed,$S[3],other,3)"),
        ("$S[4]", "$S[3]"),
        ("$S[0]", "$S[1]"),
        ("$S[2]", "$S[0]"),
        ("mix(seed,$S[2],nonce,h)", "mix(seed,$S[2],nonce,3)"),
        (
            "mix(seed,$S[1],nonce,mix(seed,$S[2],nonce,h))",
            "mix(seed,$S[1],nonce,mix(seed,$S[2],nonce,anchor))",
        ),
        (
            "mix(seed,$S[1],nonce,mix(seed,$S[2],nonce,h));",
            "mix(seed,$S[1],nonce,mix(seed,$S[2],nonce,h));mix(seed,$S[0],nonce,h);",
        ),
        ("observer.next", "other.next"),
        (
            "observer.next([[],nonce]);observer.complete();",
            "observer.next([[],nonce]);observer.complete(1);",
        ),
    ] {
        let original = EMIT_FIXED_COMPACT;
        assert!(
            original.contains(from),
            "fixture is missing the negative anchor: {from}"
        );
        let source = original.replace(from, to);
        assert_ne!(
            source.as_str(),
            original,
            "negative replacement is a no-op: {from} -> {to}"
        );
        assert!(
            !named(&source, "$THREAD_14_14", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn reverse_while_audio_refuses_wrong_payload_cursor_and_effects() {
    for (from, to) in [
        ("getChannelData(0)", "getChannelData(1)"),
        ("let out=[];", "let out=[0];"),
        ("channels[0]", "samples[0]"),
        ("channels[0]", "other[0]"),
        ("samples=Array.from(samples);", "samples=Object.from(samples);"),
        ("samples=Array.from(samples);", "other=Array.from(samples);"),
        ("let n=samples.length;", "let n=$A.length;"),
        ("let hasher=hashKey", "let hasher=otherKey"),
        ("seed>>>24", "seed>>>16"),
        ("h=mix(hasher,n,nonce,h)", "h=mix(other,n,nonce,h)"),
        ("n-1>=0", "n>=0"),
        ("--n;", "n--;"),
        ("samples[n-1]", "samples[n]"),
        ("samples[n-1]", "$A[n-1]"),
        ("observer.next", "other.next"),
        ("observer.next([out,nonce]);", "observer.next([[],nonce]);"),
        ("observer.next([out,nonce]);observer.complete();", "observer.next([out,nonce]);observer.complete(1);"),
        ("while(n-1>=0){h=mix(hasher,samples[n-1],nonce,h);--n;}", "while(n-1>=0){h=mix(hasher,samples[n-1],nonce,h);--n;h=mix(hasher,samples[0],nonce,h);}"),
    ] {
        let original = EMIT_AUDIO_REVERSE;
        assert!(
            original.contains(from),
            "fixture is missing the negative anchor: {from}"
        );
        let source = original.replace(from, to);
        assert_ne!(
            source.as_str(),
            original,
            "negative replacement is a no-op: {from} -> {to}"
        );
        assert!(
            !named(&source, "$THREAD_15_15", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn reverse_while_audio_trailer_refuses_wrong_start_trailer_and_observer() {
    for (from, to) in [
        (
            "let seed=Math.random()*4294967296;",
            "let seed=Math.random()*4294967296|1;",
        ),
        (
            "let h=mix(seed,n,nonce,3);",
            "let h=mix(seed,channels[0],nonce,3);",
        ),
        ("seed=hashKey", "seed=otherKey"),
        ("(seed|0)>>>24", "(seed|0)>>>16"),
        (
            "mix(seed,channels[0],nonce,h);",
            "other(seed,channels[0],nonce,h);",
        ),
        (
            "mix(seed,channels[0],nonce,h);",
            "mix(anchor,channels[0],nonce,h);",
        ),
        (
            "mix(seed,channels[0],nonce,h);",
            "mix(seed,channels[0],other,h);",
        ),
        (
            "mix(seed,channels[0],nonce,h);",
            "mix(seed,channels[0],nonce,3);",
        ),
        ("n-1>=0", "n>=0"),
        ("--n;", "n++;"),
        ("samples[n-1]", "samples[n]"),
        ("observer.next([out,nonce]);", "other.next([out,nonce]);"),
        ("let out=[];", "let out=[0];"),
        (
            "mix(seed,channels[0],nonce,h);\nobserver.next",
            "mix(seed,channels[0],nonce,h);mix(seed,channels[0],nonce,h);\nobserver.next",
        ),
    ] {
        let original = EMIT_AUDIO_REVERSE_TRAILER;
        assert!(
            original.contains(from),
            "fixture is missing the negative anchor: {from}"
        );
        let source = original.replace(from, to);
        assert_ne!(
            source.as_str(),
            original,
            "negative replacement is a no-op: {from} -> {to}"
        );
        assert!(
            !named(&source, "$THREAD_16_16", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

// Exact fixed-field and audio witness bodies. Each is embedded byte-for-byte in a unique
// main that declares only the captured helpers referenced by the body.

const NESTED_MIX_FIXED_INNER_MIX: &str = r#"function $THREAD_5_49632($DD) {
									let $DE = Math.random() * 4294967296;
									let $KEY_4 = [($DE | 0) >>> 24, ($DE | 0) >> 16 & 255, ($DE | 0) >> 8 & 255, ($DE | 0) & 255];
									$DE = $KEY_312(($DE | 0) ^ 1103297445);
									let $KEY_1 = $KEY_17($DE, $DD[0], $KEY_4, 3);
									$KEY_1 = $KEY_17($DE, $DD[1], $KEY_4, $KEY_17($DE, $DD[4], $KEY_4, $KEY_1));
									$KEY_1 = $KEY_17($DE, $DD[2], $KEY_4, $KEY_1);
									$KEY_17($DE, $DD[3], $KEY_4, $KEY_1);
									$KEY_0.next([[], $KEY_4]);
									$KEY_0.complete();
								}"#;
const NESTED_MIX_FIXED_INNER_MIX_SHA256: &str =
    "f87b520ab44e99abd682add05cff349868968415faec727f681f48fcbc1176ea";

const NESTED_MIX_LOOP_LENGTH_FIRST: &str = r#"function $THREAD_2_134929($BY) {
								let $KEY_3 = Math.random() * 4294967296 | 0;
								let $BZ = [$KEY_3 >>> 24, $KEY_3 >> 16 & 255, $KEY_3 >> 8 & 255, $KEY_3 & 255];
								let $CA = $BY.length;
								let $KEY_6 = $KEY_312($KEY_3 ^ 1948115183);
								let i = 0;
								let $CB = $KEY_17($KEY_6, $CA, $BZ, 3);
								for (; i < $CA; i++) {
									$CB = $KEY_17($KEY_6, $BY[i], $BZ, $CB);
								}
								$KEY_11.next([[], $BZ]);
								$KEY_11.complete();
							}"#;
const NESTED_MIX_LOOP_LENGTH_FIRST_SHA256: &str =
    "57b797db498a4753719ac32be1731d73034780041cbde6a1aa5b63fc60a7dcfb";

const NESTED_MIX_AUDIO_TRAILER: &str = r#"function $THREAD_8_149810($DY) {
									let $KEY_2 = $DY.getChannelData(0);
									let $KEY_4 = Math.random() * 4294967296 | 0;
									let $DZ = [$KEY_4 >>> 24, $KEY_4 >> 16 & 255, $KEY_4 >> 8 & 255, $KEY_4 & 255];
									$KEY_4 = $KEY_312($KEY_4 ^ 1144020272);
									$KEY_2 = Array.from($KEY_2);
									let $KEY_5 = $KEY_2.length;
									let i = 0;
									let $EA = $KEY_17($KEY_4, $KEY_5, $DZ, 3);
									for (; i < $KEY_5; i++) {
										$EA = $KEY_17($KEY_4, $KEY_2[i], $DZ, $EA);
									}
									$KEY_17($KEY_4, $KEY_64[0], $DZ, $EA);
									$KEY_108.next([[], $DZ]);
									$KEY_108.complete();
								}"#;
const NESTED_MIX_AUDIO_TRAILER_SHA256: &str =
    "8a962b3d5319226ce67ff89a39fc31bcb805a3083d5f7cdfd47cd2dd310e6653";

// Exact compact-update and reverse-audio bodies. Byte lengths and local hashes pin the
// source evidence independently of recognition.
const COMPACT_UPDATE_FIXED_COMPACT: &str = r#"function $THREAD_7_159840($CD) {
								let $CE = Math.random() * 4294967296;
								let $CF = [($CE | 0) >>> 24, ($CE | 0) >> 16 & 255, ($CE | 0) >> 8 & 255, ($CE | 0) & 255];
								let $KEY_2 = $KEY_282(($CE | 0) ^ 2632759746);
								let $CG = $KEY_323($KEY_2, $CD[4], $CF, $KEY_323($KEY_2, $CD[3], $CF, 3));
								$CG = $KEY_323($KEY_2, $CD[0], $CF, $CG);
								$KEY_323($KEY_2, $CD[1], $CF, $KEY_323($KEY_2, $CD[2], $CF, $CG));
								$KEY_5.next([[], $CF]);
								$KEY_5.complete();
							}"#;
const COMPACT_UPDATE_FIXED_COMPACT_SHA256: &str =
    "1632416594f2173604045c41292ead7c3300ec0965147aa1182f65bfe0861432";

const COMPACT_UPDATE_AUDIO_REVERSE: &str = r#"function $THREAD_6_166516($CC) {
									let $KEY_1 = $CC.getChannelData(0);
									let $KEY_5 = Math.random() * 4294967296 | 0;
									let $CD = [$KEY_5 >>> 24, $KEY_5 >> 16 & 255, $KEY_5 >> 8 & 255, $KEY_5 & 255];
									let $CE = [];
									let $KEY_2 = $KEY_282($KEY_5 ^ 3179946036);
									let $CF = $KEY_323($KEY_2, $KEY_32[0], $CD, 3);
									$KEY_1 = Array.from($KEY_1);
									let $CG = $KEY_1.length;
									$CF = $KEY_323($KEY_2, $CG, $CD, $CF);
									while ($CG - 1 >= 0) {
										$CF = $KEY_323($KEY_2, $KEY_1[$CG - 1], $CD, $CF);
										--$CG;
									}
									$KEY_16.next([$CE, $CD]);
									$KEY_16.complete();
								}"#;
const COMPACT_UPDATE_AUDIO_REVERSE_SHA256: &str =
    "854f8a551bd317fb37c67222233e0b348c6fbfbd9a3c34ef5d16074b21ca0aec";

const REVERSE_TRAILER_AUDIO_REVERSE_TRAILER: &str = r#"function $THREAD_7_1668($DB) {
							let $KEY_3 = $DB.getChannelData(0);
							let $DC = Math.random() * 4294967296;
							let $DD = [($DC | 0) >>> 24, ($DC | 0) >> 16 & 255, ($DC | 0) >> 8 & 255, ($DC | 0) & 255];
							let $DE = [];
							$DC = $KEY_310(($DC | 0) ^ 179115447);
							$KEY_3 = Array.from($KEY_3);
							let $DF = $KEY_3.length;
							let $DG = $KEY_207($DC, $DF, $DD, 3);
							while ($DF - 1 >= 0) {
								$DG = $KEY_207($DC, $KEY_3[$DF - 1], $DD, $DG);
								--$DF;
							}
							$KEY_207($DC, $KEY_29[0], $DD, $DG);
							$KEY_4.next([$DE, $DD]);
							$KEY_4.complete();
						}"#;
const REVERSE_TRAILER_AUDIO_REVERSE_TRAILER_SHA256: &str =
    "5bf34780c539cccd17af33cac778c2c2140c355d0d2282610e315d7e5d06ffba";

fn compact_update_carrier(body: &str, target: &str) -> String {
    format!(
        "function main(){{let $KEY_5={{next(){{}},complete(){{}}}},$KEY_16={{next(){{}},complete(){{}}}},$KEY_32=[0];\nfunction $KEY_282(v){{return v;}}function $KEY_323(h,v,b,s){{return s;}}\n{body}\nreturn {target};}}main();"
    )
}

fn reverse_trailer_carrier(body: &str, target: &str) -> String {
    format!(
        "function main(){{let $KEY_4={{next(){{}},complete(){{}}}},$KEY_29=[0];\nfunction $KEY_310(v){{return v;}}function $KEY_207(h,v,b,s){{return s;}}\n{body}\nreturn {target};}}main();"
    )
}

#[test]
fn compact_and_reverse_real_body_slices_are_exact_and_named() {
    for (slice, length, digest) in [
        (COMPACT_UPDATE_FIXED_COMPACT, 509, COMPACT_UPDATE_FIXED_COMPACT_SHA256),
        (COMPACT_UPDATE_AUDIO_REVERSE, 668, COMPACT_UPDATE_AUDIO_REVERSE_SHA256),
        (
            REVERSE_TRAILER_AUDIO_REVERSE_TRAILER,
            617,
            REVERSE_TRAILER_AUDIO_REVERSE_TRAILER_SHA256,
        ),
    ] {
        assert_eq!(slice.len(), length);
        assert_eq!(sha256_hex(slice.as_bytes()), digest);
    }
    for (slice, target, source) in [
        (
            COMPACT_UPDATE_FIXED_COMPACT,
            "$THREAD_7_159840",
            compact_update_carrier as fn(&str, &str) -> String,
        ),
        (
            COMPACT_UPDATE_AUDIO_REVERSE,
            "$THREAD_6_166516",
            compact_update_carrier as fn(&str, &str) -> String,
        ),
        (
            REVERSE_TRAILER_AUDIO_REVERSE_TRAILER,
            "$THREAD_7_1668",
            reverse_trailer_carrier as fn(&str, &str) -> String,
        ),
    ] {
        assert!(
            named(&source(slice, target), target, "emitSeededHashEncoding"),
            "{slice}"
        );
    }
}

fn nested_mix_carrier(body: &str, target: &str) -> String {
    format!(
        "function main(){{let $KEY_0={{next(){{}},complete(){{}}}},$KEY_11={{next(){{}},complete(){{}}}},$KEY_108={{next(){{}},complete(){{}}}},$KEY_64=[0];\nfunction $KEY_312(v){{return v;}}function $KEY_17(h,v,b,s){{return s;}}\n{body}\nreturn {target};}}main();"
    )
}

#[test]
fn nested_mix_witnesses_are_the_exact_real_slices() {
    for (slice, length, digest) in [
        (NESTED_MIX_FIXED_INNER_MIX, 549, NESTED_MIX_FIXED_INNER_MIX_SHA256),
        (
            NESTED_MIX_LOOP_LENGTH_FIRST,
            483,
            NESTED_MIX_LOOP_LENGTH_FIRST_SHA256,
        ),
        (NESTED_MIX_AUDIO_TRAILER, 639, NESTED_MIX_AUDIO_TRAILER_SHA256),
    ] {
        assert_eq!(slice.len(), length);
        assert_eq!(sha256_hex(slice.as_bytes()), digest);
    }
}

#[test]
fn nested_mix_real_slices_are_named() {
    for (slice, target) in [
        (NESTED_MIX_FIXED_INNER_MIX, "$THREAD_5_49632"),
        (NESTED_MIX_LOOP_LENGTH_FIRST, "$THREAD_2_134929"),
        (NESTED_MIX_AUDIO_TRAILER, "$THREAD_8_149810"),
    ] {
        let source = nested_mix_carrier(slice, target);
        assert!(named(&source, target, "emitSeededHashEncoding"), "{source}");
    }
}

// Integer reverse-vector witnesses: exact cleaned-source slices. Helper bodies
// are real independent factory/writer functions, not inferred from their labels.
const VECTOR_FACTORY_0: &str = r#"function $THREAD_0_108369($AN) {
			let $KEY_1, $KEY_2, $KEY_4, $KEY_5;
			function $THREAD_1_15934() {
				let $AO = $KEY_1;
				$KEY_1 = ($AO + 1) % 4;
				if (($AO + 1) % 4 === 0) {
					$KEY_2 = $KEY_4 + $KEY_5;
					let $AP = $KEY_5;
					$KEY_5 = $AP ^ $KEY_4;
					$KEY_4 = $AP ^ $KEY_4 ^ ($KEY_4 << 26 | $KEY_4 >>> 6) ^ $KEY_5 << 9;
					$KEY_5 = $KEY_5 << 13 | $KEY_5 >>> 19;
					return $KEY_2 & 255;
				}
				if (($AO + 1) % 4 === 1) {
					return $KEY_2 >> 8 & 255;
				}
				if (($AO + 1) % 4 === 2) {
					return $KEY_2 >> 16 & 255;
				}
				if (($AO + 1) % 4 === 3) {
					return $KEY_2 >>> 24 & 255;
				} else {
					return void 0;
				}
			}
			$KEY_4 = $KEY_87;
			$KEY_5 = $AN;
			$KEY_1 = -1;
			$THREAD_1_15934();
			$THREAD_1_15934();
			$THREAD_1_15934();
			$THREAD_1_15934();
			return $THREAD_1_15934;
		}"#;
const VECTOR_WRITER_0: &str = r#"function $THREAD_0_128689($AN, $AO, $AP, $AQ) {
			let $KEY_2;
			if (($AO | 0) === $AO) {
				let $AR;
				if ($AO < 0) {
					$AR = $AO < 0;
				} else if ($AO === 0) {
					$AR = 1 / $AO < 0;
				} else {
					$AR = $AO === 0;
				}
				let $AS, $AT;
				if ($AR) {
					$AS = -$AO;
					$AT = 64;
				} else {
					$AS = $AO;
					$AT = 0;
				}
				if ($AS < 32) {
					$KEY_2 = Number($AQ) + 1;
					$AP[$KEY_2] = ($AT | $AS) ^ $AN();
				} else if ($AS < 4096) {
					$KEY_2 = Number($AQ) + 1;
					$AP[$KEY_2] = ($AT | ($AS & 31 | 32)) ^ $AN();
					$AP[++$KEY_2] = $AS >> 5 ^ $AN();
				} else if ($AS < 524288) {
					$KEY_2 = Number($AQ) + 1;
					$AP[$KEY_2] = ($AT | ($AS & 31 | 32)) ^ $AN();
					$AP[++$KEY_2] = ($AS >> 5 & 127 | 128) ^ $AN();
					$AP[++$KEY_2] = $AS >>> 12 ^ $AN();
				} else if ($AS < 67108864) {
					$KEY_2 = Number($AQ) + 1;
					$AP[$KEY_2] = ($AT | ($AS & 31 | 32)) ^ $AN();
					$AP[++$KEY_2] = ($AS >> 5 & 127 | 128) ^ $AN();
					$AP[++$KEY_2] = ($AS >>> 12 & 127 | 128) ^ $AN();
					$AP[++$KEY_2] = $AS >>> 19 ^ $AN();
				} else {
					$KEY_2 = Number($AQ) + 1;
					$AP[$KEY_2] = ($AT | ($AS & 31 | 32)) ^ $AN();
					$AP[++$KEY_2] = ($AS >> 5 & 127 | 128) ^ $AN();
					$AP[++$KEY_2] = ($AS >>> 12 & 127 | 128) ^ $AN();
					$AP[++$KEY_2] = ($AS >>> 19 & 127 | 128) ^ $AN();
					$AP[++$KEY_2] = $AS >>> 26 ^ $AN();
				}
			} else {
				$KEY_2 = $AQ;
				$KEY_2 = Number($AQ) + 1;
				$AP[$KEY_2] = $AN() ^ 128;
				let $AU;
				if ($KEY_290) {
					$KEY_290[0] = $AO;
					$AU = new $KEY_391($KEY_290.buffer);
				} else if ($AO !== $AO) {
					$AU = $KEY_343;
				} else {
					let $KEY_9 = $AO < 0 || $AO === 0 && 1 / $AO < 0;
					let $AV = Math.abs($AO);
					let $AW, $KEY_7;
					if ($AV >= Math.pow(2, -1022)) {
						let $KEY_1 = Math.min(Math.floor(Math.log($AV) / Math.LN2), 1023);
						$AV = $AV / Math.pow(2, $KEY_1);
						let $AX = $AV < 1;
						$AV = $AX ? $AV * 2 : $AV;
						$AX = $AX ? $KEY_1 - 1 : $KEY_1;
						if ($AV >= 2) {
							++$AX;
							$AV = $AV / 2;
						}
						$KEY_7 = Math.pow(2, 52);
						let $AY = Math.floor($AV * $KEY_7);
						$AV = $AV * $KEY_7;
						$AX += 1023;
						let $AZ = (($AV - $AY < .5 ? $AY : $AV - $AY > .5 ? $AY + 1 : $AY % 2 ? $AY + 1 : $AY) - $KEY_7) / $KEY_7 >= 1;
						$AW = $AZ ? $AX + 1 : $AX;
						if ($AW > 2046) {
							$AW = 2047;
							$KEY_7 = 0;
						} else {
							$KEY_7 = $AZ ? 0 : ($AV - $AY < .5 ? $AY : $AV - $AY > .5 ? $AY + 1 : $AY % 2 ? $AY + 1 : $AY) - $KEY_7;
						}
					} else {
						$AV = $AV / Math.pow(2, -1074);
						let $BA = Math.floor($AV);
						$AW = 0;
						$KEY_7 = $AV - $BA < .5 ? $BA : $AV - $BA > .5 ? $BA + 1 : $BA % 2 ? $BA + 1 : $BA;
					}
					let $BB = [];
					let k = 51;
					for (; k >= 0; k--) {
						$BB.unshift(!!($KEY_7 & 1));
						$KEY_7 = Math.floor($KEY_7 / 2);
					}
					let i = 11;
					for (; i > 0; i--) {
						$BB.unshift(!!($AW & 1));
						$AW = Math.floor($AW / 2);
					}
					$BB.unshift($KEY_9);
					$AU = [];
					let j = 56;
					for (; j >= 0; j = j - 8) {
						$AU.push(Number($BB[j]) << 7 | Number($BB[j + 1]) << 6 | Number($BB[j + 2]) << 5 | Number($BB[j + 3]) << 4 | Number($BB[j + 4]) << 3 | Number($BB[j + 5]) << 2 | Number($BB[j + 6]) << 1 | Number($BB[j + 7]));
					}
				}
				let i_2 = 0;
				for (; i_2 < 8; i_2++) {
					$AP[++$KEY_2] = $AU[i_2] ^ $AN();
				}
			}
			return $KEY_2;
		}"#;
const VECTOR_0_0: &str = r#"function $THREAD_5_6248($HL) {
								let $HM = [];
								let $KEY_0 = Math.random() * 4294967296 | 0;
								let $HN = [$KEY_0 >>> 24, $KEY_0 >> 16 & 255, $KEY_0 >> 8 & 255, $KEY_0 & 255];
								let $KEY_1 = $KEY_124($KEY_0 ^ 638053951);
								let $HO = $HL.length;
								let $HP = $KEY_83($KEY_1, $HL.length, $HN, 3);
								while ($HO - 1 >= 0) {
									$HP = $KEY_83($KEY_1, $HL[$HO - 1], $HN, $HP);
									--$HO;
								}
								$KEY_14.next([$HM, $HN]);
								$KEY_14.complete();
							}"#;
const VECTOR_0_1: &str = r#"function $THREAD_5_6248_2($HL) {
								let $HM = [];
								let $KEY_0 = Math.random() * 4294967296 | 0;
								let $HN = [$KEY_0 >>> 24, $KEY_0 >> 16 & 255, $KEY_0 >> 8 & 255, $KEY_0 & 255];
								let $KEY_1 = $KEY_124($KEY_0 ^ 638053951);
								let $HO = $HL.length;
								let $HP = $KEY_83($KEY_1, $HL.length, $HN, 3);
								while ($HO - 1 >= 0) {
									$HP = $KEY_83($KEY_1, $HL[$HO - 1], $HN, $HP);
									--$HO;
								}
								$KEY_14.next([$HM, $HN]);
								$KEY_14.complete();
							}"#;
const VECTOR_0_2: &str = r#"function $THREAD_5_6248_3($HL) {
								let $HM = [];
								let $KEY_0 = Math.random() * 4294967296 | 0;
								let $HN = [$KEY_0 >>> 24, $KEY_0 >> 16 & 255, $KEY_0 >> 8 & 255, $KEY_0 & 255];
								let $KEY_1 = $KEY_124($KEY_0 ^ 638053951);
								let $HO = $HL.length;
								let $HP = $KEY_83($KEY_1, $HL.length, $HN, 3);
								while ($HO - 1 >= 0) {
									$HP = $KEY_83($KEY_1, $HL[$HO - 1], $HN, $HP);
									--$HO;
								}
								$KEY_14.next([$HM, $HN]);
								$KEY_14.complete();
							}"#;
const VECTOR_0_3: &str = r#"function $THREAD_5_6248_4($HL) {
								let $HM = [];
								let $KEY_0 = Math.random() * 4294967296 | 0;
								let $HN = [$KEY_0 >>> 24, $KEY_0 >> 16 & 255, $KEY_0 >> 8 & 255, $KEY_0 & 255];
								let $KEY_1 = $KEY_124($KEY_0 ^ 638053951);
								let $HO = $HL.length;
								let $HP = $KEY_83($KEY_1, $HL.length, $HN, 3);
								while ($HO - 1 >= 0) {
									$HP = $KEY_83($KEY_1, $HL[$HO - 1], $HN, $HP);
									--$HO;
								}
								$KEY_14.next([$HM, $HN]);
								$KEY_14.complete();
							}"#;
const VECTOR_FACTORY_1: &str = r#"function $THREAD_9_159979($BX) {
			let $KEY_0, $KEY_2, $KEY_4, $KEY_5;
			function $THREAD_2_27394() {
				let $BY = $KEY_5;
				$KEY_5 = ($BY + 1) % 4;
				if (($BY + 1) % 4 === 0) {
					$KEY_0 = $KEY_4 + $KEY_2;
					let $BZ = $KEY_2;
					$KEY_2 = $BZ ^ $KEY_4;
					$KEY_4 = $BZ ^ $KEY_4 ^ ($KEY_4 << 26 | $KEY_4 >>> 6) ^ $KEY_2 << 9;
					$KEY_2 = $KEY_2 << 13 | $KEY_2 >>> 19;
					return $KEY_0 & 255;
				}
				if (($BY + 1) % 4 === 1) {
					return $KEY_0 >> 8 & 255;
				}
				if (($BY + 1) % 4 === 2) {
					return $KEY_0 >> 16 & 255;
				}
				if (($BY + 1) % 4 === 3) {
					return $KEY_0 >>> 24 & 255;
				} else {
					return void 0;
				}
			}
			$KEY_4 = $KEY_14;
			$KEY_2 = $BX;
			$KEY_5 = -1;
			$THREAD_2_27394();
			$THREAD_2_27394();
			$THREAD_2_27394();
			$THREAD_2_27394();
			return $THREAD_2_27394;
		}"#;
const VECTOR_WRITER_1: &str = r#"function $THREAD_3_124073($BX, $BY, $BZ, $CA) {
			let $KEY_0;
			if (($BY | 0) === $BY) {
				let $CB;
				if ($BY < 0) {
					$CB = $BY < 0;
				} else if ($BY === 0) {
					$CB = 1 / $BY < 0;
				} else {
					$CB = false;
				}
				let $CC, $CD;
				if ($CB) {
					$CC = 64;
					$CD = -$BY;
				} else {
					$CC = 0;
					$CD = $BY;
				}
				if ($CD < 32) {
					$KEY_0 = Number($CA) + 1;
					$BZ[$KEY_0] = ($CC | $CD) ^ $BX();
				} else if ($CD < 4096) {
					$KEY_0 = Number($CA) + 1;
					$BZ[$KEY_0] = ($CC | ($CD & 31 | 32)) ^ $BX();
					$BZ[++$KEY_0] = $CD >> 5 ^ $BX();
				} else if ($CD < 524288) {
					$KEY_0 = Number($CA) + 1;
					$BZ[$KEY_0] = ($CC | ($CD & 31 | 32)) ^ $BX();
					$BZ[++$KEY_0] = ($CD >> 5 & 127 | 128) ^ $BX();
					$BZ[++$KEY_0] = $CD >>> 12 ^ $BX();
				} else if ($CD < 67108864) {
					$KEY_0 = Number($CA) + 1;
					$BZ[$KEY_0] = ($CC | ($CD & 31 | 32)) ^ $BX();
					$BZ[++$KEY_0] = ($CD >> 5 & 127 | 128) ^ $BX();
					$BZ[++$KEY_0] = ($CD >>> 12 & 127 | 128) ^ $BX();
					$BZ[++$KEY_0] = $CD >>> 19 ^ $BX();
				} else {
					$KEY_0 = Number($CA) + 1;
					$BZ[$KEY_0] = ($CC | ($CD & 31 | 32)) ^ $BX();
					$BZ[++$KEY_0] = ($CD >> 5 & 127 | 128) ^ $BX();
					$BZ[++$KEY_0] = ($CD >>> 12 & 127 | 128) ^ $BX();
					$BZ[++$KEY_0] = ($CD >>> 19 & 127 | 128) ^ $BX();
					$BZ[++$KEY_0] = $CD >>> 26 ^ $BX();
				}
				return $KEY_0;
			} else {
				$KEY_0 = $CA;
				let $CE = Number($CA) + 1;
				$BZ[$CE] = $BX() ^ 128;
				let $CF;
				if ($KEY_116) {
					$KEY_116[0] = $BY;
					$CF = new $KEY_44($KEY_116.buffer);
				} else if ($BY !== $BY) {
					$CF = $KEY_126;
				} else {
					const $CG = $BY < 0 || $BY === 0 && 1 / $BY < 0;
					let $CH = Math.abs($BY);
					let $CI, $KEY_11;
					if ($CH >= Math.pow(2, -1022)) {
						let $KEY_1 = Math.min(Math.floor(Math.log($CH) / Math.LN2), 1023);
						$CH = $CH / Math.pow(2, $KEY_1);
						let $CJ = $CH < 1;
						$KEY_1 = $CJ ? $KEY_1 - 1 : $KEY_1;
						$CJ = $CJ ? $CH * 2 : $CH;
						let $CK = $CJ >= 2;
						let $KEY_12 = Math.pow(2, 52);
						$KEY_11 = Math.floor(($CK ? $CJ / 2 : $CJ) * $KEY_12);
						$KEY_11 = ($CK ? $CJ / 2 : $CJ) * $KEY_12 - $KEY_11 < .5 ? $KEY_11 : ($CK ? $CJ / 2 : $CJ) * $KEY_12 - $KEY_11 > .5 ? $KEY_11 + 1 : $KEY_11 % 2 ? $KEY_11 + 1 : $KEY_11;
						$CK = ($CK ? $KEY_1 + 1 : $KEY_1) + 1023;
						let $CL = ($KEY_11 - $KEY_12) / $KEY_12 >= 1;
						$CI = $CL ? $CK + 1 : $CK;
						if ($CI > 2046) {
							$CI = 2047;
							$KEY_11 = 0;
						} else {
							$KEY_11 = $CL ? 0 : $KEY_11 - $KEY_12;
						}
					} else {
						$CH = $CH / Math.pow(2, -1074);
						let $CM = Math.floor($CH);
						$CI = 0;
						$KEY_11 = $CH - $CM < .5 ? $CM : $CH - $CM > .5 ? $CM + 1 : $CM % 2 ? $CM + 1 : $CM;
					}
					let $CN = [];
					let k = 51;
					for (; k >= 0; k--) {
						$CN.unshift(!!($KEY_11 & 1));
						$KEY_11 = Math.floor($KEY_11 / 2);
					}
					let i = 11;
					for (; i > 0; i--) {
						$CN.unshift(!!($CI & 1));
						$CI = Math.floor($CI / 2);
					}
					$CN.unshift($CG);
					$CF = [];
					let j = 56;
					for (; j >= 0; j = j - 8) {
						$CF.push(Number($CN[j]) << 7 | Number($CN[j + 1]) << 6 | Number($CN[j + 2]) << 5 | Number($CN[j + 3]) << 4 | Number($CN[j + 4]) << 3 | Number($CN[j + 5]) << 2 | Number($CN[j + 6]) << 1 | Number($CN[j + 7]));
					}
				}
				let i_2 = 0;
				for (; i_2 < 8; i_2++) {
					$BZ[++$CE] = $CF[i_2] ^ $BX();
				}
				return $CE;
			}
		}"#;
const VECTOR_1_0: &str = r#"function $THREAD_3_8444($CT) {
						let $CU = [];
						let $KEY_5 = Math.random() * 4294967296 | 0;
						let $CV = [$KEY_5 >>> 24, $KEY_5 >> 16 & 255, $KEY_5 >> 8 & 255, $KEY_5 & 255];
						let $KEY_4 = $KEY_29($KEY_5 ^ 2565407423);
						let $CW = $CT.length;
						let $CX = $KEY_96($KEY_4, $CT.length, $CV, 3);
						while ($CW - 1 >= 0) {
							$CX = $KEY_96($KEY_4, $CT[$CW - 1], $CV, $CX);
							--$CW;
						}
						$KEY_19.next([$CU, $CV]);
						$KEY_19.complete();
					}"#;
const VECTOR_FACTORY_2: &str = r#"function $THREAD_8_146963($AQ) {
			let $KEY_0, $KEY_2, $KEY_4, $KEY_5;
			function $THREAD_7_140029() {
				let $AR = $KEY_0;
				$KEY_0 = ($AR + 1) % 4;
				if (($AR + 1) % 4 === 0) {
					$KEY_2 = $KEY_4 + $KEY_5;
					let $AS = $KEY_5;
					$KEY_5 = $AS ^ $KEY_4;
					$KEY_4 = $AS ^ $KEY_4 ^ ($KEY_4 << 26 | $KEY_4 >>> 6) ^ $KEY_5 << 9;
					$KEY_5 = $KEY_5 << 13 | $KEY_5 >>> 19;
					return $KEY_2 & 255;
				}
				if (($AR + 1) % 4 === 1) {
					return $KEY_2 >> 8 & 255;
				}
				if (($AR + 1) % 4 === 2) {
					return $KEY_2 >> 16 & 255;
				}
				if (($AR + 1) % 4 === 3) {
					return $KEY_2 >>> 24 & 255;
				} else {
					return void 0;
				}
			}
			$KEY_4 = $KEY_393;
			$KEY_5 = $AQ;
			$KEY_0 = -1;
			$THREAD_7_140029();
			$THREAD_7_140029();
			$THREAD_7_140029();
			$THREAD_7_140029();
			return $THREAD_7_140029;
		}"#;
const VECTOR_WRITER_2: &str = r#"function $THREAD_3_85407($AQ, $AR, $AS, $AT) {
			let $KEY_5;
			if (($AR | 0) === $AR) {
				let $AU;
				if ($AR < 0) {
					$AU = $AR < 0;
				} else if ($AR === 0) {
					$AU = 1 / $AR < 0;
				} else {
					$AU = $AR === 0;
				}
				let $AV, $AW;
				if ($AU) {
					$AV = -$AR;
					$AW = 64;
				} else {
					$AV = $AR;
					$AW = 0;
				}
				if ($AV < 32) {
					$KEY_5 = Number($AT) + 1;
					$AS[$KEY_5] = ($AW | $AV) ^ $AQ();
				} else if ($AV < 4096) {
					$KEY_5 = Number($AT) + 1;
					$AS[$KEY_5] = ($AW | ($AV & 31 | 32)) ^ $AQ();
					$AS[++$KEY_5] = $AV >> 5 ^ $AQ();
				} else if ($AV < 524288) {
					$KEY_5 = Number($AT) + 1;
					$AS[$KEY_5] = ($AW | ($AV & 31 | 32)) ^ $AQ();
					$AS[++$KEY_5] = ($AV >> 5 & 127 | 128) ^ $AQ();
					$AS[++$KEY_5] = $AV >>> 12 ^ $AQ();
				} else if ($AV < 67108864) {
					$KEY_5 = Number($AT) + 1;
					$AS[$KEY_5] = ($AW | ($AV & 31 | 32)) ^ $AQ();
					$AS[++$KEY_5] = ($AV >> 5 & 127 | 128) ^ $AQ();
					$AS[++$KEY_5] = ($AV >>> 12 & 127 | 128) ^ $AQ();
					$AS[++$KEY_5] = $AV >>> 19 ^ $AQ();
				} else {
					$KEY_5 = Number($AT) + 1;
					$AS[$KEY_5] = ($AW | ($AV & 31 | 32)) ^ $AQ();
					$AS[++$KEY_5] = ($AV >> 5 & 127 | 128) ^ $AQ();
					$AS[++$KEY_5] = ($AV >>> 12 & 127 | 128) ^ $AQ();
					$AS[++$KEY_5] = ($AV >>> 19 & 127 | 128) ^ $AQ();
					$AS[++$KEY_5] = $AV >>> 26 ^ $AQ();
				}
			} else {
				$KEY_5 = $AT;
				$KEY_5 = Number($AT) + 1;
				$AS[$KEY_5] = $AQ() ^ 128;
				let $AX;
				if ($KEY_98) {
					$KEY_98[0] = $AR;
					$AX = new $KEY_116($KEY_98.buffer);
				} else if ($AR !== $AR) {
					$AX = $KEY_132;
				} else {
					let $KEY_6 = $AR < 0 || $AR === 0 && 1 / $AR < 0;
					let $AY = Math.abs($AR);
					let $AZ, $KEY_7;
					if ($AY >= Math.pow(2, -1022)) {
						let $KEY_0 = Math.min(Math.floor(Math.log($AY) / Math.LN2), 1023);
						$AY = $AY / Math.pow(2, $KEY_0);
						let $BA = $AY < 1;
						$KEY_0 = $BA ? $KEY_0 - 1 : $KEY_0;
						$BA = $BA ? $AY * 2 : $AY;
						let $BB = $BA >= 2;
						$BA = $BB ? $BA / 2 : $BA;
						$KEY_7 = Math.pow(2, 52);
						let $KEY_8 = Math.floor($BA * $KEY_7);
						$BA = $BA * $KEY_7 - $KEY_8 < .5 ? $KEY_8 : $BA * $KEY_7 - $KEY_8 > .5 ? $KEY_8 + 1 : $KEY_8 % 2 ? $KEY_8 + 1 : $KEY_8;
						$BB = ($BB ? $KEY_0 + 1 : $KEY_0) + 1023;
						let $BC = ($BA - $KEY_7) / $KEY_7 >= 1;
						$AZ = $BC ? $BB + 1 : $BB;
						if ($AZ > 2046) {
							$AZ = 2047;
							$KEY_7 = 0;
						} else {
							$KEY_7 = $BC ? 0 : $BA - $KEY_7;
						}
					} else {
						$AY = $AY / Math.pow(2, -1074);
						let $BD = Math.floor($AY);
						$AZ = 0;
						$KEY_7 = $AY - $BD < .5 ? $BD : $AY - $BD > .5 ? $BD + 1 : $BD % 2 ? $BD + 1 : $BD;
					}
					let $BE = [];
					let k = 51;
					for (; k >= 0; k--) {
						$BE.unshift(!!($KEY_7 & 1));
						$KEY_7 = Math.floor($KEY_7 / 2);
					}
					let i = 11;
					for (; i > 0; i--) {
						$BE.unshift(!!($AZ & 1));
						$AZ = Math.floor($AZ / 2);
					}
					$BE.unshift($KEY_6);
					$AX = [];
					let j = 56;
					for (; j >= 0; j = j - 8) {
						$AX.push(Number($BE[j]) << 7 | Number($BE[j + 1]) << 6 | Number($BE[j + 2]) << 5 | Number($BE[j + 3]) << 4 | Number($BE[j + 4]) << 3 | Number($BE[j + 5]) << 2 | Number($BE[j + 6]) << 1 | Number($BE[j + 7]));
					}
				}
				let i_2 = 0;
				for (; i_2 < 8; i_2++) {
					$AS[++$KEY_5] = $AX[i_2] ^ $AQ();
				}
			}
			return $KEY_5;
		}"#;
const VECTOR_2_0: &str = r#"function $THREAD_7_124400($BW) {
						let $BX = [];
						let $KEY_2 = Math.random() * 4294967296 | 0;
						let $BY = [$KEY_2 >>> 24, $KEY_2 >> 16 & 255, $KEY_2 >> 8 & 255, $KEY_2 & 255];
						let $KEY_3 = $KEY_331($KEY_2 ^ 1947099284);
						let $BZ = $BW.length;
						let $CA = $KEY_96($KEY_3, $BW.length, $BY, 3);
						while ($BZ - 1 >= 0) {
							$CA = $KEY_96($KEY_3, $BW[$BZ - 1], $BY, $CA);
							--$BZ;
						}
						$KEY_30.next([$BX, $BY]);
						$KEY_30.complete();
					}"#;

fn vector_carrier(body: &str, variant: usize, target: &str) -> String {
    let (factory, writer, aliases, observer) = match variant {
        0 => (
            VECTOR_FACTORY_0,
            VECTOR_WRITER_0,
            "let $KEY_124=$THREAD_0_108369;let $KEY_83;$KEY_83=$THREAD_0_128689;",
            "$KEY_14",
        ),
        1 => (
            VECTOR_FACTORY_1,
            VECTOR_WRITER_1,
            "let $KEY_29=$THREAD_9_159979;let $KEY_96;$KEY_96=$THREAD_3_124073;",
            "$KEY_19",
        ),
        2 => (
            VECTOR_FACTORY_2,
            VECTOR_WRITER_2,
            "let $KEY_331=$THREAD_8_146963;let $KEY_96;$KEY_96=$THREAD_3_85407;",
            "$KEY_30",
        ),
        _ => unreachable!(),
    };
    // The real factory has six direct bindings and is reachable through its
    // initialized alias. This carrier otherwise also has six, so Model::new
    // rejects the equal broadest reachable scopes before any family proof.
    // A neutral seventh binding makes the intended carrier main unambiguous;
    // the exact candidate, factory, writer and alias sources remain intact.
    format!("function main(){{let {observer}={{next(){{}},complete(){{}}}},carrierAnchor=0;\n{factory}\n{writer}\n{aliases}\n{body}\nreturn {target};}}main();")
}

#[test]
fn integer_reverse_vector_real_helpers_expose_unpadded_carrier_scope_ambiguity() {
    for (body, variant, target) in [
        (VECTOR_0_0, 0, "$THREAD_5_6248"),
        (VECTOR_1_0, 1, "$THREAD_3_8444"),
        (VECTOR_2_0, 2, "$THREAD_7_124400"),
    ] {
        let unpadded = vector_carrier(body, variant, target).replacen(",carrierAnchor=0;", ";", 1);
        let error = name_threads(&unpadded, true)
            .err()
            .expect("real factory and unpadded carrier must tie before family evaluation");
        assert!(
            error.contains("main program scope (equal broadest reachable binding set)"),
            "unexpected failure before family evaluation: {error}"
        );
    }
}

#[test]
fn integer_reverse_vectors_all_six_real_bodies_are_exact_and_named() {
    for (body, variant, target, length, digest) in [
        (
            VECTOR_0_0,
            0,
            "$THREAD_5_6248",
            513,
            "2737da7a1177bbd553de3270480b1b9ca24afe806634738c3b5e33614fd82109",
        ),
        (
            VECTOR_0_1,
            0,
            "$THREAD_5_6248_2",
            515,
            "b1ad670e0d4818affafd5ed43af1a22f9b0b299e80f1d58d6ea2e742bac22607",
        ),
        (
            VECTOR_0_2,
            0,
            "$THREAD_5_6248_3",
            515,
            "01145e87a7618c7352edeb9288bcbe5e6b34e16f73a5e7cbc66150f7ff1e33a1",
        ),
        (
            VECTOR_0_3,
            0,
            "$THREAD_5_6248_4",
            515,
            "2891d73ed9692db6da32f5cb0b14b91a8954ba7c4bf82d1e6159d43ad89b2ed1",
        ),
        (
            VECTOR_1_0,
            1,
            "$THREAD_3_8444",
            487,
            "60db708943f25fd1d60248f996545e02f07918d838328487424ac2d843b08ae3",
        ),
        (
            VECTOR_2_0,
            2,
            "$THREAD_7_124400",
            490,
            "3874f0b0ae610817698c84cbd069ce7dc1927f56461234878f2f21371a0fab31",
        ),
    ] {
        assert_eq!(body.len(), length);
        assert_eq!(sha256_hex(body.as_bytes()), digest);
        let source = vector_carrier(body, variant, target);
        let result = name_threads(&source, true)
            .unwrap_or_else(|error| panic!("real vector fixture setup failed: {error}\n{source}"));
        assert!(
            result.report.function_roles.iter().any(|role| {
                role.current_name.as_deref() == Some(target)
                    && role
                        .candidates
                        .iter()
                        .any(|name| name == "emitSeededHashEncoding")
                    && role
                        .evaluators
                        .iter()
                        .any(|name| name == "encoding_seeded_hash_encoding")
            }),
            "owning vector lacks source proof: {source}"
        );
        assert!(
            result.report.outcomes.iter().any(|outcome| {
                outcome.binding.name == target
                    && outcome.applied
                    && outcome.proposed_name.as_deref() == Some("emitSeededHashEncoding")
            }),
            "real vector was not named: {source}"
        );
    }
}

// Frame-preservation witness with the real factory, writer, and their initialized or
// assigned aliases.
const PRESERVE_FRAME_FACTORY: &str = r#"function $THREAD_5_40654($BN) {
			let $KEY_0, $KEY_1, $KEY_2, $KEY_4;
			function $THREAD_0_23280() {
				let $BO = $KEY_0;
				$KEY_0 = ($BO + 1) % 4;
				if (($BO + 1) % 4 === 0) {
					$KEY_4 = $KEY_1 + $KEY_2;
					let $BP = $KEY_2;
					$KEY_2 = $BP ^ $KEY_1;
					$KEY_1 = $BP ^ $KEY_1 ^ ($KEY_1 << 26 | $KEY_1 >>> 6) ^ $KEY_2 << 9;
					$KEY_2 = $KEY_2 << 13 | $KEY_2 >>> 19;
					return $KEY_4 & 255;
				}
				if (($BO + 1) % 4 === 1) {
					return $KEY_4 >> 8 & 255;
				}
				if (($BO + 1) % 4 === 2) {
					return $KEY_4 >> 16 & 255;
				}
				if (($BO + 1) % 4 === 3) {
					return $KEY_4 >>> 24 & 255;
				} else {
					return void 0;
				}
			}
			$KEY_1 = $KEY_109;
			$KEY_2 = $BN;
			$KEY_0 = -1;
			$THREAD_0_23280();
			$THREAD_0_23280();
			$THREAD_0_23280();
			$THREAD_0_23280();
			return $THREAD_0_23280;
		}"#;
const PRESERVE_FRAME_WRITER: &str = r#"function $THREAD_2_34310($BN, $BO, $BP, $BQ) {
			let $KEY_4;
			if (($BO | 0) === $BO) {
				let $BR;
				if ($BO < 0) {
					$BR = $BO < 0;
				} else if ($BO === 0) {
					$BR = 1 / $BO < 0;
				} else {
					$BR = $BO === 0;
				}
				let $BS, $BT;
				if ($BR) {
					$BS = 64;
					$BT = -$BO;
				} else {
					$BS = 0;
					$BT = $BO;
				}
				if ($BT < 32) {
					$KEY_4 = Number($BQ) + 1;
					$BP[$KEY_4] = ($BS | $BT) ^ $BN();
				} else if ($BT < 4096) {
					$KEY_4 = Number($BQ) + 1;
					$BP[$KEY_4] = ($BS | ($BT & 31 | 32)) ^ $BN();
					$BP[++$KEY_4] = $BT >> 5 ^ $BN();
				} else if ($BT < 524288) {
					$KEY_4 = Number($BQ) + 1;
					$BP[$KEY_4] = ($BS | ($BT & 31 | 32)) ^ $BN();
					$BP[++$KEY_4] = ($BT >> 5 & 127 | 128) ^ $BN();
					$BP[++$KEY_4] = $BT >>> 12 ^ $BN();
				} else if ($BT < 67108864) {
					$KEY_4 = Number($BQ) + 1;
					$BP[$KEY_4] = ($BS | ($BT & 31 | 32)) ^ $BN();
					$BP[++$KEY_4] = ($BT >> 5 & 127 | 128) ^ $BN();
					$BP[++$KEY_4] = ($BT >>> 12 & 127 | 128) ^ $BN();
					$BP[++$KEY_4] = $BT >>> 19 ^ $BN();
				} else {
					$KEY_4 = Number($BQ) + 1;
					$BP[$KEY_4] = ($BS | ($BT & 31 | 32)) ^ $BN();
					$BP[++$KEY_4] = ($BT >> 5 & 127 | 128) ^ $BN();
					$BP[++$KEY_4] = ($BT >>> 12 & 127 | 128) ^ $BN();
					$BP[++$KEY_4] = ($BT >>> 19 & 127 | 128) ^ $BN();
					$BP[++$KEY_4] = $BT >>> 26 ^ $BN();
				}
			} else {
				$KEY_4 = $BQ;
				let $KEY_6 = Number($BQ) + 1;
				$BP[$KEY_6] = $BN() ^ 128;
				let $BU;
				if ($KEY_390) {
					$KEY_390[0] = $BO;
					$BU = new $KEY_152($KEY_390.buffer);
				} else if ($BO !== $BO) {
					$BU = $KEY_237;
				} else {
					const $BV = $BO < 0 || $BO === 0 && 1 / $BO < 0;
					let $BW = Math.abs($BO);
					let $BX;
					if ($BW >= Math.pow(2, -1022)) {
						let $KEY_3 = Math.min(Math.floor(Math.log($BW) / Math.LN2), 1023);
						$BW = $BW / Math.pow(2, $KEY_3);
						let $BY = $BW < 1;
						$BW = $BY ? $BW * 2 : $BW;
						$BY = $BY ? $KEY_3 - 1 : $KEY_3;
						if ($BW >= 2) {
							++$BY;
							$BW = $BW / 2;
						}
						let $BZ = Math.pow(2, 52);
						let $KEY_5 = Math.floor($BW * $BZ);
						$BW = $BW * $BZ;
						$BW = $BW - $KEY_5 < .5 ? $KEY_5 : $BW - $KEY_5 > .5 ? $KEY_5 + 1 : $KEY_5 % 2 ? $KEY_5 + 1 : $KEY_5;
						$BY += 1023;
						let $CA = ($BW - $BZ) / $BZ >= 1;
						$BX = $CA ? $BY + 1 : $BY;
						if ($BX > 2046) {
							$BX = 2047;
							$BW = 0;
						} else {
							$BW = $CA ? 0 : $BW - $BZ;
						}
					} else {
						$BW = $BW / Math.pow(2, -1074);
						let $CB = Math.floor($BW);
						$BX = 0;
						$BW = $BW - $CB < .5 ? $CB : $BW - $CB > .5 ? $CB + 1 : $CB % 2 ? $CB + 1 : $CB;
					}
					let $CC = [];
					let k = 51;
					for (; k >= 0; k--) {
						$CC.unshift(!!($BW & 1));
						$BW = Math.floor($BW / 2);
					}
					let i = 11;
					for (; i > 0; i--) {
						$CC.unshift(!!($BX & 1));
						$BX = Math.floor($BX / 2);
					}
					$CC.unshift($BV);
					$BU = [];
					let j = 56;
					for (; j >= 0; j = j - 8) {
						$BU.push(Number($CC[j]) << 7 | Number($CC[j + 1]) << 6 | Number($CC[j + 2]) << 5 | Number($CC[j + 3]) << 4 | Number($CC[j + 4]) << 3 | Number($CC[j + 5]) << 2 | Number($CC[j + 6]) << 1 | Number($CC[j + 7]));
					}
				}
				let i_2 = 0;
				for (; i_2 < 8; i_2++) {
					$BP[++$KEY_6] = $BU[i_2] ^ $BN();
				}
				$KEY_4 = $KEY_6;
			}
			return $KEY_4;
		}"#;
const PRESERVE_FRAME_BODY: &str = r#"function $THREAD_0_38208($CG) {
						let $KEY_4 = Math.random() * 4294967296 | 0;
						let $CH = [];
						let $CI = [$KEY_4 >>> 24, $KEY_4 >> 16 & 255, $KEY_4 >> 8 & 255, $KEY_4 & 255];
						let $KEY_0 = $KEY_448($KEY_4 ^ 2705884635);
						let $CJ = $CG.length;
						let $CK = $KEY_428($KEY_0, $CJ, $CI, 3);
						while ($CJ - 1 >= 0) {
							$CK = $KEY_428($KEY_0, $CG[$CJ - 1], $CI, $CK);
							--$CJ;
						}
						$KEY_11.next([$CH, $CI]);
						$KEY_11.complete();
					}"#;
const PRESERVE_FRAME_SHA256: &str =
    "6838293c81fad8501a8dac78e07ca04de742da1a40ac393d7e769be93a512203";

fn preservation_frame_carrier(body: &str, target: &str) -> String {
    format!(
        "function main(){{let $KEY_11={{next(){{}},complete(){{}}}},carrierAnchor=0;\n{PRESERVE_FRAME_FACTORY}\n{PRESERVE_FRAME_WRITER}\nlet $KEY_448=$THREAD_5_40654;let $KEY_428;$KEY_428=$THREAD_2_34310;\n{body}\nreturn {target};}}main();"
    )
}

fn assert_preserved_frame(source: &str, target: &str) {
    let result = name_threads(source, true)
        .unwrap_or_else(|error| panic!("preservation fixture setup failed: {error}\n{source}"));
    let role = result
        .report
        .function_roles
        .iter()
        .find(|role| role.current_name.as_deref() == Some(target))
        .expect("preservation owner must be indexed");
    assert!(
        role.candidates
            .iter()
            .any(|name| name == "emitSeededSignalFrame"),
        "{role:?}"
    );
    assert!(
        role.evaluators
            .iter()
            .any(|name| name == "signals_seeded_signal_frame"),
        "{role:?}"
    );
    assert!(
        !role
            .candidates
            .iter()
            .any(|name| name == "emitSeededHashEncoding"),
        "{role:?}"
    );
    assert!(
        !role
            .evaluators
            .iter()
            .any(|name| name == "encoding_seeded_hash_encoding"),
        "{role:?}"
    );
    assert!(
        result.report.outcomes.iter().any(|outcome| {
            outcome.binding.name == target
                && outcome.applied
                && outcome.proposed_name.as_deref() == Some("emitSeededSignalFrame")
        }),
        "existing frame name was not preserved: {source}"
    );
}

#[test]
fn integer_descending_preserves_exact_cached_frame_frame_and_alpha_salt_equivalents() {
    assert_eq!(PRESERVE_FRAME_BODY.len(), 484);
    assert_eq!(
        sha256_hex(PRESERVE_FRAME_BODY.as_bytes()),
        PRESERVE_FRAME_SHA256
    );
    let target = "$THREAD_0_38208";
    for salt in ["0", "2705884635", "4294967295"] {
        let body = PRESERVE_FRAME_BODY.replace("2705884635", salt);
        let direct_length = body.replace("$KEY_0, $CJ, $CI, 3", "$KEY_0, $CG.length, $CI, 3");
        let later_payload = body
            .replace("let $CH = [];", "")
            .replace("let $KEY_0 =", "let $CH = [];\nlet $KEY_0 =");
        for equivalent in [body, direct_length, later_payload] {
            let source = preservation_frame_carrier(&equivalent, target);
            assert_preserved_frame(&source, target);
            // Alpha-renaming includes actual helpers and captured aliases.
            let renamed = source
                .replace(target, "$THREAD_619_771_2")
                .replace("$THREAD_5_40654", "makeStream")
                .replace("$THREAD_2_34310", "writeField")
                .replace("$THREAD_0_23280", "nextByte")
                .replace("$KEY_448", "factoryAlias")
                .replace("$KEY_428", "writerAlias")
                .replace("$KEY_11", "observer")
                .replace("$KEY_4", "$KEY_901")
                .replace("$KEY_0", "$KEY_902")
                .replace("$CG", "input")
                .replace("$CH", "payload")
                .replace("$CI", "header")
                .replace("$CJ", "remaining")
                .replace("$CK", "cursor");
            assert_preserved_frame(&renamed, "$THREAD_619_771_2");
            assert_preserved_frame(&source.replace("let ", "var "), target);
        }
    }
}

#[test]
fn integer_descending_defers_only_to_same_owner_actual_frame_proof() {
    // These equivalent vector encodings do not meet the existing Frame proof. The pure callback
    // determines whether the integer extension should defer.
    let payload_first = PRESERVE_FRAME_BODY.replace("let $CH = [];", "").replace(
        "let $KEY_4 = Math.random()",
        "let $CH = [];\nlet $KEY_4 = Math.random()",
    );
    let unsigned = PRESERVE_FRAME_BODY.replace(
        "Math.random() * 4294967296 | 0",
        "(Math.random() * 4294967296) >>> 0",
    );
    for body in [&payload_first, &unsigned] {
        let source = preservation_frame_carrier(body, "$THREAD_0_38208");
        assert!(
            named(&source, "$THREAD_0_38208", "emitSeededHashEncoding"),
            "{source}"
        );
    }
    let sibling = payload_first.replace("$THREAD_0_38208", "$THREAD_619_810");
    let both = format!("{PRESERVE_FRAME_BODY}\n{sibling}");
    let source = preservation_frame_carrier(&both, "$THREAD_619_810");
    assert!(
        named(&source, "$THREAD_619_810", "emitSeededHashEncoding"),
        "{source}"
    );
    assert_preserved_frame(&source, "$THREAD_0_38208");
}

#[test]
fn integer_descending_preservation_keeps_existing_fixed_dual_proof_hash_governance() {
    // The existing fixed family still proves the frame shape. Its canonical Hash name must
    // survive this deferral, which applies only to descending integer encoding.
    let source = EMIT_FIXED
        .replace(
            "let seed=Math.random()*4294967296;",
            "let seed=Math.random()*4294967296|0;",
        )
        .replace("(seed|0)", "seed");
    let result = name_threads(&source, true).expect("fixed agreement carrier must resolve");
    let role = result
        .report
        .function_roles
        .iter()
        .find(|role| role.current_name.as_deref() == Some("$THREAD_4_4"))
        .expect("fixed owner must be indexed");
    assert!(
        role.candidates
            .iter()
            .any(|name| name == "emitSeededHashEncoding"),
        "{role:?}"
    );
    assert!(
        role.evaluators
            .iter()
            .any(|name| name == "encoding_seeded_hash_encoding"),
        "{role:?}"
    );
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == "$THREAD_4_4")
        .expect("fixed owner must have a naming outcome");
    assert!(
        outcome.applied && outcome.proposed_name.as_deref() == Some("emitSeededHashEncoding"),
        "{outcome:?}"
    );
    assert!(
        outcome
            .explanation
            .iter()
            .any(|line| line.contains("canonical governed label emitSeededHashEncoding retained")),
        "{outcome:?}"
    );
}
