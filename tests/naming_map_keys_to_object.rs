use shape::naming::name_threads;

fn named(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.outcomes.iter().any(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some("mapKeysToObject")
                && outcome.evaluator.as_deref() == Some("runtime_map_keys_to_object")
        })
    })
}

const MAPPER: &str = r#"function main(){function $THREAD_0_60515(keys,values){let source=values;let result=A.Object.create(null);keys.forEach((key,index)=>{result[key]=source[index];});return result;}return $THREAD_0_60515;}main();"#;

#[test]
fn mapper_accepts_generated_and_alpha_variants() {
    for source in [
        MAPPER.to_string(),
        MAPPER
            .replace("$THREAD_0_60515", "$THREAD_902_44771_3")
            .replace("keys", "$KEY_21")
            .replace("values", "$KEY_22")
            .replace("source", "alias")
            .replace("result", "mapped")
            .replace("key", "field")
            .replace("index", "offset"),
    ] {
        assert!(named(&source), "{source}");
    }
}

#[test]
fn mapper_refuses_changed_operation_arguments_and_returns() {
    for (from, to) in [
        ("A.Object.create(null)", "A.Object.create({})"),
        ("keys.forEach", "keys.map"),
        ("result[key]", "result[index]"),
        ("source[index]", "source[key]"),
        ("result[key]=source[index]", "source[key]=result[index]"),
        ("return result", "return source"),
        ("let source=values;", "let source=other;"),
        (
            "let result=A.Object.create(null);",
            "let result=A.Object.create(null);result=other;",
        ),
        ("(key,index)=>", "(key,index,extra)=>"),
        ("(key,index)=>", "(key=0,index)=>"),
        ("(key,index)=>", "(...key)=>"),
        ("(key,index)=>", "({key},index)=>"),
        ("let source=values;", "var source=values;var source;"),
        (
            "let result=A.Object.create(null);",
            "let {result}=A.Object.create(null);",
        ),
    ] {
        let source = MAPPER.replace(from, to);
        assert!(!named(&source), "accepted {from} -> {to}: {source}");
    }
}

#[test]
fn mapper_refuses_alias_and_callback_identity_changes() {
    for (from, to) in [
        ("keys.forEach((key,index)=>", "other.forEach((key,index)=>"),
        ("result[key]=source[index]", "other[key]=source[index]"),
        ("result[key]=source[index]", "result[key]=other[index]"),
        ("result[key]=source[index]", "result[key]=source[offset]"),
        (
            "keys.forEach((key,index)=>{result[key]=source[index];});",
            "keys.forEach((key,index)=>{result[key]=source[index];evil();});",
        ),
        (
            "let source=values;",
            "let source;source=values;source=other;",
        ),
        ("function $THREAD_0_60515", "async function $THREAD_0_60515"),
        ("function $THREAD_0_60515", "function* $THREAD_0_60515"),
    ] {
        let source = MAPPER.replace(from, to);
        assert!(!named(&source), "accepted {from} -> {to}: {source}");
    }
}

#[test]
fn readable_names_and_collisions_remain_protected() {
    let readable = MAPPER.replace("$THREAD_0_60515", "keepMapper");
    assert_eq!(name_threads(&readable, true).unwrap().source, readable);
    let collision = MAPPER.replace("function main(){", "function main(){let mapKeysToObject=1;");
    assert!(!named(&collision));
}
