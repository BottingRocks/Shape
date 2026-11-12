use shape::naming::labels::build_label_reference;
use shape::naming::{name_threads, name_threads_with_labels};

// Portable algorithms only. No captured program, build tokens or metadata.
const ENCRYPT: &str = r#"function main(){let context=globalThis;let subtle=context.crypto?.subtle;
function $THREAD_1_10(input){
let encoded=new TextEncoder().encode(input);let pending;
if(subtle!=null){pending=subtle.generateKey({name:"AES-GCM",length:128},true,["encrypt","decrypt"]).then(key=>key);}else{pending=null;}
if(pending!=null){return pending.then(generated=>{
let key=generated;let iv=context.crypto?.getRandomValues(new Uint8Array(12));
return subtle.encrypt({name:"AES-GCM",iv:iv},key,encoded).then(encrypted=>{
let bytes=encrypted;return subtle.exportKey("raw",key).then(raw=>{
let data=bytes;let nonce=iv;let packed=new Uint8Array(data.byteLength+raw.byteLength+nonce.byteLength);
packed.set(new Uint8Array(raw),0);packed.set(new Uint8Array(nonce),raw.byteLength);
packed.set(new Uint8Array(data),raw.byteLength+nonce.byteLength);return packed.buffer;
});});});}else{return null;}}return $THREAD_1_10;}main();"#;

const DECRYPT: &str = r#"function main(){let context=globalThis;let subtle=context.crypto?.subtle;
function diagnostic(message){console.log(message);}let report=diagnostic;
function $THREAD_2_20(input){
let buffer=new Uint8Array(input.split(",").map(item=>parseInt(item,10))).buffer;
let key=buffer.slice(0,16);let iv=buffer.slice(16,28);let data=buffer.slice(28,buffer.byteLength);
key={extractedKey:key,extractedIv:void 0,extractedData:void 0};key.extractedIv=iv;key.extractedData=data;
let encrypted=key.extractedData;let nonce=key.extractedIv;
return subtle.importKey("raw",key.extractedKey,"AES-GCM",true,["encrypt","decrypt"]).then(imported=>{
let algorithm={name:"AES-GCM"};algorithm.iv=nonce;
return subtle.decrypt(algorithm,imported,encrypted).then(result=>result);
}).then(result=>{let value=JSON.parse(new TextDecoder().decode(result)||"");
if(value==null){report({message:"missing data"});return void 0;}else{return value;}});}
return $THREAD_2_20;}main();"#;

fn named(source: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .outcomes
            .iter()
            .any(|o| o.applied && o.proposed_name.as_deref() == Some(name))
    })
}
fn refuse_changes(source: &str, name: &str, changes: &[(&str, &str)]) {
    for (from, to) in changes {
        assert!(source.contains(from), "missing mutation input: {from}");
        assert!(
            !named(&source.replace(from, to), name),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn encryption_proves_complete_callbacks_and_packing_with_alpha_variants() {
    for source in [
        ENCRYPT.to_string(),
        ENCRYPT
            .replace("$THREAD_1_10", "$THREAD_999_123456_2")
            .replace("input", "$KEY_997")
            .replace("subtle", "service")
            .replace("crypto?.service", "crypto?.subtle")
            .replace("packed", "resultBytes"),
        ENCRYPT.replace("let iv=context", "let iv;iv=context"),
        ENCRYPT.replace(
            "let subtle=context.crypto?.subtle;",
            "let subtle;subtle=context.crypto?.subtle;",
        ),
        ENCRYPT.replace("name:\"AES-GCM\",length:128", "length:128,name:\"AES-GCM\""),
        ENCRYPT.replace("let encoded=", "const encoded="),
        ENCRYPT.replace("let context=globalThis;", "let context;context=globalThis;"),
    ] {
        assert!(named(&source, "encryptStringAesGcm"), "{source}");
        let r = name_threads(&source, true).unwrap();
        let mut replay = source;
        for edit in r.report.edits.iter().rev() {
            replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
        }
        assert_eq!(replay, r.source);
    }
}

#[test]
fn encryption_rejects_api_argument_order_return_and_algorithm_changes() {
    refuse_changes(
        ENCRYPT,
        "encryptStringAesGcm",
        &[
            ("TextEncoder", "OtherEncoder"),
            (".encode(input)", ".encode(other)"),
            ("generateKey(", "deriveKey("),
            ("length:128", "length:256"),
            ("AES-GCM", "AES-CBC"),
            (
                "true,[\"encrypt\",\"decrypt\"]",
                "false,[\"encrypt\",\"decrypt\"]",
            ),
            ("[\"encrypt\",\"decrypt\"]", "[\"decrypt\",\"encrypt\"]"),
            (".then(key=>key)", ".then(key=>other)"),
            ("getRandomValues", "randomUUID"),
            ("Uint8Array(12)", "Uint8Array(16)"),
            ("iv:iv},key,encoded", "iv:iv},encoded,key"),
            ("exportKey(\"raw\",key)", "exportKey(\"jwk\",key)"),
            ("Uint8Array(raw),0", "Uint8Array(raw),1"),
            (
                "Uint8Array(nonce),raw.byteLength",
                "Uint8Array(data),raw.byteLength",
            ),
            (
                "raw.byteLength+nonce.byteLength);return",
                "nonce.byteLength+raw.byteLength);return",
            ),
            ("return packed.buffer", "return packed"),
            ("return null", "return void 0"),
            ("return subtle.encrypt", "subtle.encrypt"),
            ("return subtle.exportKey", "subtle.exportKey"),
            ("return pending.then", "pending.then"),
            ("let data=bytes", "let data=encoded"),
            ("pending!=null", "pending==null"),
            (
                "let iv=context.crypto?.getRandomValues",
                "let iv=other.crypto?.getRandomValues",
            ),
            ("let key=generated;", "let key=generated;sideEffect();"),
            ("let key=generated;", "let key=generated;return function(){"),
            (".then(generated=>", ".then(async generated=>"),
            (".then(raw=>", ".then((raw=other)=>"),
            (".then(raw=>", ".then((...raw)=>"),
            (
                "function $THREAD_1_10(input)",
                "function $THREAD_1_10({input})",
            ),
            (
                "function $THREAD_1_10(input)",
                "function $THREAD_1_10(input=other)",
            ),
            (
                "function $THREAD_1_10(input)",
                "function $THREAD_1_10(input,extra)",
            ),
        ],
    );
}

#[test]
fn crypto_captures_require_single_plain_origins_and_one_context() {
    refuse_changes(
        ENCRYPT,
        "encryptStringAesGcm",
        &[
            (
                "let subtle=context.crypto?.subtle;",
                "let subtle=context.crypto?.other;",
            ),
            (
                "let subtle=context.crypto?.subtle;",
                "let subtle=other.crypto?.subtle;",
            ),
            (
                "let subtle=context.crypto?.subtle;",
                "let subtle=context.crypto?.subtle;subtle=other;",
            ),
            (
                "let subtle=context.crypto?.subtle;",
                "var subtle=context.crypto?.subtle;var subtle;",
            ),
            (
                "let context=globalThis;",
                "var context=globalThis;var context;",
            ),
            (
                "let context=globalThis;",
                "let context=globalThis;context=other;",
            ),
            (
                "let subtle=context.crypto?.subtle;",
                "let {subtle}=other;subtle=context.crypto?.subtle;",
            ),
            (
                "let subtle=context.crypto?.subtle;",
                "let [subtle]=other;subtle=context.crypto?.subtle;",
            ),
            (
                "function main(){let context=globalThis;let subtle=context.crypto?.subtle;",
                "function main(subtle){let context=globalThis;subtle=context.crypto?.subtle;",
            ),
            (
                "function main(){let context=globalThis;let subtle=context.crypto?.subtle;",
                "function main(context){context=globalThis;let subtle=context.crypto?.subtle;",
            ),
            (
                "let encoded=new TextEncoder()",
                "let TextEncoder=Other;let encoded=new TextEncoder()",
            ),
            (
                "let encoded=new TextEncoder()",
                "var input;let encoded=new TextEncoder()",
            ),
            (".then(raw=>{", ".then(raw=>{let Uint8Array=Other;"),
            ("let encoded=", "let Uint8Array=Other;let encoded="),
        ],
    );
}

#[test]
fn decryption_proves_receiver_preserving_slices_and_separate_record_variant() {
    let call=DECRYPT.replace("let key=buffer.slice(0,16);let iv=buffer.slice(16,28);",
        "let pair=[buffer,buffer.slice];let key=pair[1].call(pair[0],0,16);let other=[buffer,buffer.slice];let iv=other[1].call(other[0],16,28);");
    let record=DECRYPT.replace("key={extractedKey:key,extractedIv:void 0,extractedData:void 0};key.extractedIv=iv;key.extractedData=data;",
        "let record={extractedKey:void 0,extractedIv:void 0,extractedData:void 0};record.extractedKey=key;record.extractedIv=iv;record.extractedData=data;").replace("key.extracted","record.extracted");
    for source in [
        DECRYPT.to_string(),
        call.clone(),
        record,
        call.replace("$THREAD_2_20", "$THREAD_567_899_3")
            .replace("buffer", "$KEY_100")
            .replace(").$KEY_100", ").buffer")
            .replace("imported", "keyObject"),
        DECRYPT.replace("missing data", "any unrelated diagnostic text"),
        DECRYPT.replace("report({message:", "diagnostic({message:"),
        DECRYPT.replace("let report=diagnostic;", "let report;report=diagnostic;"),
    ] {
        assert!(named(&source, "decryptPackedAesGcmBlob"), "{source}");
    }
}

#[test]
fn decryption_rejects_changed_decoding_slices_keys_callbacks_and_finalization() {
    refuse_changes(DECRYPT,"decryptPackedAesGcmBlob",&[
        ("split(\",\")","split(\";\")"),("parseInt(item,10)","parseInt(item,16)"),
        ("parseInt(item,10)","parseInt(other,10)"),
        (".map(item=>", ".map(({item})=>"),
        ("slice(0,16)","slice(0,15)"),("slice(16,28)","slice(16,29)"),
        ("slice(28,buffer.byteLength)","slice(28,other.byteLength)"),
        ("extractedKey:key","extractedKey:iv"),("key.extractedIv=iv","key.extractedIv=data"),
        ("importKey(\"raw\"","importKey(\"jwk\""),
        ("true,[\"encrypt\",\"decrypt\"]","false,[\"encrypt\",\"decrypt\"]"),
        ("decrypt(algorithm,imported,encrypted)","decrypt(algorithm,encrypted,imported)"),
        ("algorithm.iv=nonce","algorithm.iv=encrypted"),
        ("then(result=>result)","then(result=>encrypted)"),
        ("JSON.parse", "JSON.stringify"),("TextDecoder","TextEncoder"),
        ("decode(result)||\"\"","decode(result)||\"{}\""),
        ("value==null","value!=null"),("return value","return result"),
        ("return void 0","return null"),("report({message:","report({other:"),
        ("function $THREAD_2_20(input)","function $THREAD_2_20(input,JSON)"),
        ("let report=diagnostic;","let report=diagnostic;report=other;"),
        ("let report=diagnostic;","var report=diagnostic;var report;"),
        ("let report=diagnostic;","let report=diagnostic;diagnostic=other;"),
        ("let report=diagnostic;","let report=unknownCallable;"),
        ("function diagnostic(message){console.log(message);}","function diagnostic(message){console.log(message);}function diagnostic(message){other(message);}"),
    ]);
    let call = DECRYPT
        .replace(
            "let key=buffer.slice(0,16);",
            "let pair=[buffer,buffer.slice];let key=pair[1].call(pair[0],0,16);",
        )
        .replace(
            "let iv=buffer.slice(16,28);",
            "let other=[buffer,buffer.slice];let iv=other[1].call(other[0],16,28);",
        );
    refuse_changes(
        &call,
        "decryptPackedAesGcmBlob",
        &[
            ("pair[1].call(pair[0]", "pair[1].call(other[0]"),
            ("[buffer,buffer.slice]", "[buffer,buffer.splice]"),
            ("other[1].call(other[0]", "other[1].call(pair[0]"),
        ],
    );
}

#[test]
fn captured_runtime_origins_and_redeclarations_cannot_prove_crypto_services() {
    let parameter = ENCRYPT.replace(
        "function main(){let context=globalThis;let subtle=context.crypto?.subtle;",
        "function main(subtle){let context=globalThis;subtle=context.crypto?.subtle;",
    );
    let prefixes = [
        ("function main(subtle){", "function main(subtle=external){"),
        ("function main(subtle){", "function main(...subtle){"),
        ("function main(subtle){", "function main({subtle}){"),
        (
            "function main(subtle){",
            "function main(){try{throw external;}catch(subtle){",
        ),
        (
            "function main(subtle){",
            "function main(){for(let subtle of external){",
        ),
        (
            "function main(subtle){",
            "function main(){for(let subtle in external){",
        ),
    ];
    assert!(!named(&parameter, "encryptStringAesGcm"));
    for (from, to) in prefixes {
        let mut source = parameter.replace(from, to);
        if to.contains("catch") || to.contains("for(") {
            source = source.replace(";}main();", ";}}main();");
        }
        assert!(!named(&source, "encryptStringAesGcm"), "{source}");
    }
}

#[test]
fn names_collisions_and_inherited_conflicts_preserve_existing_policy() {
    for (source, generated, name) in [
        (ENCRYPT, "$THREAD_1_10", "encryptStringAesGcm"),
        (DECRYPT, "$THREAD_2_20", "decryptPackedAesGcmBlob"),
    ] {
        let human = source.replace(generated, "humanChosenName");
        assert!(name_threads(&human, true)
            .unwrap()
            .source
            .contains("function humanChosenName("));
        let collision = source.replace(
            "function main(){",
            &format!("function main(){{let {name}=42;consume({name});"),
        );
        assert!(!named(&collision, name));
        let reference = build_label_reference(source, &human).unwrap();
        let r = name_threads_with_labels(source, true, Some(&reference)).unwrap();
        let target = r
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == generated)
            .unwrap();
        assert!(!target.applied);
        assert!(target.reason.as_deref().unwrap().contains("conflicting"));
    }
}

#[test]
fn resource_management_declarations_are_not_plain_algorithm_locals() {
    assert!(!named(
        &ENCRYPT.replace("let encoded=", "using encoded="),
        "encryptStringAesGcm"
    ));
    assert!(!named(
        &DECRYPT.replace("let algorithm=", "using algorithm="),
        "decryptPackedAesGcmBlob"
    ));
}

fn descriptor_encryption() -> String {
    ENCRYPT.replace(
        "pending=subtle.generateKey({name:\"AES-GCM\",length:128},true,[\"encrypt\",\"decrypt\"]).then(key=>key);",
        "let service=subtle;let algorithm={name:\"AES-GCM\"};Object.defineProperty(algorithm,\"length\",{writable:true,configurable:true,enumerable:true,value:128});pending=service.generateKey(algorithm,true,[\"encrypt\",\"decrypt\"]).then(key=>key);",
    )
}

#[test]
fn aes_key_length_data_descriptor_proves_full_algorithm_and_alpha_variants() {
    let source = descriptor_encryption();
    for source in [
        source.clone(),
        source
            .replace("$THREAD_1_10", "$THREAD_778_91402_6")
            .replace("algorithm", "keyOptions")
            .replace("service", "keyService")
            .replace("generated", "freshKey")
            .replace("context", "environment"),
        source
            .replace("let service=subtle;", "")
            .replace("pending=service.generateKey", "pending=subtle.generateKey"),
        source.replace(
            "writable:true,configurable:true,enumerable:true,value:128",
            "value:128,enumerable:true,configurable:true,writable:true",
        ),
    ] {
        assert!(named(&source, "encryptStringAesGcm"), "{source}");
    }
}

#[test]
fn aes_descriptor_refuses_attributes_receiver_operation_and_callback_near_misses() {
    refuse_changes(
        &descriptor_encryption(),
        "encryptStringAesGcm",
        &[
            ("writable:true", "writable:false"),
            ("configurable:true", "configurable:false"),
            ("enumerable:true", "enumerable:false"),
            ("value:128", "value:256"),
            ("value:128", "get:()=>128"),
            ("writable:true,", ""),
            (
                "defineProperty(algorithm,\"length\"",
                "defineProperty(other,\"length\"",
            ),
            (
                "defineProperty(algorithm,\"length\"",
                "defineProperty(algorithm,\"size\"",
            ),
            ("Object.defineProperty", "Reflect.defineProperty"),
            ("let service=subtle", "let service=other"),
            ("let service=subtle;", "let service=subtle;service=other;"),
            ("let service=subtle;", "var service=subtle;var service;"),
            (
                "service.generateKey(algorithm,true",
                "service.generateKey(algorithm,false",
            ),
            (".then(key=>key)", ".then(key=>algorithm)"),
            ("return packed.buffer", "return raw"),
            ("function main(){", "function main(Object){"),
        ],
    );
}

// The candidate retains stable captured crypto/subtle identities. Its IV is declared
// without initialization, then byte-framing length and copying derive from the IV backing
// ArrayBuffer. Proof requires the split single-write IV local and its buffer provenance to
// belong to the same complete algorithm.
const SPLIT_IV_WITNESS: &str = r#"function main(){
let $KEY_443; $KEY_443=globalThis;
let $KEY_403; $KEY_403=$KEY_443.crypto?.subtle;
function $THREAD_0_82587($BR) {
			let $KEY_1 = new TextEncoder().encode($BR);
			let $BS;
			if ($KEY_403 != null) {
				let $BT = $KEY_403;
				let $BU = { name: "AES-GCM" };
				Object.defineProperty($BU, "length", {
					writable: true,
					configurable: true,
					enumerable: true,
					value: 128
				});
				$BS = $BT.generateKey($BU, true, ["encrypt", "decrypt"]).then($BV => $BV);
			} else {
				$BS = null;
			}
			if ($BS != null) {
				return $BS.then($BV => {
					let $KEY_0 = $BV;
					let $KEY_2;
					$KEY_2 = $KEY_443.crypto?.getRandomValues(new Uint8Array(12));
					return $KEY_403.encrypt({
						name: "AES-GCM",
						iv: $KEY_2
					}, $KEY_0, $KEY_1).then($BW => {
						let $KEY_1 = $BW;
						return $KEY_403.exportKey("raw", $KEY_0).then($BX => {
							let $KEY_6 = $KEY_1;
							let $KEY_3 = $KEY_2.buffer;
							let $KEY_0 = new Uint8Array($KEY_6.byteLength + $BX.byteLength + $KEY_3.byteLength);
							$KEY_0.set(new Uint8Array($BX), 0);
							$KEY_0.set(new Uint8Array($KEY_3), $BX.byteLength);
							$KEY_0.set(new Uint8Array($KEY_6), $BX.byteLength + $KEY_3.byteLength);
							return $KEY_0.buffer;
						});
					});
				});
			} else {
				return null;
			}
		}
return $THREAD_0_82587;}main();"#;

fn split_iv_buffer_encryption() -> String {
    descriptor_encryption()
        .replace(
            "let iv=context.crypto?.getRandomValues(new Uint8Array(12));",
            "let iv;iv=context.crypto?.getRandomValues(new Uint8Array(12));",
        )
        .replace("let nonce=iv;", "let nonce=iv.buffer;")
}

#[test]
fn recovered_split_iv_candidate_proves_the_iv_backing_buffer_framing() {
    assert!(
        named(SPLIT_IV_WITNESS, "encryptStringAesGcm"),
        "{SPLIT_IV_WITNESS}"
    );
    let r = name_threads(SPLIT_IV_WITNESS, true).unwrap();
    let claims = r
        .report
        .outcomes
        .iter()
        .filter(|o| {
            o.applied
                && o.evaluator.as_deref() == Some("shared_packing_algorithm")
                && o.proposed_name.as_deref() == Some("encryptStringAesGcm")
        })
        .count();
    assert_eq!(claims, 1);
    let mut replay = SPLIT_IV_WITNESS.to_string();
    for edit in r.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, r.source);
}

#[test]
fn split_iv_buffer_descriptor_proves_alpha_spelling_and_service_variants() {
    for source in [
        split_iv_buffer_encryption(),
        split_iv_buffer_encryption()
            .replace("$THREAD_1_10", "$THREAD_778_91402_6")
            .replace("algorithm", "keyOptions")
            .replace("service", "keyService")
            .replace("generated", "freshKey")
            .replace("context", "environment"),
        split_iv_buffer_encryption()
            .replace("let service=subtle;", "")
            .replace("pending=service.generateKey", "pending=subtle.generateKey"),
        split_iv_buffer_encryption().replace(
            "writable:true,configurable:true,enumerable:true,value:128",
            "value:128,enumerable:true,configurable:true,writable:true",
        ),
    ] {
        assert!(named(&source, "encryptStringAesGcm"), "{source}");
    }
}

#[test]
fn split_iv_buffer_variant_refuses_foreign_ivs_and_wrong_framing() {
    refuse_changes(
        &split_iv_buffer_encryption(),
        "encryptStringAesGcm",
        &[
            // The framing buffer must be the exact IV used by the encrypt config.
            ("let nonce=iv.buffer;", "let nonce=other.buffer;"),
            ("iv:iv}", "iv:key}"),
            // ... and it must be the backing buffer, not another view/derivation.
            ("let nonce=iv.buffer;", "let nonce=iv.slice(0);"),
            ("let nonce=iv.buffer;", "let nonce=iv.byteLength;"),
            // The length term and the packed bytes must both use that buffer.
            (
                "data.byteLength+raw.byteLength+nonce.byteLength",
                "data.byteLength+raw.byteLength+iv.byteLength",
            ),
            ("new Uint8Array(nonce)", "nonce"),
            // The recovered split declaration is only a complete single-write form.
            (
                "let iv;iv=context.crypto?.getRandomValues(new Uint8Array(12));",
                "let iv;iv=context.crypto?.getRandomValues(new Uint8Array(12));iv=other;",
            ),
            // The IV still comes from the same captured crypto context.
            (
                "iv=context.crypto?.getRandomValues",
                "iv=other.crypto?.getRandomValues",
            ),
        ],
    );
}

#[test]
fn recovered_split_iv_candidate_refuses_foreign_iv_and_framing_near_misses() {
    refuse_changes(
        SPLIT_IV_WITNESS,
        "encryptStringAesGcm",
        &[
            ("let $KEY_3 = $KEY_2.buffer;", "let $KEY_3 = $KEY_1.buffer;"),
            ("iv: $KEY_2", "iv: $KEY_0"),
            (
                "let $KEY_3 = $KEY_2.buffer;",
                "let $KEY_3 = $KEY_2.slice(0);",
            ),
            (
                "$KEY_6.byteLength + $BX.byteLength + $KEY_3.byteLength",
                "$KEY_6.byteLength + $BX.byteLength + $KEY_2.byteLength",
            ),
            ("new Uint8Array($KEY_3)", "$KEY_3"),
            ("$KEY_2 = $KEY_443.crypto?", "$KEY_2 = $KEY_403.crypto?"),
        ],
    );
}
