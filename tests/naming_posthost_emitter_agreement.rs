//! Focused proofs for seeded-emitter naming agreement.
//!
//! Independent frame and hash-encoding anchor proofs propose emitSeededSignalFrame and
//! emitSeededHashEncoding on the same straight-line body. name_threads collapses that pair
//! to the canonical governed label. These tests pin conflict variants and preserve solo
//! claims, foreign evaluators, wrong shapes, shadowed builtins, inherited frame labels,
//! human names, and safe duplicate-role allocations.
use shape::naming::labels::build_label_reference;
use shape::naming::{name_threads, name_threads_with_labels, ThreadName};
use shape::naming::program::ProgramSourceEdit;

/// Both anchor evaluators prove this fixed-form emitter: a `| 0` random seed, four-byte
/// nonce, XOR-reseeded mixer chain over the formal, and next([[], nonce])/complete() on one
/// captured observer. The frame and hash-encoding roles must agree.
const AGREED: &str = r#"function main(){
let observer={next(){},complete(){}},anchor=1;
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_4_100($S){
let seed=Math.random()*4294967296|0;
let nonce=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
seed=hashKey(seed^1234567);
let h=mix(seed,$S[3],nonce,3);
h=mix(seed,$S[4],nonce,h);
h=mix(seed,$S[2],nonce,h);
h=mix(seed,$S[1],nonce,h);
mix(seed,$S[0],nonce,h);
observer.next([[],nonce]);
observer.complete();
}
return $THREAD_4_100;}main();"#;

/// The same proof with every generated/generated-looking spelling changed and a
/// collision suffix on the thread name. The agreement is name-independent, and a
/// lone suffixed binding has no unsuffixed rival so it still applies.
const AGREED_RENAMED: &str = r#"function main(){
let $KEY_8={next(){},complete(){}},$KEY_9=1;
function hashOf(v){return v;}function blend(a,b,c,d){return d;}
function $THREAD_91_777_2($K){
let $KEY_5=Math.random()*4294967296|0;
let $KEY_6=[$KEY_5>>>24,$KEY_5>>16&255,$KEY_5>>8&255,$KEY_5&255];
$KEY_5=hashOf($KEY_5^7654321);
let $KEY_7=blend($KEY_5,$K[3],$KEY_6,3);
$KEY_7=blend($KEY_5,$K[4],$KEY_6,$KEY_7);
$KEY_7=blend($KEY_5,$K[2],$KEY_6,$KEY_7);
$KEY_7=blend($KEY_5,$K[1],$KEY_6,$KEY_7);
blend($KEY_5,$K[0],$KEY_6,$KEY_7);
$KEY_8.next([[],$KEY_6]);
$KEY_8.complete();
}
return $THREAD_91_777_2;}main();"#;

/// Frame-only proof: two writers and no uncoerced digest do not meet the hash-encoding
/// fixed/loop proofs, which need nine or ten statements. The solo emitSeededSignalFrame
/// label must survive.
const SOLO_FRAME: &str = r#"function main(){
let observer={next(value){},complete(){}};
function stream(key){return function(){return key;};}
function write(state,value,header){return 3;}
function $THREAD_4_100(input){
let seed=Math.random()*4294967296|0;
let header=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
seed=stream(seed^123456);
let cursor=write(seed,input[0],header,3);
cursor=write(seed,input[1],header,cursor);
observer.next([[],header]);
observer.complete();
}
return $THREAD_4_100;}main();"#;

/// Hash-encoding-only proof: the uncoerced seed fails the frame proof and its
/// `(seed|0)>>>24` header check. The solo emitSeededHashEncoding label must survive.
const SOLO_ENCODING: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_4_4($S){let seed=Math.random()*4294967296;
let nonce=[(seed|0)>>>24,(seed|0)>>16&255,(seed|0)>>8&255,(seed|0)&255];
seed=hashKey((seed|0)^1234567);
let h=mix(seed,$S[3],nonce,3);h=mix(seed,$S[4],nonce,h);
h=mix(seed,$S[2],nonce,h);h=mix(seed,$S[1],nonce,h);mix(seed,$S[0],nonce,h);
observer.next([[],nonce]);observer.complete();}
return $THREAD_4_4;}main();"#;

/// The while digest satisfies the frame and target random_digest proofs, but fails the
/// hash-encoding anchor fixed/loop proofs, which require a for scan with exactly nine
/// statements. This differs from the agreed evaluator pair, so the roles must stay
/// ambiguous.
const FOREIGN_ENCODING: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_5_5($D){
let seed=Math.random()*4294967296|0;
let nonce=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
let out=[];
seed=hashKey(seed^7654321);
let n=$D.length;
let h=mix(seed,n,nonce,3);
while(n-1>=0){h=mix(seed,$D[n-1],nonce,h);n--;}
observer.next([out,nonce]);
observer.complete();
}
return $THREAD_5_5;}main();"#;

fn applied_as(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.outcomes.iter().any(|o| {
            o.binding.name == target && o.applied && o.proposed_name.as_deref() == Some(name)
        })
    })
}

fn applied_names(source: &str, target: &str) -> Vec<Option<String>> {
    let r = name_threads(source, true).unwrap();
    r.report
        .outcomes
        .iter()
        .filter(|o| o.binding.name == target && o.applied)
        .map(|o| o.proposed_name.clone())
        .collect()
}

fn reason(source: &str, target: &str) -> Option<String> {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == target)
        .and_then(|o| o.reason.clone())
}

fn apply(source: &str) -> String {
    name_threads(source, true).unwrap().source
}

fn outcome_for<'a>(report: &'a [ThreadName], name: &str) -> &'a ThreadName {
    report.iter().find(|o| o.binding.name == name).unwrap()
}

/// Replay the reported edits explicitly by descending byte offset, proving the
/// concrete names and reference rewrites are exactly the transformation that was
/// applied. Never rely on an unsorted edit list: overlapping coordinates drift.
fn replay(source: &str, result: &str, edits: &[ProgramSourceEdit]) {
    let mut replay = source.to_owned();
    let mut edits = edits.to_vec();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result);
}

/// The exact owned-function span encoded in a `ThreadName::function_id`
/// (`fn@<start>..<end>`), used to pick the function's own role rather than the
/// outermost enclosing one.
fn function_id_span(function_id: &str) -> (u32, u32) {
    let (start, end) = function_id
        .strip_prefix("fn@")
        .and_then(|rest| rest.split_once(".."))
        .expect("function_id is fn@start..end");
    (start.parse().unwrap(), end.parse().unwrap())
}

#[test]
fn agreed_frame_and_encoding_proofs_collapse_to_the_canonical_label() {
    for (source, target) in [
        (AGREED.to_string(), "$THREAD_4_100"),
        (AGREED_RENAMED.to_string(), "$THREAD_91_777_2"),
    ] {
        assert!(
            applied_as(&source, target, "emitSeededHashEncoding"),
            "{source}"
        );
        // The frame synonym is not a second applied result on the same binding,
        // and the reported role carries only the canonical candidate.
        assert_eq!(
            applied_names(&source, target),
            vec![Some("emitSeededHashEncoding".to_string())],
            "{source}"
        );
        // The encoding proof alone cannot trigger collapse. The retained claim carries the
        // agreement note only when both independent frame and hash-encoding anchor proofs fire on
        // this same owned body.
        let agreement: Vec<String> = {
            let r = name_threads(&source, true).unwrap();
            outcome_for(&r.report.outcomes, target).explanation.clone()
        };
        assert!(
            agreement.iter().any(|e| {
                e.contains("emitSeededSignalFrame")
                    && e.contains("emitSeededHashEncoding")
                    && e.contains("independent proofs agree")
            }),
            "canonical label must come from both proofs: {agreement:?}\n{source}"
        );
        let role_names: Vec<_> = {
            let r = name_threads(&source, true).unwrap();
            let o = outcome_for(&r.report.outcomes, target);
            let (start, end) = function_id_span(&o.function_id);
            r.report
                .function_roles
                .iter()
                .find(|role| role.function_span.start == start && role.function_span.end == end)
                .map(|role| role.candidates.clone())
                .unwrap_or_default()
        };
        assert_eq!(
            role_names,
            vec!["emitSeededHashEncoding".to_string()],
            "{source}"
        );
    }
}

#[test]
fn solo_frame_claim_keeps_its_own_name() {
    assert!(applied_as(
        SOLO_FRAME,
        "$THREAD_4_100",
        "emitSeededSignalFrame"
    ));
    assert!(!applied_as(
        SOLO_FRAME,
        "$THREAD_4_100",
        "emitSeededHashEncoding"
    ));
}

#[test]
fn solo_encoding_claim_keeps_its_own_name() {
    assert!(applied_as(
        SOLO_ENCODING,
        "$THREAD_4_4",
        "emitSeededHashEncoding"
    ));
    assert!(!applied_as(
        SOLO_ENCODING,
        "$THREAD_4_4",
        "emitSeededSignalFrame"
    ));
}

#[test]
fn foreign_encoding_proof_is_not_an_agreed_alias() {
    assert_eq!(
        reason(FOREIGN_ENCODING, "$THREAD_5_5").as_deref(),
        Some("conflicting static evaluator names")
    );
    assert!(applied_names(FOREIGN_ENCODING, "$THREAD_5_5").is_empty());
    assert_eq!(apply(FOREIGN_ENCODING), FOREIGN_ENCODING);
}

#[test]
fn wrong_shape_refuses_both_proofs() {
    for source in [
        // Both observer-end proofs require an exact two-element payload.
        AGREED.replace("observer.next([[],nonce]);", "observer.next([[],nonce,3]);"),
        // The random seed must be Math.random-derived.
        AGREED.replace(
            "let seed=Math.random()*4294967296|0;",
            "let seed=Date.now()|0;",
        ),
        // The mixer bytes must be the same nonce array as the header.
        AGREED.replace("mix(seed,$S[0],nonce,h);", "mix(seed,$S[0],other,h);"),
        // The observer must be one captured receiver, not two.
        AGREED.replace("observer.complete();", "other.complete();"),
    ] {
        let target = "$THREAD_4_100";
        assert!(
            applied_names(&source, target).is_empty(),
            "accepted {source}"
        );
        assert_eq!(apply(&source), source, "{source}");
    }
}

#[test]
fn single_sided_proof_is_not_aliased_to_the_canonical_label() {
    // An extra writer preserves the frame proof (at least two writers) but breaks the fixed
    // hash-encoding proof exact ten-statement shape. The solo frame label must remain.
    let source = AGREED.replace(
        "observer.next([[],nonce]);",
        "mix(seed,$S[5],nonce,h);observer.next([[],nonce]);",
    );
    assert!(
        applied_as(&source, "$THREAD_4_100", "emitSeededSignalFrame"),
        "{source}"
    );
    assert!(!applied_as(
        &source,
        "$THREAD_4_100",
        "emitSeededHashEncoding"
    ));
}

#[test]
fn shadowed_random_refuses_the_agreement() {
    let source = AGREED.replace(
        "function main(){",
        "function main(){\nlet Math={random(){return 0;}};",
    );
    assert!(
        applied_names(&source, "$THREAD_4_100").is_empty(),
        "{source}"
    );
    assert_eq!(apply(&source), source, "{source}");
}

#[test]
fn human_names_and_readable_bindings_are_untouched() {
    let human = AGREED.replace("$THREAD_4_100", "humanEmitter");
    assert_eq!(apply(&human), human);
}

#[test]
fn inherited_frame_label_keeps_the_site_ambiguous() {
    // An inherited structural claim may propose the same frame name. Only the independently
    // proved frame anchor is a removable synonym; the inherited label must survive as ambiguity
    // metadata.
    let labeled = AGREED.replace("$THREAD_4_100", "emitSeededSignalFrame");
    let reference = build_label_reference(AGREED, &labeled).unwrap();
    let result = name_threads_with_labels(AGREED, true, Some(&reference)).unwrap();
    let outcome = outcome_for(&result.report.outcomes, "$THREAD_4_100");
    assert!(!outcome.applied);
    assert_eq!(
        outcome.reason.as_deref(),
        Some("conflicting static evaluator names")
    );
    assert_eq!(result.source, AGREED);
    let (start, end) = function_id_span(&outcome.function_id);
    let role = result
        .report
        .function_roles
        .iter()
        .find(|role| role.function_span.start == start && role.function_span.end == end)
        .expect("emitter function role");
    assert_eq!(role.status, "ambiguous");
    assert_eq!(
        role.candidates,
        vec![
            "emitSeededHashEncoding".to_string(),
            "emitSeededSignalFrame".to_string()
        ]
    );
    assert!(role
        .evaluators
        .contains(&"inherited_structural_label".to_string()));
    assert!(role
        .evaluators
        .contains(&"encoding_seeded_hash_encoding".to_string()));
    assert!(!role
        .evaluators
        .contains(&"signals_seeded_signal_frame".to_string()));
}

#[test]
fn identical_conflict_copies_stay_under_the_collision_refusal() {
    // Historical name: before the duplicate-role allocator these identical
    // unsuffixed copies were both refused as a `SimultaneousCollision`. The
    // committed allocator now gives each independently proven copy a scope-safe
    // concrete ordinal name, so both apply while the canonical role and the
    // existing unsuffixed base survive.
    let clone = AGREED.replace(
        "return $THREAD_4_100;}main();",
        &format!(
            "{}\nreturn $THREAD_4_100;}}main();",
            agreed_body("$THREAD_4_101")
        ),
    );
    let result = name_threads(&clone, true).unwrap();
    let first = outcome_for(&result.report.outcomes, "$THREAD_4_100");
    let second = outcome_for(&result.report.outcomes, "$THREAD_4_101");
    assert!(first.applied && second.applied);
    assert_eq!(
        first.proposed_name.as_deref(),
        Some("emitSeededHashEncoding")
    );
    assert_eq!(
        second.proposed_name.as_deref(),
        Some("emitSeededHashEncoding2")
    );
    assert_eq!(first.role_name.as_deref(), Some("emitSeededHashEncoding"));
    assert_eq!(second.role_name.as_deref(), Some("emitSeededHashEncoding"));
    // Reference identity: the existing unsuffixed declaration keeps the base and
    // the only reference to it follows; the cloned copy gets the ordinal.
    assert!(result
        .source
        .contains("function emitSeededHashEncoding($S){"));
    assert!(result
        .source
        .contains("function emitSeededHashEncoding2($S){"));
    assert!(result
        .source
        .contains("return emitSeededHashEncoding;}main();"));
    assert!(!result.source.contains("$THREAD_4_100"));
    assert!(!result.source.contains("$THREAD_4_101"));
    assert_eq!(first.reference_edits, 1);
    assert_eq!(second.reference_edits, 0);
    replay(&clone, &result.source, &result.report.edits);
}

#[test]
fn suffixed_conflict_copy_keeps_the_existing_reservation() {
    // Historical name: the suffixed clone used to stay unresolved under the
    // pre-allocator reservation. The committed allocator keeps the canonical
    // role on the existing unsuffixed binding and gives the clone the next free
    // ordinal, so the copies stay distinguishable and reference-safe.
    let source = AGREED.replace(
        "return $THREAD_4_100;}main();",
        &format!(
            "{}\nreturn $THREAD_4_100;}}main();",
            agreed_body("$THREAD_4_100_2")
        ),
    );
    let result = name_threads(&source, true).unwrap();
    let base = outcome_for(&result.report.outcomes, "$THREAD_4_100");
    let clone = outcome_for(&result.report.outcomes, "$THREAD_4_100_2");
    assert!(base.applied && clone.applied);
    assert_eq!(
        base.proposed_name.as_deref(),
        Some("emitSeededHashEncoding")
    );
    assert_eq!(
        clone.proposed_name.as_deref(),
        Some("emitSeededHashEncoding2")
    );
    assert_eq!(base.role_name.as_deref(), Some("emitSeededHashEncoding"));
    assert_eq!(clone.role_name.as_deref(), Some("emitSeededHashEncoding"));
    assert_eq!(clone.disambiguator, Some(2));
    assert!(result
        .source
        .contains("function emitSeededHashEncoding($S){"));
    assert!(result
        .source
        .contains("function emitSeededHashEncoding2($S){"));
    assert!(result
        .source
        .contains("return emitSeededHashEncoding;}main();"));
    assert!(!result.source.contains("$THREAD_4_100"));
    assert_eq!(base.reference_edits, 1);
    assert_eq!(clone.reference_edits, 0);
    replay(&source, &result.source, &result.report.edits);
}

fn agreed_body(name: &str) -> String {
    format!(
        "function {name}($S){{\n\
let seed=Math.random()*4294967296|0;\n\
let nonce=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];\n\
seed=hashKey(seed^1234567);\n\
let h=mix(seed,$S[3],nonce,3);\n\
h=mix(seed,$S[4],nonce,h);\n\
h=mix(seed,$S[2],nonce,h);\n\
h=mix(seed,$S[1],nonce,h);\n\
mix(seed,$S[0],nonce,h);\n\
observer.next([[],nonce]);\n\
observer.complete();\n\
}}"
    )
}
