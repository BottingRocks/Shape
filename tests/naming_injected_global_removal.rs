use shape::naming::name_threads;

const ROLE: &str = "removeInjectedGlobalAndNotifyCleanup";

fn named(source: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some(ROLE)
                && outcome.evaluator.as_deref() == Some("browser_injected_global_removal")
        })
}

// Portable reproduction of the owned body with descriptive identities. The
// consumer, applier, method, receiver and children are opaque locals; `window`
// is only the unshadowed global object.
const TEARDOWN: &str = r#"function main(){
let injectionKey="probe-key";
let cleanupReceiver={remove(){}};
let cleanupMethod=cleanupReceiver.remove;
let firstChild={id:"first"};
let secondChild={id:"second"};
let notificationSink=function(value){void value;};
let applyToReceiver=function(method,receiver,child){return method.call(receiver,child);};
function $THREAD_2_128173($BW){
delete window[injectionKey];
try{notificationSink($BW);}catch(e){}
applyToReceiver(cleanupMethod,cleanupReceiver,firstChild);
applyToReceiver(cleanupMethod,cleanupReceiver,secondChild);
}
return $THREAD_2_128173;
}
main();"#;

// The exact seed body and generated capture spellings, with only its
// surrounding declarations made portable.
const SEED_BODY: &str = r#"function main(){
let $KEY_1="probe-key";
let $KEY_3={id:"style"};
let $KEY_5={removeChild(){}};
let $KEY_9={id:"iframe"};
let $KEY_34=$KEY_5.removeChild;
let $KEY_0=function($BW){void $BW;};
let $KEY_68=function($BT,$BU,$BV){return $BT.call($BU,$BV);};
function $THREAD_2_128173($BW){
delete window[$KEY_1];
try{
$KEY_0($BW);
}catch(e){}
$KEY_68($KEY_34,$KEY_5,$KEY_9);
$KEY_68($KEY_34,$KEY_5,$KEY_3);
}
return $THREAD_2_128173;
}
main();"#;

#[test]
fn teardown_body_accepts_portable_alpha_and_seed_spellings() {
    assert!(named(TEARDOWN));
    let alpha = TEARDOWN
        .replace("$THREAD_2_128173", "$THREAD_77_999999")
        .replace("injectionKey", "retireKey")
        .replace("cleanupReceiver", "target")
        .replace("cleanupMethod", "erase")
        .replace("firstChild", "leftChild")
        .replace("secondChild", "rightChild")
        .replace("notificationSink", "notify")
        .replace("applyToReceiver", "invoke")
        .replace("$BW", "$BX");
    assert!(named(&alpha));
    assert!(named(SEED_BODY));
}

#[test]
fn deletion_catch_and_finalizer_shapes_are_required() {
    for (from, to) in [
        (
            "delete window[injectionKey];",
            "delete document[injectionKey];",
        ),
        ("delete window[injectionKey];", "delete window[0];"),
        ("delete window[injectionKey];", "void window[injectionKey];"),
        (
            "try{notificationSink($BW);}catch(e){}",
            "try{notificationSink(0);}catch(e){}",
        ),
        (
            "try{notificationSink($BW);}catch(e){}",
            "try{notificationSink($BW,$BW);}catch(e){}",
        ),
        (
            "try{notificationSink($BW);}catch(e){}",
            "try{notificationSink($BW);}catch(e){notificationSink(e);}",
        ),
        (
            "try{notificationSink($BW);}catch(e){}",
            "try{notificationSink($BW);}catch(e){}finally{notificationSink(0);}",
        ),
        (
            "try{notificationSink($BW);}catch(e){}",
            "try{notificationSink($BW);}catch({e}){}",
        ),
        (
            "try{notificationSink($BW);}catch(e){}",
            "try{notificationSink($BW);}catch{}",
        ),
        (
            "try{notificationSink($BW);}catch(e){}",
            "try{notificationSink($BW);notificationSink(e);}catch(e){}",
        ),
    ] {
        let source = TEARDOWN.replace(from, to);
        assert!(!named(&source), "unexpected claim for {from} -> {to}");
    }
}

#[test]
fn application_arguments_identity_and_order_are_required() {
    for (from, to) in [
        (
            "try{notificationSink($BW);}catch(e){}\napplyToReceiver(cleanupMethod,cleanupReceiver,firstChild);",
            "applyToReceiver(cleanupMethod,cleanupReceiver,firstChild);\ntry{notificationSink($BW);}catch(e){}",
        ),
        (
            "applyToReceiver(cleanupMethod,cleanupReceiver,secondChild);",
            "applyToReceiver(firstChild,cleanupReceiver,secondChild);",
        ),
        (
            "applyToReceiver(cleanupMethod,cleanupReceiver,secondChild);",
            "applyToReceiver(cleanupMethod,firstChild,secondChild);",
        ),
        (
            "applyToReceiver(cleanupMethod,cleanupReceiver,secondChild);",
            "applyToReceiver(cleanupMethod,cleanupReceiver,firstChild);",
        ),
        (
            "applyToReceiver(cleanupMethod,cleanupReceiver,secondChild);",
            "applyToReceiver(cleanupMethod,cleanupReceiver,secondChild,firstChild);",
        ),
        (
            "applyToReceiver(cleanupMethod,cleanupReceiver,secondChild);",
            "applyToReceiver(cleanupReceiver,cleanupReceiver,secondChild);",
        ),
        (
            "applyToReceiver(cleanupMethod,cleanupReceiver,secondChild);",
            "window(cleanupMethod,cleanupReceiver,secondChild);",
        ),
        (
            "applyToReceiver(cleanupMethod,cleanupReceiver,firstChild);",
            "applyToReceiver?.(cleanupMethod,cleanupReceiver,firstChild);",
        ),
        (
            "try{notificationSink($BW);}catch(e){}",
            "try{applyToReceiver($BW);}catch(e){}",
        ),
    ] {
        let source = TEARDOWN.replace(from, to);
        assert!(!named(&source), "unexpected claim for {from} -> {to}");
    }
}

#[test]
fn capture_origin_and_plain_formals_are_required() {
    for (from, to) in [
        (
            "let cleanupMethod=cleanupReceiver.remove;",
            "let [cleanupMethod]=[cleanupReceiver.remove];",
        ),
        (
            "let cleanupMethod=cleanupReceiver.remove;",
            "function cleanupMethod(){}",
        ),
        ("function main(){", "function main(){let window={};"),
        (
            "function $THREAD_2_128173($BW){",
            "function $THREAD_2_128173($BW,$BX){",
        ),
        (
            "function $THREAD_2_128173($BW){",
            "function $THREAD_2_128173($BW=0){",
        ),
        (
            "function $THREAD_2_128173($BW){",
            "async function $THREAD_2_128173($BW){",
        ),
        (
            "delete window[injectionKey];",
            "delete window[cleanupMethod];",
        ),
    ] {
        let source = TEARDOWN.replace(from, to);
        if source == TEARDOWN {
            continue;
        }
        assert!(!named(&source), "unexpected claim for {from} -> {to}");
    }
}

#[test]
fn extra_statements_and_missing_calls_are_refused() {
    for (from, to) in [
        (
            "applyToReceiver(cleanupMethod,cleanupReceiver,secondChild);\n}",
            "applyToReceiver(cleanupMethod,cleanupReceiver,secondChild);\nnotificationSink(0);\n}",
        ),
        (
            "applyToReceiver(cleanupMethod,cleanupReceiver,secondChild);\n}",
            "}",
        ),
        (
            "delete window[injectionKey];\n",
            "delete window[injectionKey];\nnotificationSink(0);\n",
        ),
    ] {
        let source = TEARDOWN.replace(from, to);
        assert!(!named(&source), "unexpected claim for {from} -> {to}");
    }
}

#[test]
fn existing_role_names_and_collisions_are_protected() {
    let human = TEARDOWN.replace("$THREAD_2_128173", ROLE);
    assert_eq!(name_threads(&human, true).unwrap().source, human);

    let collision = TEARDOWN.replace(
        "function $THREAD_2_128173",
        "let removeInjectedGlobalAndNotifyCleanup=0;function $THREAD_2_128173",
    );
    assert!(!named(&collision));
}
