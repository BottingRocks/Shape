use shape::naming::name_threads;

// Portable, reachable main wrapper for the exact owned-body shape. All
// receivers are captured DATA; no native/global role is asserted.
const CLEANUP: &str = r#"function main(){
let d1=1,d2=2,d3=3,d4=4,d5=5;
let tokens={a:1,b:2};
let documentStub={body:{}};
function $THREAD_0_68224(form){
let target=form;
if(tokens!=null){
Object.keys(tokens).forEach(key=>{
let element;
if(target.elements.length!==0){
let pair=[target,target.compareDocumentPosition];
element=pair[1].call(pair[0],target.elements[0])&Node.DOCUMENT_POSITION_CONTAINED_BY;
}else{
element=false;
}
let selector=(element?target:documentStub.body).querySelector;
element=selector.call(element?target:documentStub.body,"input[type=hidden][name=\""+("EE30zvQLWf-"+key)+"\"]");
if(element&&element.parentNode!=null){
element.parentNode.removeChild(element);
}
});
}
}
return $THREAD_0_68224;
}main();"#;

// The exact recovered body, with the seed's `$KEY_` capture spelling and
// initializer-free captured declarations that are assigned later. This is a
// portable wrapper, not the full captured program.
const REAL_SHAPE: &str = r#"function main(){
let spacer1=1,spacer2=2,spacer3=3,spacer4=4;
let $KEY_4,$KEY_8;
$KEY_4={body:{}};
$KEY_8={a:1,b:2};
function $THREAD_0_68224($BN){
let $KEY_0=$BN;
if($KEY_8!=null){
Object.keys($KEY_8).forEach($BO=>{
let $BP;
if($KEY_0.elements.length!==0){
let $BQ=[$KEY_0,$KEY_0.compareDocumentPosition];
$BP=$BQ[1].call($BQ[0],$KEY_0.elements[0])&Node.DOCUMENT_POSITION_CONTAINED_BY;
}else{
$BP=false;
}
let $BR=($BP?$KEY_0:$KEY_4.body).querySelector;
$BP=$BR.call($BP?$KEY_0:$KEY_4.body,"input[type=hidden][name=\""+("EE30zvQLWf-"+$BO)+"\"]");
if($BP&&$BP.parentNode!=null){
$BP.parentNode.removeChild($BP);
}
});
}
}
return $THREAD_0_68224;
}main();"#;

const ROLE: &str = "removeInjectedTokenInputs";

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|report| {
        report.report.outcomes.iter().any(|outcome| {
            outcome.binding.name == target
                && outcome.applied
                && outcome.proposed_name.as_deref() == Some(name)
        })
    })
}

fn refused(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|report| {
        let target_outcome = report
            .report
            .outcomes
            .iter()
            .find(|outcome| outcome.binding.name == target);
        target_outcome.is_some()
            && !target_outcome.is_some_and(|outcome| {
                outcome.applied && outcome.proposed_name.as_deref() == Some(name)
            })
    })
}

fn alpha_renamed() -> String {
    CLEANUP
        .replace("$THREAD_0_68224", "$THREAD_912_99881_2")
        .replace("tokens", "$TOK")
        .replace("documentStub", "$DOC")
        .replace("let element;", "let $ELEM;")
        .replace("element=pair", "$ELEM=pair")
        .replace("element=false", "$ELEM=false")
        .replace("(element?", "($ELEM?")
        .replace("if(element&&", "if($ELEM&&")
        .replace("element.parentNode", "$ELEM.parentNode")
        .replace("removeChild(element)", "removeChild($ELEM)")
        .replace("element=selector", "$ELEM=selector")
        .replace("target", "$TGT")
        .replace("pair", "$PAIR")
        .replace("selector", "$SEL")
        .replace("key=>", "$KEY=>")
        .replace("+key)", "+$KEY)")
        .replace("form", "$FRM")
        .replace("EE30zvQLWf-", "abc-")
}

#[test]
fn proves_exact_token_input_cleanup() {
    assert!(named(CLEANUP, "$THREAD_0_68224", ROLE), "{CLEANUP}");
}

#[test]
fn proves_real_capture_spelling_with_initializer_free_data() {
    assert!(named(REAL_SHAPE, "$THREAD_0_68224", ROLE), "{REAL_SHAPE}");
}

#[test]
fn proves_generated_ids_alpha_renames_and_opaque_prefix() {
    let renamed = alpha_renamed();
    assert!(named(&renamed, "$THREAD_912_99881_2", ROLE), "{renamed}");
}

#[test]
fn refuses_changed_api_and_operations() {
    for (from, to) in [
        ("Object.keys(tokens)", "Object.values(tokens)"),
        (".forEach(key=>", ".map(key=>"),
        ("pair[1].call(pair[0]", "pair[0].call(pair[1]"),
        ("pair[1].call(pair[0]", "pair[1].call(pair[1]"),
        ("selector.call(", "selector.apply("),
        (
            "element.parentNode.removeChild(element)",
            "element.parentNode.appendChild(element)",
        ),
        (".querySelector;", ".getElementById;"),
        ("input[type=hidden]", "input[type=text]"),
        (
            "if(target.elements.length!==0)",
            "if(target.elements.length===0)",
        ),
        (
            "&Node.DOCUMENT_POSITION_CONTAINED_BY",
            "|Node.DOCUMENT_POSITION_CONTAINED_BY",
        ),
        (
            "Node.DOCUMENT_POSITION_CONTAINED_BY",
            "Node.DOCUMENT_POSITION_CONTAINS",
        ),
        ("element=false;", "element=true;"),
        ("if(tokens!=null)", "if(tokens==null)"),
        ("if(tokens!=null)", "if(tokens!==null)"),
        (
            "if(element&&element.parentNode!=null)",
            "if(element||element.parentNode!=null)",
        ),
        (
            "if(element&&element.parentNode!=null)",
            "if(element&&element.parentNode==null)",
        ),
        ("let element;", "let element=null;"),
    ] {
        let source = CLEANUP.replace(from, to);
        assert!(
            refused(&source, "$THREAD_0_68224", ROLE),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn refuses_changed_callback_and_selector_concatenation() {
    for (from, to) in [
        (
            "(element?target:documentStub.body).querySelector",
            "(element?target:documentStub.head).querySelector",
        ),
        (
            "selector.call(element?target:documentStub.body,",
            "selector.call(element?target:documentStub.head,",
        ),
        (
            "selector.call(element?target:documentStub.body,",
            "selector.call(target,",
        ),
        (
            "selector.call(element?target:documentStub.body,",
            "selector.call(element?other:documentStub.body,",
        ),
        ("(\"EE30zvQLWf-\"+key)", "(\"\"+key)"),
        ("(\"EE30zvQLWf-\"+key)", "(\"EE30zvQLWf-\"+other)"),
        (r#"+"\"]")"#, r#"+"\"}")"#),
        ("key=>", "({key})=>"),
        ("key=>", "(...key)=>"),
        ("key=>", "(key=0)=>"),
        ("key=>", "(key,other)=>"),
        ("key=>", "async key=>"),
        ("key=>{", "key=>{return 1;"),
        ("key=>{", "key=>{key=other;"),
        ("forEach(key=>{", "forEach(function(key){"),
    ] {
        let source = CLEANUP.replace(from, to);
        assert!(
            refused(&source, "$THREAD_0_68224", ROLE),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn refuses_changed_alias_receiver_and_removal() {
    for (from, to) in [
        ("let target=form;", "let target=other;"),
        ("let target=form;", "let target=form;target=other;"),
        ("let target=form;", "var target=form;function target(){}"),
        ("let target=form;", "let target=form,extra=1;"),
        (
            "pair[1].call(pair[0],target.elements[0])",
            "pair[1].call(pair[0],target.elements[1])",
        ),
        (
            "pair[1].call(pair[0],target.elements[0])",
            "pair[1].call(pair[0],target.elements)",
        ),
        (
            "[target,target.compareDocumentPosition]",
            "[other,target.compareDocumentPosition]",
        ),
        (
            "[target,target.compareDocumentPosition]",
            "[target,other.compareDocumentPosition]",
        ),
        ("documentStub.body", "documentStub.head"),
        ("documentStub.body", "other.body"),
        ("element.parentNode", "element.parent"),
        (
            "element.parentNode.removeChild(element)",
            "element.parentNode.remove(element)",
        ),
        (
            "let pair=[target,target.compareDocumentPosition];",
            "let pair=[target,target.compareDocumentPosition,1];",
        ),
    ] {
        let source = CLEANUP.replace(from, to);
        assert!(
            refused(&source, "$THREAD_0_68224", ROLE),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn refuses_reordered_extra_or_returning_bodies() {
    for source in [
        CLEANUP.replace(
            r#"let selector=(element?target:documentStub.body).querySelector;
element=selector.call(element?target:documentStub.body,"input[type=hidden][name=\""+("EE30zvQLWf-"+key)+"\"]");"#,
            r#"element=selector.call(element?target:documentStub.body,"input[type=hidden][name=\""+("EE30zvQLWf-"+key)+"\"]");
let selector=(element?target:documentStub.body).querySelector;"#,
        ),
        CLEANUP.replace(
            "element.parentNode.removeChild(element);",
            "element.parentNode.removeChild(element);return element;",
        ),
        CLEANUP.replace("});\n}", "});\nuseValue(1);\n}"),
        CLEANUP.replace(
            "let element;\nif(target.elements.length!==0){",
            "if(target.elements.length!==0){let element;",
        ),
    ] {
        assert!(refused(&source, "$THREAD_0_68224", ROLE), "{source}");
    }
}

#[test]
fn refuses_parameter_and_declaration_loopholes() {
    for (from, to) in [
        (
            "function $THREAD_0_68224(form)",
            "function $THREAD_0_68224(form={})",
        ),
        (
            "function $THREAD_0_68224(form)",
            "function $THREAD_0_68224(...form)",
        ),
        (
            "function $THREAD_0_68224(form)",
            "function $THREAD_0_68224({form})",
        ),
        ("function $THREAD_0_68224", "async function $THREAD_0_68224"),
        ("function $THREAD_0_68224", "function* $THREAD_0_68224"),
        ("let tokens={a:1,b:2};", "for(var tokens of [{a:1}]){}"),
    ] {
        let source = CLEANUP.replace(from, to);
        assert!(
            refused(&source, "$THREAD_0_68224", ROLE),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn refuses_future_capture_declarations() {
    let future = CLEANUP.replace("let tokens={a:1,b:2};", "").replace(
        "return $THREAD_0_68224;",
        "let tokens={a:1,b:2};return $THREAD_0_68224;",
    );
    assert!(refused(&future, "$THREAD_0_68224", ROLE), "{future}");
    let future_body = CLEANUP.replace("let documentStub={body:{}};", "").replace(
        "return $THREAD_0_68224;",
        "let documentStub={body:{}};return $THREAD_0_68224;",
    );
    assert!(
        refused(&future_body, "$THREAD_0_68224", ROLE),
        "{future_body}"
    );
}

#[test]
fn preserves_meaningful_names_and_collisions() {
    let meaningful = CLEANUP.replace("$THREAD_0_68224", "removeInjectedTokenInputs");
    assert_eq!(
        name_threads(&meaningful, true).unwrap().source,
        meaningful,
        "existing meaningful function name must be preserved"
    );

    let collision = CLEANUP
        .replace("$THREAD_0_68224", "$THREAD_912_99881_2")
        .replace(
            "function main(){",
            "function main(){let removeInjectedTokenInputs=17;useValue(removeInjectedTokenInputs);",
        );
    assert!(
        refused(&collision, "$THREAD_912_99881_2", ROLE),
        "related-scope collision must not be renamed"
    );
}

#[test]
fn direct_receiver_cleanup_complete_protocol() {
    let source = r#"function main(){
let tokens, documentStub;
function $THREAD_0_401(form){let target=form;if(tokens!=null){Object.keys(tokens).forEach(key=>{
const container=target.elements.length!==0&&target.compareDocumentPosition(target.elements[0])&Node.DOCUMENT_POSITION_CONTAINED_BY?target:documentStub.body;
let found=container.querySelector("input[type=hidden][name=\""+("opaque-"+key)+"\"]");
if(found&&found.parentNode!=null){found.parentNode.removeChild(found);}
});}}return $THREAD_0_401;}main();"#;
    assert!(named(source, "$THREAD_0_401", ROLE));
    let alpha = source
        .replace("target", "receiver")
        .replace("key=>", "entry=>")
        .replace("+key)", "+entry)")
        .replace("found", "element")
        .replace("$THREAD_0_401", "$THREAD_33_77");
    assert!(named(&alpha, "$THREAD_33_77", ROLE));
    for (from, to) in [
        ("length!==0", "length===0"),
        ("target.elements[0]", "target.elements[1]"),
        ("CONTAINED_BY", "CONTAINS"),
        ("?target:documentStub.body", "?documentStub.body:target"),
        ("container.querySelector", "target.querySelector"),
        ("+key)", "+target)"),
        ("removeChild(found)", "removeChild(target)"),
        ("found.parentNode!=null", "found.parentNode==null"),
        ("removeChild(found);", "removeChild(found);sideEffect();"),
    ] {
        assert!(source.contains(from));
        assert!(
            refused(&source.replace(from, to), "$THREAD_0_401", ROLE),
            "{from} -> {to}"
        );
    }
}
