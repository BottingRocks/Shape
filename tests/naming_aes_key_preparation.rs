use shape::naming::name_threads;
const NAME: &str = "prepareTextAndGenerateAesGcmKey";
const BASE: &str = r#"function main(){let rootState=0;let context=globalThis;let subtle=context.crypto?.subtle;
function $THREAD_1_10(input){let encoded=new TextEncoder().encode(input);let pending;
if(subtle!=null){pending=subtle.generateKey({name:"AES-GCM",length:128},true,["encrypt","decrypt"]).then(key=>key);}else{pending=null;}
if(pending!=null){return pending.then(key=>{return [encoded,key];});}else{return null;}}
return $THREAD_1_10;}main();"#;
fn named(source: &str) -> bool {
    name_threads(source, true)
        .unwrap_or_else(|e| panic!("{e}: {source}"))
        .report
        .outcomes
        .iter()
        .any(|o| o.applied && o.role_name.as_deref() == Some(NAME))
}
fn descriptor() -> String {
    BASE.replace("pending=subtle.generateKey({name:\"AES-GCM\",length:128}",
        "let service=subtle;let options={name:\"AES-GCM\"};Object.defineProperty(options,\"length\",{writable:true,configurable:true,enumerable:true,value:128});pending=service.generateKey(options")
}
#[test]
fn exact_owned_variants_and_replay() {
    for source in [BASE.to_owned(), descriptor(), BASE.replace("subtle", "engine").replace("crypto?.engine", "crypto?.subtle"),
        BASE.replace("let subtle=context.crypto?.subtle;", "let subtle;subtle=context.crypto?.subtle;"),
        BASE.replace("key=>{return [encoded,key];}", "key=>{throw 7;}"),
        BASE.replace("globalThis", "typeof global !== \"undefined\" && global != null && global.global === global ? global : typeof self !== \"undefined\" && self != null && self.self === self ? self : typeof window !== \"undefined\" && window != null && window.window === window ? window : void 0")
    ] {
        assert!(named(&source), "{source}");
        let result = name_threads(&source, true).unwrap();
        assert!(!result.report.outcomes.iter().any(|o| o.applied && o.role_name.as_deref() == Some("encryptStringAesGcm")));
        let mut replay = source;
        for edit in result.report.edits.iter().rev() { replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement); }
        assert_eq!(replay, result.source);
    }
}
#[test]
fn decisive_owned_and_origin_rejections() {
    for (from, to) in [
        ("length:128", "length:256"),
        ("AES-GCM", "AES-CBC"),
        (
            "true,[\"encrypt\",\"decrypt\"]",
            "false,[\"encrypt\",\"decrypt\"]",
        ),
        ("[\"encrypt\",\"decrypt\"]", "[\"encrypt\"]"),
        (".encode(input)", ".encode(other)"),
        (".then(key=>key)", ".then(key=>null)"),
        ("return null", "return void 0"),
        ("if(subtle!=null)", "if(other!=null)"),
        ("let context=globalThis", "let context={crypto:{subtle:{}}}"),
        ("function main(){", "function main(TextEncoder){"),
        ("function main(){", "function main(globalThis){"),
        ("return $THREAD_1_10;", "subtle=other;return $THREAD_1_10;"),
        ("return $THREAD_1_10;", "context=other;return $THREAD_1_10;"),
        (
            "let subtle=context.crypto?.subtle;",
            "let subtle;$THREAD_1_10('early');subtle=context.crypto?.subtle;",
        ),
        (
            "let subtle=context.crypto?.subtle;",
            "let subtle;let early=$THREAD_1_10;early('early');subtle=context.crypto?.subtle;",
        ),
        (
            "let subtle=context.crypto?.subtle;",
            "let subtle;escape($THREAD_1_10);subtle=context.crypto?.subtle;",
        ),
        (
            "let subtle=context.crypto?.subtle;",
            "let subtle;if(flag){subtle=context.crypto?.subtle;}",
        ),
        ("subtle.generateKey", "sibling.generateKey"),
    ] {
        assert!(BASE.contains(from));
        assert!(!named(&BASE.replace(from, to)), "accepted {from} -> {to}");
    }
}
#[test]
fn descriptors_require_exact_flags_same_object_and_native_object() {
    let source = descriptor();
    assert!(named(&source));
    for (from, to) in [
        ("writable:true", "writable:false"),
        ("configurable:true", "configurable:false"),
        ("enumerable:true", "enumerable:false"),
        ("value:128", "value:256"),
        ("defineProperty(options", "defineProperty(other"),
        ("generateKey(options", "generateKey(other"),
        ("let service=subtle;", "let service=other;"),
        ("function main(){", "function main(Object){"),
        (
            "pending=service.generateKey",
            "service=other;pending=service.generateKey",
        ),
    ] {
        assert!(source.contains(from));
        assert!(!named(&source.replace(from, to)), "accepted {from} -> {to}");
    }
}
#[test]
fn nested_or_sibling_donors_do_not_prove_parent() {
    let unrelated = BASE.replace("new TextEncoder().encode(input)", "input");
    assert!(!named(&unrelated.replace(
        "return [encoded,key];",
        "return new TextEncoder().encode(input);"
    )));
    assert!(!named(&BASE.replace("new TextEncoder().encode(input)", "new Other().encode(input)").replace("return $THREAD_1_10;", "function sibling(input){return new TextEncoder().encode(input);}return $THREAD_1_10;")));
}
