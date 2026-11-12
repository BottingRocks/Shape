// Retain all original whole-body packing controls in this focused target.
include!("naming_thread_shared_packing.rs");

const PREPARER: &str = "prepareTextAndGenerateAesGcmKey";
fn guarded(source: &str) -> String {
    source.replace("let iv=context.crypto?.getRandomValues(new Uint8Array(12));",
        "let iv;if(context.crypto==null){iv=void 0;}else{iv=context.crypto.getRandomValues(new Uint8Array(12));}")
}
fn descriptor(source: &str, alias: bool) -> String {
    source.replace(r#"pending=subtle.generateKey({name:"AES-GCM",length:128}"#,
        &format!(r#"{}let algorithm={{name:"AES-GCM"}};Object.defineProperty(algorithm,"length",{{writable:true,configurable:true,enumerable:true,value:128}});pending={}.generateKey(algorithm"#,
            if alias { "let service=subtle;" } else { "" }, if alias { "service" } else { "subtle" }))
}
fn assert_role(source: &str, role: &str) {
    let r=name_threads(source,true).unwrap();
    let o=r.report.outcomes.iter().find(|o|o.applied && o.role_name.as_deref()==Some(role)).expect(source);
    assert_eq!(o.proposed_name.as_deref(),Some(role));
    let mut replay=source.to_string();
    for e in r.report.edits.iter().rev(){ replay.replace_range(e.start as usize..e.end as usize,&e.replacement); }
    assert_eq!(replay,r.source);
}
#[test]
fn original_packing_wins_then_closed_preparer_then_new_packing() {
    for source in [ENCRYPT.to_string(),descriptor(ENCRYPT,true),descriptor(ENCRYPT,false)] {
        assert_role(&source,"encryptStringAesGcm");
    }
    for source in [guarded(ENCRYPT),guarded(&descriptor(ENCRYPT,true))] {
        assert_role(&source,PREPARER);
        assert!(!named(&source,"encryptStringAesGcm"));
        assert_role(&source.replace("input","plaintext").replace("encoded","textBytes").replace("generated","newKey"),PREPARER);
    }
    let fresh=guarded(&descriptor(ENCRYPT,false));
    assert_role(&fresh,"encryptStringAesGcm");
    assert_role(&fresh.replace("input","plaintext").replace("packed","framedBytes"),"encryptStringAesGcm");
    assert!(!named(&fresh,PREPARER));
}
#[test]
fn legacy_preparer_does_not_borrow_continuation_evidence() {
    let source=guarded(&descriptor(ENCRYPT,true));
    // Continuation internals are deliberately opaque to the accepted preparer.
    for (from,to) in [("iv:iv","iv:encoded"),("return packed.buffer","return packed"),("let key=generated;","let key=generated;effect();")] {
        assert_role(&source.replace(from,to),PREPARER);
        assert!(!named(&source.replace(from,to),"encryptStringAesGcm"));
    }
}
#[test]
fn new_packing_requires_complete_crypto_identity_initialization_effect_order_and_return() {
    let source=guarded(&descriptor(ENCRYPT,false));
    refuse_changes(&source,"encryptStringAesGcm",&[
        ("subtle.encrypt(","foreign.encrypt("),
        ("subtle.exportKey(","foreign.exportKey("),
        ("let algorithm=", "let service=foreign;let algorithm="),
        ("pending=subtle.generateKey(algorithm", "pending=foreign.generateKey(algorithm"),
        ("let subtle=context.crypto?.subtle;", "let subtle=foreign.crypto?.subtle;"),
        ("let subtle=context.crypto?.subtle;", "let subtle=context.crypto?.subtle;subtle=foreign;"),
        ("let iv;if", "let iv=foreign;if"),
        ("iv=void 0;", "iv=void 0;effect();"),
        ("return packed.buffer", "return packed"),
        ("Uint8Array(raw),0", "Uint8Array(nonce),0"),
        ("let key=generated;", "effect();let key=generated;"),
        ("return subtle.encrypt", "subtle.encrypt"),
    ]);
    // Altering the closed parent contract must not synthesize a preparer claim.
    let old=guarded(&descriptor(ENCRYPT,true));
    refuse_changes(&old,PREPARER,&[
        ("pending=service.generateKey", "pending=foreign.generateKey"),
        ("let service=subtle;", "let service=foreign;"),
        (".then(key=>key)", ".then(key=>null)"),
        ("return pending.then", "pending.then"),
        ("return null", "return void 0"),
        ("new TextEncoder().encode(input)", "new TextEncoder().encode(foreign)"),
        ("let encoded=", "effect();let encoded="),
        ("let subtle=context.crypto?.subtle;", "let subtle;if(flag){subtle=context.crypto?.subtle;}"),
    ]);
}
