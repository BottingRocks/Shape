//! Focused tests for rolling-frontier adapters.
//!
//! Each fixture is a portable, reachable main() wrapper. Generated function identifiers are
//! renamed normally; alpha variants rename only owned locals. Operation, callback, alias,
//! and return changes must refuse the bounded owned-body proofs.
use shape::naming::name_threads;

const DISTINCT: &str = r#"function main(){
function $THREAD_4_8801(record,state){
let copy=record;
if(state.samples.some(stored=>{
let matches;
if(copy==null||stored==null||copy.length!==stored.length){matches=false;}
else{let index=0;for(;;){if(index>=copy.length){matches=true;break;}if(copy[index]===stored[index]){++index;continue;}matches=false;break;}}
return matches;
})){return void 0;}
state.count=Number(state.count)+1;
if(state.samples.length>7){return void 0;}
else{let snapshot=Array.prototype.slice.call(copy);state.samples.push(snapshot);return void 0;}
}
return $THREAD_4_8801;
}main();"#;

const CHUNKED: &str = r#"function main(){
function $THREAD_12_501(table,keys){
let source=table;
return Object.fromEntries(keys.filter(key=>source["p-"+key]!=null).map(key=>{
let index=0;
let chunk=source["p-"+key];
for(;source["p-"+key+index]!=null;index++){chunk=chunk+source["p-"+key+index];}
chunk=["p-"+key,chunk];
return chunk;
}));
}
return $THREAD_12_501;
}main();"#;

fn named(source: &str, expected: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .outcomes
            .iter()
            .any(|o| o.applied && o.proposed_name.as_deref() == Some(expected))
    })
}

fn refuses(source: &str, role: &str) {
    assert!(!named(source, role), "unexpected {role} for {source}");
}

// A negative only counts when the fragment exists, the replacement really
// changes the fixture, and the changed fixture still parses. This prevents a
// typo or an unrelated parse failure from masquerading as a refusal.
fn refuses_change(base: &str, from: &str, to: &str, role: &str) {
    assert!(base.contains(from), "fixture lacks fragment {from:?}");
    let changed = base.replace(from, to);
    assert_ne!(changed, base, "replacement did not change {from:?}");
    assert!(
        name_threads(&changed, false).is_ok(),
        "negative fixture does not parse: {from:?} -> {to:?}"
    );
    refuses(&changed, role);
}

#[test]
fn distinct_vector_sample_generated_and_alpha_variants() {
    for source in [
        DISTINCT.to_string(),
        DISTINCT
            .replace("$THREAD_4_8801", "$THREAD_903_4402_2")
            .replace("record", "input")
            .replace("state", "bucket")
            .replace("copy", "clone")
            .replace("stored", "saved")
            .replace("matches", "equal")
            .replace("index", "cursor")
            .replace("snapshot", "captured")
            .replace("samples", "values")
            .replace("count", "total")
            .replace(">7", ">3"),
        DISTINCT
            .replace("state.samples", "state.items")
            .replace("state.count", "state.seen"),
        DISTINCT.replace("let copy", "const copy"),
    ] {
        assert!(named(&source, "recordDistinctVectorSample"), "{source}");
    }
}

#[test]
fn distinct_vector_sample_operations_callbacks_aliases_and_returns_are_proven() {
    for (from, to) in [
        ("state.samples.some", "state.samples.every"),
        ("copy.length!==stored.length", "copy.length===stored.length"),
        ("copy[index]===stored[index]", "copy[index]==stored[index]"),
        ("index>=copy.length", "index>copy.length"),
        (
            "state.count=Number(state.count)+1",
            "state.count=Number(state.count)",
        ),
        (
            "state.count=Number(state.count)+1",
            "state.count=state.count+1",
        ),
        ("state.samples.length>7", "state.samples.length>=7"),
        ("state.samples.length>7", "state.samples.length>0"),
        ("state.samples.length>7", "state.samples.length<7"),
        (
            "Array.prototype.slice.call(copy)",
            "Array.prototype.slice.apply(copy)",
        ),
        ("state.samples.push(snapshot)", "state.samples.push(copy)"),
        ("state.samples.push(snapshot)", "state.other.push(snapshot)"),
        ("let copy=record;", "let copy=state;"),
        ("let copy=record;", "let copy=record;copy=state;"),
        ("return void 0;", "return;"),
        ("++index", "index--"),
        ("++index", "index++"),
        ("let copy=record;", "let copy=record;let repeat=copy;"),
        ("if(state.samples.some", "if(!state.samples.some"),
        ("})){return void 0;}", "})){return false;}"),
    ] {
        refuses_change(DISTINCT, from, to, "recordDistinctVectorSample");
    }
}

#[test]
fn distinct_vector_sample_rejects_binding_and_parameter_holes() {
    for (from, to) in [
        ("(record,state)", "({record},state)"),
        ("(record,state)", "(record=other,state)"),
        ("(record,state)", "(record,...state)"),
        ("(record,state)", "(state,record)"),
        ("function $THREAD_4_8801", "async function $THREAD_4_8801"),
        ("function $THREAD_4_8801", "function* $THREAD_4_8801"),
        ("let matches;", "let matches;let extra;"),
        ("stored==null", "stored===null"),
        ("index>=copy.length", "index>=stored.length"),
        ("copy[index]===stored[index]", "stored[index]===copy[index]"),
        ("Array.prototype.slice.call(copy)", "slice.call(copy)"),
        ("Number(state.count)", "Int(state.count)"),
    ] {
        refuses_change(DISTINCT, from, to, "recordDistinctVectorSample");
    }
}

#[test]
fn chunked_entries_generated_and_alpha_variants() {
    for source in [
        CHUNKED.to_string(),
        CHUNKED
            .replace("$THREAD_12_501", "$THREAD_71_2209_2")
            .replace("table", "map")
            .replace("keys", "names")
            .replace("source", "data")
            .replace("index", "cursor")
            .replace("chunk", "parts")
            .replace("\"p-\"", "\"q-\""),
        CHUNKED.replace("let source=table;", "const source=table;"),
    ] {
        assert!(named(&source, "reassembleChunkedEntries"), "{source}");
    }
}

#[test]
fn chunked_entries_operations_order_prefix_and_returns_are_proven() {
    for (from, to) in [
        ("Object.fromEntries", "Object.assign"),
        (".filter(", ".map("),
        (".map(key=>{", ".flatMap(key=>{"),
        ("source[\"p-\"+key]!=null", "source[\"p-\"+key]==null"),
        ("source[\"p-\"+key]!=null", "source[\"p-\"+key]"),
        (
            "chunk=chunk+source[\"p-\"+key+index]",
            "chunk=source[\"p-\"+key+index]+chunk",
        ),
        (
            "chunk=chunk+source[\"p-\"+key+index]",
            "chunk=chunk-source[\"p-\"+key+index]",
        ),
        ("chunk=[\"p-\"+key,chunk]", "chunk=[chunk,\"p-\"+key]"),
        ("let index=0;", "let index=1;"),
        ("index++)", "index--)"),
        ("return chunk;", "return [chunk];"),
        ("source[\"p-\"+key];", "source[\"q-\"+key];"),
        ("source[\"p-\"+key+index]", "source[\"p-\"+key+index+0]"),
        ("let source=table;", "let source=table;source=keys;"),
        (
            "let chunk=source[\"p-\"+key];",
            "let chunk=keys[\"p-\"+key];",
        ),
        (
            "{chunk=chunk+source[\"p-\"+key+index];}",
            "{chunk=[chunk];}",
        ),
        (
            "Object.fromEntries(keys.filter",
            "Object.fromEntries(keys.map",
        ),
    ] {
        refuses_change(CHUNKED, from, to, "reassembleChunkedEntries");
    }
}

#[test]
fn chunked_entries_rejects_binding_and_parameter_holes() {
    for (from, to) in [
        ("(table,keys)", "({table},keys)"),
        ("(table,keys)", "(table,keys=other)"),
        ("(table,keys)", "(table,...keys)"),
        ("(table,keys)", "(keys,table)"),
        ("function $THREAD_12_501", "async function $THREAD_12_501"),
        ("function $THREAD_12_501", "function* $THREAD_12_501"),
        ("source[\"p-\"+key]", "source?.[\"p-\"+key]"),
        ("keys.filter(key=>", "keys.filter((key,extra)=>"),
        ("keys.filter(key=>", "keys.filter((key=other)=>"),
    ] {
        refuses_change(CHUNKED, from, to, "reassembleChunkedEntries");
    }
}

#[test]
fn human_names_and_neighbouring_algorithms_are_not_overwritten() {
    let distinct = DISTINCT.replace("$THREAD_4_8801", "recordExistingSample");
    assert_eq!(name_threads(&distinct, true).unwrap().source, distinct);
    let chunked = CHUNKED.replace("$THREAD_12_501", "readExistingChunks");
    assert_eq!(name_threads(&chunked, true).unwrap().source, chunked);

    // A near miss with a different retention rule must not take either role.
    refuses(
        &DISTINCT.replace(
            "state.samples.length>7",
            "state.samples.length>7&&state.count>0",
        ),
        "recordDistinctVectorSample",
    );
    refuses(
        &CHUNKED.replace(".filter(", ".forEach("),
        "reassembleChunkedEntries",
    );
}
