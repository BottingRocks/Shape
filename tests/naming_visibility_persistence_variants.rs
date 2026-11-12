use shape::naming::name_threads;

// Invented capture harness exercising own-body syntax, not claimed source-backed.
const SOURCE: &str = r#"function main(){
let host=globalThis;
let date=host.Date;
let previous=Number(new date());
let counter={p:"",c:0,tt:"",et:[],lit:0,tlr:0,top:0,lvTop:0};
let record={a:{c:0,ci:[],ts:0},b:0};
function transform(value){return Promise.resolve(value);}
function report(error){return error;}
let encode=transform;
let sink=report;
function $THREAD_0_123(){
if(host.document.visibilityState==="hidden"){
counter.lvTop=Number((Number(new date())-previous)/1000/60/60);
previous=Number(new date());
}else if(host.document.visibilityState==="visible"){
previous=Number(new date());
}
record.a.ts=record.a.ts+counter.lvTop;
let pending=encode(JSON.stringify(record.a));
if(pending!=null){
pending.then(value=>new Uint8Array(value).toString()).then(value=>{
return host.localStorage.setItem("arbitrary-key",JSON.stringify({a:value,b:record.a.c}));
}).catch(error=>{return sink({message:"IM6",stack:""+error});});
}
}
return $THREAD_0_123;
}main();"#;

fn recognized(source: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| {
            o.applied && o.evaluator.as_deref() == Some("owned_visibility_persistence_variants")
        })
}

#[test]
fn visibility_persistence_positive_alpha_and_meaningful_name() {
    assert!(recognized(SOURCE));
    let alpha = SOURCE
        .replace("host", "context")
        .replace("date", "clockConstructor")
        .replace("previous", "lastClock")
        .replace("counter", "elapsedState")
        .replace("record", "snapshot")
        .replace("pending", "result")
        .replace("$THREAD_0_123", "$THREAD_42_8989");
    assert!(recognized(&alpha));
    let human = SOURCE.replace("$THREAD_0_123", "existingPurpose");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
}

#[test]
fn visibility_persistence_rejects_changed_protocol_and_shadowed_origins() {
    for (from, to) in [
        ("===\"hidden\"", "===\"visible\""),
        ("===\"visible\"", "!==\"visible\""),
        ("/1000/60/60", "/1000/60/24"),
        ("Number(new date())-previous", "previous-Number(new date())"),
        ("counter.lvTop=Number", "counter.top=Number"),
        ("record.a.ts=record.a.ts", "record.b=record.a.ts"),
        ("record.a.ts+counter.lvTop", "record.a.c+counter.lvTop"),
        ("JSON.stringify(record.a)", "JSON.stringify(counter)"),
        ("b:record.a.c", "b:record.a.ts"),
        ("a:value,b:", "a:pending,b:"),
        ("pending!=null", "pending==null"),
        ("new Uint8Array(value)", "new Uint8Array(pending)"),
        (".then(value=>new", ".then(other=>new"),
        ("stack:\"\"+error", "stack:\"\"+pending"),
        ("message:\"IM6\"", "message:\"IM7\""),
        ("return sink(", "return encode("),
        ("host.Date", "host.NotDate"),
        (
            "host=globalThis",
            "host={Date:Date,document:document,localStorage:localStorage}",
        ),
        (
            "let host=globalThis;",
            "let globalThis={};let host=globalThis;",
        ),
        (
            "let date=host.Date;",
            "let Number=(v)=>v;let date=host.Date;",
        ),
        (
            "let date=host.Date;",
            "let JSON={stringify(v){return v;}};let date=host.Date;",
        ),
        (
            "let date=host.Date;",
            "let Uint8Array=function(v){return v;};let date=host.Date;",
        ),
        (
            "function $THREAD_0_123(){",
            "function $THREAD_0_123(){let host=globalThis;",
        ),
        ("counter.lvTop=Number", "extra();counter.lvTop=Number"),
        (
            "record.a.ts=record.a.ts+counter.lvTop;",
            "extra();record.a.ts=record.a.ts+counter.lvTop;",
        ),
        (
            "return host.localStorage",
            "extra();return host.localStorage",
        ),
        ("return sink({", "extra();return sink({"),
        (
            "previous=Number(new date());\n}",
            "previous=Number(new host.Date());\n}",
        ),
    ] {
        let negative = SOURCE.replace(from, to);
        assert_ne!(negative, SOURCE, "unused mutation {from}");
        assert!(!recognized(&negative), "accepted mutation {from} -> {to}");
    }
}

#[test]
fn visibility_persistence_rejects_early_exposure_and_effect_order() {
    let late = SOURCE.replace("let date=host.Date;", "let date;").replace(
        "return $THREAD_0_123;",
        "date=host.Date;return $THREAD_0_123;",
    );
    // Initial clock capture precedes Date initialization even when callback exposure is late.
    assert!(!recognized(&late));
    let late_state = SOURCE
        .replace("let counter={p:", "let counter;let unused={p:")
        .replace(
            "return $THREAD_0_123;",
            "$THREAD_0_123();counter={lvTop:0};return $THREAD_0_123;",
        );
    assert!(!recognized(&late_state));
    let swapped=SOURCE.replace("counter.lvTop=Number((Number(new date())-previous)/1000/60/60);\nprevious=Number(new date());",
        "previous=Number(new date());\ncounter.lvTop=Number((Number(new date())-previous)/1000/60/60);");
    assert!(!recognized(&swapped));
}
