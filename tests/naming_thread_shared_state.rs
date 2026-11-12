use shape::naming::name_threads;

const LOOKUP: &str = r#"function main(){let table={};
function $THREAD_9_123(object,key){
if(table[key]==null){return null;}
let cursor=0;while(cursor<table[key].length){
if(object===table[key][cursor].object){return table[key][cursor];}
else{++cursor;continue;}}return null;
}return $THREAD_9_123;}main();"#;
const UNITS: &str = r#"function main(){function $THREAD_8_456(input){let output=[];
if(input!=null){input.forEach(unit=>{output.push(String.fromCharCode(unit));});}
return output.join("");}return $THREAD_8_456;}main();"#;
fn named(source: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .outcomes
            .iter()
            .any(|o| o.applied && o.proposed_name.as_deref() == Some(name))
    })
}
#[test]
fn bucket_lookup_proves_scoped_record_identity() {
    for source in [
        LOOKUP.to_owned(),
        LOOKUP
            .replace("$THREAD_9_123", "$THREAD_71_887766_2")
            .replace("object,key", "$KEY_4,key")
            .replace("object===", "$KEY_4===")
            .replace("cursor", "position")
            .replace("table", "$KEY_899"),
        LOOKUP.replace("++cursor", "cursor++"),
    ] {
        assert!(named(&source, "findObjectRecordInBucket"), "{source}");
        let r = name_threads(&source, true).unwrap();
        let mut replay = source.clone();
        for e in r.report.edits.iter().rev() {
            replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
        }
        assert_eq!(replay, r.source);
    }
}
#[test]
fn bucket_lookup_refuses_changed_operations() {
    for (from, to) in [
        ("table[key]==null", "table[key]===null"),
        ("return null;", "return false;"),
        ("cursor=0", "cursor=1"),
        ("cursor<table[key].length", "cursor<=table[key].length"),
        ("object===table", "object==table"),
        ("[cursor].object", "[cursor].value"),
        ("return table[key][cursor]", "return table[key]"),
        (
            "return table[key][cursor]",
            "return table[key][cursor].object",
        ),
        ("++cursor", "--cursor"),
        ("++cursor;continue;", "++cursor;break;"),
        ("[cursor].object", "[key].object"),
        ("table[key].length", "other[key].length"),
        ("table[key][cursor].object", "other[key][cursor].object"),
        ("object,key", "object,key=0"),
        ("object,key", "object,...key"),
        ("object,key", "{object},key"),
        ("let cursor=0;", "var cursor=0;var cursor;"),
        ("let table={};", "var table={};var table;"),
        ("++cursor;continue;", "++cursor;object=other;continue;"),
        ("table[key]==null", "table?.[key]==null"),
    ] {
        let s = LOOKUP.replace(from, to);
        assert!(
            !named(&s, "findObjectRecordInBucket"),
            "accepted {from} -> {to}"
        );
    }
}
#[test]
fn bucket_lookup_refuses_runtime_parameter_and_destructured_captures() {
    for source in [
        LOOKUP.replace("main(){let table={};", "main(table){"),
        LOOKUP.replace("let table={};", "let {table}=provider;"),
        LOOKUP.replace("let table={};", "let [table]=provider;"),
    ] {
        assert!(!named(&source, "findObjectRecordInBucket"));
    }
}
#[test]
fn code_units_proves_callback_and_join() {
    for source in [
        UNITS.to_owned(),
        UNITS
            .replace("$THREAD_8_456", "$THREAD_300_761_4")
            .replace("input", "$KEY_94")
            .replace("unit", "code")
            .replace("output", "characters"),
        UNITS.replace(
            "unit=>{output.push(String.fromCharCode(unit));}",
            "unit=>output.push(String.fromCharCode(unit))",
        ),
    ] {
        assert!(named(&source, "codeUnitsToString"), "{source}");
    }
}
#[test]
fn code_units_refuses_api_argument_return_and_scope_mutations() {
    for (from, to) in [
        ("fromCharCode", "fromCodePoint"),
        ("fromCharCode(unit)", "fromCharCode(unit,0)"),
        ("fromCharCode(unit)", "fromCharCode(other)"),
        ("output.push", "other.push"),
        ("output.join", "other.join"),
        ("join(\"\")", "join(\",\")"),
        ("input.forEach", "input.map"),
        ("input!=null", "input==null"),
        ("unit=>", "(unit=0)=>"),
        ("unit=>", "(...unit)=>"),
        ("unit=>", "({unit})=>"),
        ("unit=>", "async unit=>"),
        ("input){", "input=[]) {"),
        ("let output=[];", "let output=[0];"),
        ("let output=[];", "var output=[];var output;"),
        ("main(){", "main(){let String=custom;"),
        ("String.fromCharCode(unit)", "String?.fromCharCode(unit)"),
        (
            "output.push(String.fromCharCode(unit));",
            "unit=other;output.push(String.fromCharCode(unit));",
        ),
        ("return output.join", "return input.join"),
    ] {
        let s = UNITS.replace(from, to);
        assert!(!named(&s, "codeUnitsToString"), "accepted {from} -> {to}");
    }
}
#[test]
fn names_and_collisions_preserve_existing_guards() {
    for source in [
        LOOKUP.replace("$THREAD_9_123", "humanLookup"),
        UNITS.replace("$THREAD_8_456", "humanDecoder"),
    ] {
        assert_eq!(name_threads(&source, true).unwrap().source, source);
    }
    for (source, name) in [
        (LOOKUP, "findObjectRecordInBucket"),
        (UNITS, "codeUnitsToString"),
    ] {
        let collision = source.replace(
            "main(){",
            &format!("main(){{let {name}=17;useValue({name});"),
        );
        assert!(!named(&collision, name));
    }
}

#[test]
fn code_units_supports_exact_receiver_method_pair() {
    let source = UNITS.replace(
        "return output.join(\"\");",
        "let pair=[output,output.join];return pair[1].call(pair[0],\"\");",
    );
    assert!(named(&source, "codeUnitsToString"));
    for (from, to) in [
        ("[output,output.join]", "[other,output.join]"),
        ("[output,output.join]", "[output,other.join]"),
        ("pair[1].call", "pair[0].call"),
        ("pair[0],\"\"", "pair[1],\"\""),
        ("pair[1].call", "pair[1].apply"),
        ("pair[0],\"\"", "pair[0],\",\""),
        ("pair[0],\"\"", "pair[0],\"\",0"),
        ("return pair[1]", "pair=other;return pair[1]"),
        ("let pair=", "var pair;var pair="),
        ("output.join]", "output?.join]"),
    ] {
        assert!(
            !named(&source.replace(from, to), "codeUnitsToString"),
            "accepted {from} -> {to}"
        );
    }
}

const TRIPLE: &str = "function main(){let first=[],second=[],third=0;function $THREAD_23_94(a,b,c){first=a;second=b;third=c;}return $THREAD_23_94;}main();";

#[test]
fn captured_slots_prove_exact_read_identity_and_ordered_triple() {
    for (source, role) in [
        (TRIPLE.to_owned(), "setCapturedStateTriple"),
        (TRIPLE.replace("$THREAD_23_94", "$THREAD_888_76_2").replace("first", "$KEY_812").replace("second", "middle").replace("third", "last").replace("(a,b,c)", "(x,y,z)").replace("=a;", "=x;").replace("=b;", "=y;").replace("=c;", "=z;"), "setCapturedStateTriple"),
        ("function main(){let state;function $THREAD_1_2(){return state;}return $THREAD_1_2;}main();".to_owned(), "readCapturedState"),
        ("function main(){let state;function $THREAD_11_21(value){return value===state;}return $THREAD_11_21;}main();".to_owned(), "isCapturedState"),
        ("function main(){let $KEY_128;function $THREAD_21_31(item){return $KEY_128===item;}return $THREAD_21_31;}main();".to_owned(), "isCapturedState"),
    ] {
        assert!(named(&source, role), "{role}: {source}");
    }
}

#[test]
fn captured_triple_refuses_changed_flow_and_nonstorage_origins() {
    for (from, to) in [
        ("first=a", "first+=a"),
        ("first=a;second=b", "first=b;second=a"),
        ("third=c", "second=c"),
        ("third=c;", "third=c;return c;"),
        ("first=a;", "let first=a;"),
        ("first=a;", "first=a;sideEffect();"),
        ("(a,b,c)", "(a,b,c=0)"),
        ("(a,b,c)", "({a},b,c)"),
        (
            "let first=[],second=[],third=0;",
            "var first=[],second=[],third=0;var first;",
        ),
        (
            "function main(){let first=[],second=[],third=0;",
            "function main(first,second,third){",
        ),
        (
            "let first=[],second=[],third=0;",
            "let {first,second,third}=provider;",
        ),
        ("first=a;", "a=other;first=a;"),
    ] {
        assert!(
            !named(&TRIPLE.replace(from, to), "setCapturedStateTriple"),
            "{from} -> {to}"
        );
    }
    for prefix in [
        "for(let first of values){",
        "for(let first in values){",
        "try{}catch(first){",
    ] {
        let source = TRIPLE
            .replace(
                "let first=[],second=[],third=0;",
                &format!("let second=[],third=0;{prefix}"),
            )
            .replace("return $THREAD_23_94;}", "return $THREAD_23_94;}}");
        assert!(!named(&source, "setCapturedStateTriple"), "{source}");
    }
}

#[test]
fn captured_slot_predicates_refuse_near_misses_and_preserve_names() {
    let source = "function main(){let state;function $THREAD_11_21(value){return value===state;}return $THREAD_11_21;}main();";
    for (from, to) in [
        ("===", "=="),
        ("===", "!=="),
        ("return value===state", "return value===state.value"),
        ("return value===state", "return state===state"),
        ("return value===state", "return value===value"),
        ("let state;", "var state;var state;"),
        ("function main(){let state;", "function main(state){"),
        ("return value===state", "value=other;return value===state"),
    ] {
        assert!(
            !named(&source.replace(from, to), "isCapturedState"),
            "{from} -> {to}"
        );
    }
    for role in [
        "readCapturedState",
        "isCapturedState",
        "setCapturedStateTriple",
    ] {
        let collision = TRIPLE.replace(
            "let first",
            &format!("let {role}=0;useValue({role});let first"),
        );
        if role == "setCapturedStateTriple" {
            assert!(!named(&collision, role));
        }
    }
    let human = TRIPLE.replace("$THREAD_23_94", "updateState");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
    let key = TRIPLE.replace("$THREAD_23_94", "$KEY_812");
    assert_eq!(name_threads(&key, true).unwrap().source, key);
}
