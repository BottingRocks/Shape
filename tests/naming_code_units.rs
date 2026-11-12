// Keep the accepted packing negatives in the same focused build/run.
include!("naming_thread_shared_packing.rs");

#[test]
fn explicit_iv_guard_proves_complete_packing_and_rejects_foreign_values() {
    let source = ENCRYPT.replace(
        r#"pending=subtle.generateKey({name:"AES-GCM",length:128}"#,
        r#"let algorithm={name:"AES-GCM"};Object.defineProperty(algorithm,"length",{writable:true,configurable:true,enumerable:true,value:128});pending=subtle.generateKey(algorithm"#
    ).replace("let nonce=iv;", "let nonce=iv.buffer;").replace(
        "let iv=context.crypto?.getRandomValues(new Uint8Array(12));",
        "let iv;if(context.crypto==null){iv=void 0;}else{iv=context.crypto.getRandomValues(new Uint8Array(12));}"
    );
    for s in [source.clone(), source.replace("generated", "resultKey").replace("encoded", "plaintextBytes")] {
        assert!(named(&s, "encryptStringAesGcm"));
    }
    refuse_changes(&source, "encryptStringAesGcm", &[
        ("crypto==null", "crypto!=null"),
        ("iv=void 0", "iv=null"),
        ("Uint8Array(12)", "Uint8Array(13)"),
        ("iv:iv", "iv:encoded"),
        ("let nonce=iv.buffer", "let nonce=encoded.buffer"),
        ("getRandomValues(new", "getRandomValues(sideEffect(),new"),
        ("iv=void 0;", "iv=void 0;sideEffect();"),
        ("let context=globalThis", "let context=globalThis;context=other"),
        ("let subtle=context.crypto?.subtle;", "let subtle=context.crypto?.subtle;subtle=other;"),
    ]);
}

#[test]
fn explicit_decode_fallback_keeps_own_result_and_parse_order() {
    let source = DECRYPT.replace("let value=JSON.parse(new TextDecoder().decode(result)||\"\");",
        "let value=new TextDecoder().decode(result);if(!value){value=\"\";}value=JSON.parse(value);");
    assert!(named(&source, "decryptPackedAesGcmBlob"));
    assert!(named(&source.replace("result", "decodedResult").replace("nonce", "ivBytes"), "decryptPackedAesGcmBlob"));
    refuse_changes(&source, "decryptPackedAesGcmBlob", &[
        ("if(!value)", "if(value)"),
        ("value=\"\";", "value=\"null\";"),
        ("JSON.parse(value)", "JSON.parse(result)"),
        ("decode(result)", "decode(encrypted)"),
        ("value=JSON.parse(value);", "sideEffect();value=JSON.parse(value);"),
        ("buffer.slice(16,28)", "buffer.slice(15,28)"),
        ("return value;", "return result;"),
    ]);
}

const JOIN_TRUTHY: &str = r#"function main(){function $THREAD_1_2(input){let output=[];if(input!=null){input.forEach(unit=>{output.push(String.fromCharCode(unit));});}return (output?output:output-1).join("");}return $THREAD_1_2;}main();"#;
const JOIN_COMPUTED: &str = r#"function main(){function $THREAD_1_2(input){let output=[];if(input!=null){input.forEach(unit=>{output.push(String.fromCharCode(unit));});}let absent=!output;return output[absent?"isArray":"join"]("");}return $THREAD_1_2;}main();"#;
#[test]
fn join_truthy_receivers_prove_every_callback_and_reject_shadow_alias_writers() {
    for source in [JOIN_TRUTHY, JOIN_COMPUTED] {
        assert!(named(source,"codeUnitsToString"));
        assert!(named(&source.replace("output","units").replace("unit=>","item=>").replace("fromCharCode(unit)","fromCharCode(item)"),"codeUnitsToString"));
        refuse_changes(source,"codeUnitsToString",&[
            ("input!=null","input==null"),
            ("fromCharCode(unit)","fromCharCode(input)"),
            ("output.push(","input.push("),
            ("String.fromCharCode", "Other.fromCharCode"),
            ("let output=[];", "let output=[];output=foreign;"),
            ("unit=>{", "unit=>{let output=[];"),
            ("function main(){", "function main(String){"),
            ("function main(){", "function main(){let String=Other;"),
            ("return output[", "return input["),
        ].iter().copied().filter(|(a,_)| source.contains(a)).collect::<Vec<_>>().as_slice());
    }
    assert!(!named(&JOIN_COMPUTED.replace("absent?\"isArray\":\"join\"", "absent?\"join\":\"isArray\""),"codeUnitsToString"));
}

const INTERVAL: &str = r#"function main(){let clock=()=>Date.now();let previous=0,ring={},maximum,minimum,count=0,complete=false;
function $THREAD_1_2(){let now=clock();let old=previous;previous=now;ring.put(now-old);
if(maximum===void 0&&minimum===void 0){maximum=now-old;minimum=now-old;}
else if(now-old>maximum){maximum=now-old;}else if(now-old<minimum){minimum=now-old;}
++count;complete=true;}return $THREAD_1_2;}main();"#;
#[test]
fn interval_completion_and_precomputed_delta_keep_order_receivers_and_flag() {
    let delta=INTERVAL.replace("let old=previous;", "let elapsed=now-previous;").replace("now-old", "elapsed");
    for source in [INTERVAL.to_string(),delta] {
        assert!(named(&source,"recordIntervalAndExtrema"),"{source}");
        assert!(named(&source.replace("previous","last").replace("maximum","hi"),"recordIntervalAndExtrema"));
        refuse_changes(&source,"recordIntervalAndExtrema",&[
            ("complete=true", "complete=false"),
            ("complete=true", "count=true"),
            ("++count", "count++"),
            ("ring.put(", "previous.put("),
            ("previous=now;", "previous=0;"),
            ("&&minimum", "||minimum"),
            ("complete=true;", "complete=true;sideEffect();"),
            ("clock=()=>Date.now()", "clock=unknownCallable"),
        ]);
    }
}
