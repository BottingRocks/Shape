use shape::naming::name_threads;
const BASE: &str = r#"function main(){
let context={localStorage:{}},report=function(value){return value;},transform=function(value){return value;},state={};
function $THREAD_1_30(){
let stored;try{stored=context.localStorage.getItem("opaque");}catch(e){report({message:"IM9",stack:""+e});return void 0;}
if(stored!=null){let record;try{record=JSON.parse(stored);}catch(e2){let diagnostic={message:"IM10",stack:void 0};diagnostic.stack=e2+"";report(diagnostic);return void 0;}
if(record.a){transform(record.a).then(value=>{state.a=value;if(record.b){state.b=record.b;}}).catch(error=>{return report({message:"IM8",stack:""+error});});}}
}
return $THREAD_1_30;}main();"#;
fn named(source: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| {
            o.binding.name == "$THREAD_1_30"
                && o.applied
                && o.proposed_name.as_deref() == Some("restoreAggregateEventState")
        })
}
#[test]
fn storage_positive_and_decisive_near_misses() {
    assert!(named(BASE));
    assert!(named(
        &BASE
            .replace("context", "ambient")
            .replace("opaque", "different-key")
            .replace("record", "entry")
    ));
    for (from, to) in [
        ("if(record.a)", "if(record.b)"),
        ("state.a=value", "state.b=value"),
        ("state.b=record.b", "state.b=record.a"),
        ("transform(record.a)", "transform(record.b)"),
        (
            "state.a=value;if(record.b)",
            "state.a=value;sideEffect();if(record.b)",
        ),
        ("JSON.parse(stored)", "JSON.stringify(stored)"),
        ("record.b;", "record.a;"),
        (
            "return $THREAD_1_30;",
            "transform=other;return $THREAD_1_30;",
        ),
        ("function main(){", "function main(JSON){"),
    ] {
        assert!(BASE.contains(from));
        assert!(!named(&BASE.replace(from, to)), "{from} -> {to}");
    }
    let report = name_threads(BASE, true).unwrap();
    let mut replay = BASE.to_string();
    for edit in report.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, report.source);
}

const HISTORY: &str = r#"function main(){let CAP_EVENTS={},CAP_CLOCK=Date,CAP_START=0,CAP_ORIGIN=0,CAP_PARSE=function(value){return value;},CAP_HOST="",CAP_PROTOCOL="",CAP_PORT="",CAP_PREVIOUS=0,CAP_STATE={},CAP_REFRESH=function(value){return value;},CAP_TRANSFORM=function(value){return value;},CAP_CONTEXT={},CAP_REPORT=function(value){return value;};function $THREAD_1_50(eventType,url){
let history=CAP_EVENTS.et;
let elapsed=Number(new CAP_CLOCK())-CAP_START;
let hours=Number((Number(new CAP_CLOCK())-CAP_ORIGIN)/1e3/60/60);
CAP_EVENTS.lit=eventType;CAP_EVENTS.tlr=elapsed;
let protocol="";let hostname="";let port="";let path="";
if(url){let parsed=CAP_PARSE(url);
hostname=parsed.hostname===CAP_HOST?"":parsed.hostname;
protocol=parsed.protocol===CAP_PROTOCOL?"":parsed.protocol;
port=parsed.port===CAP_PORT?"":parsed.port;
let length=parsed.pathname.length;
path=(length<70?parsed.pathname:parsed.pathname.substring(0,65)+"-"+length+"-"+parsed.pathname.substring(length-7))||"";
}
let entry={p1:protocol,p2:hostname,p3:port,p4:path,tc:path?1:0,ty:void 0,lst:void 0};
let types=[eventType];entry.ty=types;let times=[elapsed];entry.lst=times;
let index=history.findIndex(item=>item.p1.length===0&&item.p2.length===0&&item.p3.length===0&&item.p4===path||item.p1===protocol&&item.p2===hostname&&item.p3===port&&item.p4===path);
if(index===-1){history.push(entry);}else{history[index].ty.push(eventType);history[index].lst.push(elapsed);
if(path){history[index].tc=Number(history[index].tc)+1;}
if(history[index].ty.length>10){history[index].ty.shift();history[index].lst.shift();}
}
if(history.length>1){history.shift();}CAP_EVENTS.et=history;
if(path){CAP_EVENTS.c=Number(CAP_EVENTS.c)+1;}
CAP_EVENTS.top=hours;CAP_EVENTS.lvTop=Number((Number(new CAP_CLOCK())-CAP_PREVIOUS)/1e3/60/60);
CAP_PREVIOUS=Number(new CAP_CLOCK());
if(path){CAP_STATE.a.c=Number(CAP_STATE.a.c)+1;}
CAP_STATE.a.ts=CAP_STATE.a.ts+CAP_EVENTS.lvTop;CAP_STATE.b=CAP_STATE.a.c;
CAP_REFRESH();let pending=CAP_TRANSFORM(JSON.stringify(CAP_STATE.a));
if(pending!=null){pending.then(value=>new Uint8Array(value).toString()).then(value=>{return CAP_CONTEXT.localStorage.setItem("STORAGE_KEY",JSON.stringify({a:value,b:CAP_STATE.a.c}));}).catch(error=>{return CAP_REPORT({message:"IM6",stack:""+error});});}
}return $THREAD_1_50;}main();"#;
#[test]
fn history_complete_order() {
    let prove = |s: &str| {
        name_threads(s, true)
            .unwrap()
            .report
            .outcomes
            .iter()
            .any(|o| {
                o.binding.name == "$THREAD_1_50"
                    && o.applied
                    && o.proposed_name.as_deref() == Some("recordAndPersistBoundedEventHistory")
            })
    };
    assert!(prove(HISTORY));
    for (from, to) in [
        ("length>10", "length>11"),
        ("if(history.length>1)", "if(history.length>2)"),
        ("substring(0,65)", "substring(0,64)"),
        ("history[index].lst.shift();", "history[index].ty.shift();"),
        ("CAP_STATE.b=CAP_STATE.a.c", "CAP_STATE.b=CAP_STATE.a.ts"),
        ("CAP_REFRESH();let pending", "let pending"),
    ] {
        assert!(HISTORY.contains(from));
        assert!(!prove(&HISTORY.replace(from, to)), "{from} -> {to}");
    }
}

const MAP: &str = r#"function main(){let context={localStorage:{}},report=function(v){return v;},transform=function(v){return v;},encoded={},decoded={};
function $THREAD_1_31(){let stored;try{stored=context.localStorage.getItem("opaque");}catch(e){report({message:"IM3",stack:""+e});return void 0;}
let pending=stored;if(pending!=null){try{encoded=JSON.parse(stored);}catch(e2){report({message:"IM10",stack:""+e2});return void 0;}
decoded=Object.create(null);let keys=Object.keys(encoded);pending=keys.map(key=>transform(encoded[key]));
Promise.allSettled(pending).then(results=>{return results.map((item,index)=>{if(item.status==="fulfilled"){decoded[keys[index]]=item.value;}else{report({message:"IM4",stack:""});}});}).catch(error=>{return report({message:"IM4",stack:""+error});});}}
return $THREAD_1_31;}main();"#;
const SAVE: &str = r#"function main(){let context={localStorage:{}},report=function(v){return v;},transform=function(v){return v;},encoded={},events={},index="i";
function $THREAD_1_32(){let record=Object.create(null);let stored;try{stored=context.localStorage.getItem("opaque");}catch(e){report({message:"IM5",stack:""+e});return void 0;}
if(stored!=null){try{record=JSON.parse(stored);}catch(e2){let diagnostic={message:"IM10",stack:void 0};diagnostic.stack=e2+"";report(diagnostic);return void 0;}if(Object.keys(record).length>0){encoded=record;}}
if(Object.keys(events).length!==0){let pending=transform(JSON.stringify(events));if(pending!=null){pending.then(value=>{record[index]=new Uint8Array(value).toString();encoded=record;encoded=Object.fromEntries(Object.entries(encoded).slice(-1));context.localStorage.setItem("opaque",JSON.stringify(encoded));}).catch(error=>{return report({message:"IM2",stack:""+error});});}}}
return $THREAD_1_32;}main();"#;

// Synthetic capture harness for the two independently inspected owned
// bodies. Full original-seed verification is recorded separately by the CLI.
const REUSED_SAVE: &str = r#"function main(){let context={localStorage:{}},report=function(v){return v;},transform=function(v){return v;},encoded={},events={},index="i";
function $THREAD_1_33(){let record=Object.create(null);let stored;try{stored=context.localStorage.getItem("opaque");}catch(e){report({message:"IM5",stack:""+e});return void 0;}
let shared=stored;if(shared!=null){try{record=JSON.parse(stored);}catch(e2){report({message:"IM10",stack:e2+""});return void 0;}if(Object.keys(record).length>0){encoded=record;}}
if(Object.keys(events).length!==0){shared=JSON.stringify(events);let pending=transform(shared);if(pending!=null){shared=14671;pending.then(value=>{record[index]=new Uint8Array(value).toString();encoded=record;encoded=Object.fromEntries(Object.entries(encoded).slice(-1));context.localStorage.setItem("opaque",JSON.stringify(encoded));}).catch(error=>{return report({message:"IM2",stack:""+error});});}}}
return $THREAD_1_33;}main();"#;

#[test]
fn reused_serialization_slot_complete_protocol() {
    let prove = |source: &str| {
        let output = name_threads(source, true).unwrap();
        let mut replay = source.to_string();
        for edit in output.report.edits.iter().rev() {
            replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
        }
        assert_eq!(replay, output.source);
        output.report.outcomes.iter().any(|o| {
            o.binding.name == "$THREAD_1_33"
                && o.applied
                && o.proposed_name.as_deref() == Some("saveLatestSerializedEventMap")
        })
    };
    assert!(prove(REUSED_SAVE));
    let staged = REUSED_SAVE.replace("shared=14671", "shared=4974").replace(
        "record[index]=new Uint8Array(value).toString();",
        "let bytes=new Uint8Array(value).toString();record[index]=bytes;",
    );
    assert!(prove(&staged));
    assert!(prove(
        &staged
            .replace("shared", "scratch")
            .replace("opaque", "other-key")
    ));
    for (from, to) in [
        ("shared=14671", "shared=sideEffect()"),
        ("shared=14671", "encoded=14671"),
        ("shared=14671", "shared+=14671"),
        ("shared=14671", "shared=14671;report(shared)"),
        ("new Uint8Array(value)", "new Uint8Array(shared)"),
        ("transform(shared)", "transform(stored)"),
        ("slice(-1)", "slice(-2)"),
        ("setItem(\"opaque\"", "setItem(\"other\""),
        ("JSON.stringify(events)", "JSON.stringify(record)"),
        ("let shared=stored", "const shared=stored"),
        ("let shared=stored", "shared=stored"),
        ("function main(){", "function main(JSON){"),
        (
            "return $THREAD_1_33;",
            "transform=other;return $THREAD_1_33;",
        ),
    ] {
        assert!(REUSED_SAVE.contains(from));
        assert!(!prove(&REUSED_SAVE.replace(from, to)), "{from} -> {to}");
    }
    assert!(!prove(
        &staged.replace("record[index]=bytes", "record[index]=shared")
    ));
}
#[test]
fn map_and_save_protocols() {
    let prove = |source: &str, target: &str, role: &str| {
        name_threads(source, true)
            .unwrap()
            .report
            .outcomes
            .iter()
            .any(|o| {
                o.binding.name == target && o.applied && o.proposed_name.as_deref() == Some(role)
            })
    };
    assert!(prove(MAP, "$THREAD_1_31", "restoreSerializedEventMap"));
    assert!(prove(SAVE, "$THREAD_1_32", "saveLatestSerializedEventMap"));
    for (from, to) in [
        ("item.status===\"fulfilled\"", "item.status===\"rejected\""),
        (
            "decoded[keys[index]]=item.value",
            "decoded[keys[index]]=item.reason",
        ),
        ("encoded[key]", "encoded[index]"),
        ("Promise.allSettled", "Promise.all"),
    ] {
        assert!(MAP.contains(from));
        assert!(
            !prove(
                &MAP.replace(from, to),
                "$THREAD_1_31",
                "restoreSerializedEventMap"
            ),
            "{from} -> {to}"
        );
    }
    for (from, to) in [
        ("slice(-1)", "slice(-2)"),
        ("setItem(\"opaque\"", "setItem(\"other-key\""),
        (
            "record[index]=new Uint8Array(value)",
            "record[index]=new Uint8Array(stored)",
        ),
        ("JSON.stringify(events)", "JSON.stringify(record)"),
        (
            "encoded=record;encoded=Object.fromEntries",
            "encoded=Object.fromEntries",
        ),
    ] {
        assert!(SAVE.contains(from));
        assert!(
            !prove(
                &SAVE.replace(from, to),
                "$THREAD_1_32",
                "saveLatestSerializedEventMap"
            ),
            "{from} -> {to}"
        );
    }
}
