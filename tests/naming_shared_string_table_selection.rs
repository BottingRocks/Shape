use shape::naming::name_threads;

// Portable own-body copy. Captures are unnecessary: only the formal and immutable local
// alias flow into four owned callbacks. No encoding tag table is present.
const SOURCE: &str = r#"function main(){
function $THREAD_5_1681(counts) {
 let table=counts;
 return Object.keys(counts).filter(key=>{
  if(table[key]>=2){return key.length*table[key]>=8;}else{return false;}
 }).map(key=>{return [key,(key.length+1)*table[key]-(key.length+1+table[key]*2)];})
 .sort((a,b)=>b[1]-a[1]).slice(0,255).map(pair=>pair[0]);
}
return $THREAD_5_1681;
}main();"#;
fn claims(source: &str) -> usize {
    name_threads(source, true)
        .expect("fixture must parse and build a valid Model")
        .report
        .outcomes
        .iter()
        .filter(|o| {
            o.applied
                && o.evaluator.as_deref() == Some("signals_string_table_selection")
                && o.role_name.as_deref() == Some("selectStringTable")
        })
        .count()
}
#[test]
fn standalone_complete_chain_and_lexical_variants() {
    for source in [
        SOURCE.to_string(),
        SOURCE.replace("Object.keys(counts)", "Object.keys(table)"),
        SOURCE
            .replace("counts", "frequencies")
            .replace("table", "local")
            .replace("key", "token")
            .replace("Object.tokens", "Object.keys")
            .replace("$THREAD_5_1681", "$THREAD_91_432_2"),
        SOURCE
            .replace("b[1]-a[1]", "(b[1])-(a[1])")
            .replace("pair=>pair[0]", "pair=>{return pair[0];}"),
        SOURCE.replace(
            "return $THREAD_5_1681;",
            "function decoy(table){return table;}return $THREAD_5_1681;",
        ),
    ] {
        assert_eq!(claims(&source), 1, "{source}");
    }
}
#[test]
fn valid_semantic_mutations_fail_closed() {
    for (from, to) in [
        ("table[key]>=2", "table[key]>2"),
        ("table[key]>=2", "table[key]>=3"),
        ("key.length*table[key]>=8", "key.length*table[key]>=9"),
        ("return false;", "return true;"),
        ("table[key]*2", "table[key]*3"),
        ("(key.length+1)*table[key]", "(key.length+2)*table[key]"),
        ("b[1]-a[1]", "a[1]-b[1]"),
        ("b[1]-a[1]", "b[0]-a[1]"),
        ("slice(0,255)", "slice(1,255)"),
        ("slice(0,255)", "slice(0,256)"),
        ("pair=>pair[0]", "pair=>pair[1]"),
        ("pair=>pair[0]", "pair=>{ sideEffect();return pair[0];}"),
        ("key=>{return [key,", "key=>{sideEffect();return [key,"),
        ("table=counts;", "table=counts;table=other;"),
        ("table=counts;", "table=other;"),
        (
            "function $THREAD_5_1681(counts)",
            "function $THREAD_5_1681(counts={})",
        ),
        (
            "function $THREAD_5_1681(counts)",
            "async function $THREAD_5_1681(counts)",
        ),
        (
            "function $THREAD_5_1681(counts)",
            "function $THREAD_5_1681(counts,...rest)",
        ),
        ("Object.keys(counts)", "Object.keys(counts,other)"),
        (".filter(key=>", ".filter(async key=>"),
        ("key=>{\n  if(table[key]>=2)", "key=>{\n  if(other[key]>=2)"),
        ("let table=counts;", "let table=counts;let Object=other;"),
        (".sort((a,b)=>b[1]-a[1])", ".sort((a,b)=>b[1]-a[1],other)"),
    ] {
        assert!(
            SOURCE.contains(from),
            "mutation must change evidence: {from}"
        );
        let source = SOURCE.replace(from, to);
        assert_eq!(claims(&source), 0, "mutation {from} -> {to}\n{source}");
    }
}
#[test]
fn unused_complete_donor_does_not_name_outer_or_impostor() {
    let source = SOURCE.replace(
        "return $THREAD_5_1681;",
        r#"
function $THREAD_91_1(value){return value;}
function $THREAD_92_1(value){function donor(){return Object.keys(value);}return value;}
return [$THREAD_5_1681,$THREAD_91_1,$THREAD_92_1];"#,
    );
    let report = name_threads(&source, true).expect("valid Model").report;
    assert_eq!(claims(&source), 1);
    assert!(report
        .outcomes
        .iter()
        .filter(|o| o.binding.name == "$THREAD_91_1" || o.binding.name == "$THREAD_92_1")
        .all(|o| o.role_name.as_deref() != Some("selectStringTable")));
}
