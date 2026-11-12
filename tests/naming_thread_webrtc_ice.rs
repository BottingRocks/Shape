//! WebRTC ICE collector with an explicit if/else constructor fallback.
//!
//! The platform proof already accepts a single || constructor expression. This spelling
//! assigns RTCPeerConnection/mozRTCPeerConnection in one arm and webkitRTCPeerConnection in
//! the other before constructing the peer. Tests retain the base proof, explicit fallback,
//! alpha spelling, and dataflow near misses. The unchanged handleIceCandidate body is
//! claimed only when its collectLocalIpsViaWebRtc parent is recognized.

use shape::naming::name_threads;

const COLLECTOR: &str = "$THREAD_0_90637";
const HANDLER: &str = "$THREAD_7_109798";
const COLLECTOR_NAME: &str = "collectLocalIpsViaWebRtc";
const HANDLER_NAME: &str = "handleIceCandidate";
const COLLECTOR_EVALUATOR: &str = "function_finder::webrtc_ice_candidate_collector";
const HANDLER_EVALUATOR: &str = "function_finder::webrtc_ice_candidate_handler";

/// The RTC constructor is resolved through explicit if/else fallback before new $AR(...).
/// The nested function retains the handleIceCandidate body.
const EXPLICIT_CONSTRUCTOR_FALLBACK: &str = r#"function main(){
function $THREAD_0_90637($AM,$AN,$AO,$AP){
let $KEY_1=$AN;
let $KEY_2,$KEY_7;
$KEY_171.push($AO);
$KEY_70.push($AP);
$KEY_328=true;
let $AR;
if($KEY_1.RTCPeerConnection||$KEY_1.mozRTCPeerConnection){
$AR=$KEY_1.RTCPeerConnection||$KEY_1.mozRTCPeerConnection;
}else{
$AR=$KEY_1.webkitRTCPeerConnection;
}
if(typeof $AR!=="function"){
$KEY_151($KEY_1,[]);
return void 0;
}
$KEY_7=[];
let $AS={iceServers:void 0};
let $AT=[];
if($AM!==""){
let $AU={urls:"stun:"+$AM};
$AT[0]=$AU;
}
$AS.iceServers=$AT;
try{
$KEY_2=new $AR($AS);
if(typeof $KEY_2.createDataChannel!=="function"){
return void 0;
}else{
function $THREAD_7_109798($AV){
if($AV.candidate==null){
$KEY_2.close();
$KEY_151($KEY_1,$KEY_7);
return void 0;
}else{
let $KEY_0=/([0-9]{1,3}(\.[0-9]{1,3}){3}|[a-f0-9]{1,4}(:[a-f0-9]{1,4}){7})/.exec($AV.candidate.candidate);
let $AW,$KEY_5;
if($KEY_0!==null&&$KEY_0.length>1){
$KEY_5=$KEY_0[1];
let $AX=/host|srflx|prflx|relay/.exec($AV.candidate.candidate);
$AW=$AX!==null?$AX[0]:"";
}else{
$AW="";
}
let $AY=$KEY_5;
if($AY!=null){
$AY=!$KEY_7.some($AZ=>$AZ.ip===$KEY_5);
}else{
$AY=$AY!=null;
}
if($AY){
$AW={ip:$KEY_5,candidate:$AW};
$KEY_7.push($AW);
}
return void 0;
}
}
$KEY_2.onicecandidate=$THREAD_7_109798;
$KEY_2.createDataChannel("");
try{
$KEY_2.createOffer().then($AV=>$KEY_2.setLocalDescription($AV),$AV=>$KEY_31($KEY_1,$AV));
}catch(e){
if(e instanceof TypeError){
try{
$KEY_2.createDataChannel("");
$KEY_2.createOffer($AV=>$KEY_2.setLocalDescription($AV),$AV=>$KEY_31($KEY_1,$AV));
}catch(e_2){
$KEY_2.close();
$KEY_31($KEY_1,e_2);
return void 0;
}
return void 0;
}else{
$KEY_2.close();
$KEY_31($KEY_1,e);
return void 0;
}
}
return void 0;
}
}catch(e_3){
$KEY_31($KEY_1,e_3);
return void 0;
}
}
return $THREAD_0_90637;
}
main();"#;

/// The explicit fallback block, replaced in the negative and base variants.
const FALLBACK_BLOCK: &str = "let $AR;\nif($KEY_1.RTCPeerConnection||$KEY_1.mozRTCPeerConnection){\n$AR=$KEY_1.RTCPeerConnection||$KEY_1.mozRTCPeerConnection;\n}else{\n$AR=$KEY_1.webkitRTCPeerConnection;\n}";

/// The nearest base shape: one `||` expression the original proof already owns.
const CHAIN_BLOCK: &str =
    "let $AR=$KEY_1.RTCPeerConnection||$KEY_1.mozRTCPeerConnection||$KEY_1.webkitRTCPeerConnection;";

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.outcomes.iter().any(|o| {
            o.applied && o.binding.name == target && o.proposed_name.as_deref() == Some(name)
        })
    })
}

fn claimed_by(source: &str, name: &str, evaluator: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.function_roles.iter().any(|role| {
            role.candidates.iter().any(|c| c.as_str() == name)
                && role.evaluators.iter().any(|e| e.as_str() == evaluator)
        })
    })
}

/// A negative source must still parse and analyze; only the naming proof is
/// expected to fail, so refusals are never vacuous.
fn refused(source: &str, target: &str, name: &str) {
    let result = name_threads(source, true)
        .unwrap_or_else(|e| panic!("negative source must parse and analyze: {e}\n{source}"));
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes:\n{source}"));
    assert!(
        !(outcome.applied && outcome.proposed_name.as_deref() == Some(name)),
        "accepted {target} as {name}:\n{source}"
    );
}

fn replaced(template: &str, from: &str, to: &str) -> String {
    assert!(template.contains(from), "missing replacement anchor {from}");
    let out = template.replace(from, to);
    assert!(out != template, "replacement {from} made no change");
    out
}

#[test]
fn base_chain_is_nonvacuous_before_the_fallback() {
    // The base proof exercises the same collector: if this fixture stopped
    // being recognized for any reason, the refusal tests below would pass
    // vacuously, so it is asserted first.
    let base = replaced(EXPLICIT_CONSTRUCTOR_FALLBACK, FALLBACK_BLOCK, CHAIN_BLOCK);
    assert!(named(&base, COLLECTOR, COLLECTOR_NAME), "{base}");
    assert!(named(&base, HANDLER, HANDLER_NAME), "{base}");
    assert!(claimed_by(&base, COLLECTOR_NAME, COLLECTOR_EVALUATOR), "{base}");
}

#[test]
fn explicit_constructor_fallback_is_named_with_its_handler() {
    assert!(named(EXPLICIT_CONSTRUCTOR_FALLBACK, COLLECTOR, COLLECTOR_NAME), "{EXPLICIT_CONSTRUCTOR_FALLBACK}");
    assert!(named(EXPLICIT_CONSTRUCTOR_FALLBACK, HANDLER, HANDLER_NAME), "{EXPLICIT_CONSTRUCTOR_FALLBACK}");
    assert!(claimed_by(EXPLICIT_CONSTRUCTOR_FALLBACK, COLLECTOR_NAME, COLLECTOR_EVALUATOR));
    assert!(claimed_by(EXPLICIT_CONSTRUCTOR_FALLBACK, HANDLER_NAME, HANDLER_EVALUATOR));
}

#[test]
fn constructor_fallback_alpha_respelling_round_trips() {
    let alpha = EXPLICIT_CONSTRUCTOR_FALLBACK
        .replace(COLLECTOR, "$THREAD_991_90001")
        .replace(HANDLER, "$THREAD_992_90002")
        .replace("$AR", "$ZZ")
        .replace("$KEY_1.", "$KHOST.")
        .replace("$KEY_1,", "$KHOST,")
        .replace("$KEY_1=", "$KHOST=")
        .replace("$KEY_1)", "$KHOST)");
    assert_ne!(alpha, EXPLICIT_CONSTRUCTOR_FALLBACK);
    assert!(named(&alpha, "$THREAD_991_90001", COLLECTOR_NAME), "{alpha}");
    assert!(named(&alpha, "$THREAD_992_90002", HANDLER_NAME), "{alpha}");
}

#[test]
fn nested_else_if_fallback_reads_one_shared_host() {
    let nested = replaced(
        EXPLICIT_CONSTRUCTOR_FALLBACK,
        FALLBACK_BLOCK,
        "let $AR;\nif($KEY_1.RTCPeerConnection){$AR=$KEY_1.RTCPeerConnection;}else if($KEY_1.mozRTCPeerConnection){$AR=$KEY_1.mozRTCPeerConnection;}else{$AR=$KEY_1.webkitRTCPeerConnection;}",
    );
    assert!(named(&nested, COLLECTOR, COLLECTOR_NAME), "{nested}");
    assert!(named(&nested, HANDLER, HANDLER_NAME), "{nested}");
}

#[test]
fn donor_branch_and_foreign_host_are_refused() {
    // A branch whose value is not an RTC constructor is a donor, not a fallback:
    // the collector stops being recognized, and the parent-gated handler does too.
    let donor = replaced(
        EXPLICIT_CONSTRUCTOR_FALLBACK,
        "$AR=$KEY_1.webkitRTCPeerConnection;",
        "$AR=$KEY_1.OtherTransport;",
    );
    refused(&donor, COLLECTOR, COLLECTOR_NAME);
    refused(&donor, HANDLER, HANDLER_NAME);

    // Two branches reading the constructor from different hosts are refused.
    let foreign = replaced(
        &replaced(
            EXPLICIT_CONSTRUCTOR_FALLBACK,
            "let $KEY_1=$AN;",
            "let $KEY_1=$AN;let $KEY_9=$AO;",
        ),
        "$AR=$KEY_1.webkitRTCPeerConnection;",
        "$AR=$KEY_9.webkitRTCPeerConnection;",
    );
    refused(&foreign, COLLECTOR, COLLECTOR_NAME);
    refused(&foreign, HANDLER, HANDLER_NAME);
}

#[test]
fn extra_ctor_write_and_declarator_donor_are_refused() {
    // A third write outside the branches is not branch-owned.
    let extra = replaced(
        EXPLICIT_CONSTRUCTOR_FALLBACK,
        "if(typeof $AR!==\"function\"){",
        "$AR=$KEY_1.RTCPeerConnection;\nif(typeof $AR!==\"function\"){",
    );
    refused(&extra, COLLECTOR, COLLECTOR_NAME);

    // An initializer donor plus the branch writes is not a pure branch fallback.
    let init_donor = replaced(
        EXPLICIT_CONSTRUCTOR_FALLBACK,
        "let $AR;\nif($KEY_1.RTCPeerConnection",
        "let $AR=$KEY_1.RTCPeerConnection;\nif($KEY_1.RTCPeerConnection",
    );
    refused(&init_donor, COLLECTOR, COLLECTOR_NAME);
}

#[test]
fn captured_and_non_dominating_constructors_are_refused() {
    // The constructor local is captured from an enclosing scope, not owned.
    let captured = replaced(
        &replaced(
            EXPLICIT_CONSTRUCTOR_FALLBACK,
            "function main(){\nfunction $THREAD_0_90637",
            "function main(){\nlet $AR;\nfunction $THREAD_0_90637",
        ),
        "let $AR;\nif($KEY_1.RTCPeerConnection",
        "if($KEY_1.RTCPeerConnection",
    );
    refused(&captured, COLLECTOR, COLLECTOR_NAME);

    // The branches must dominate the construction: assigning after `new` is not
    // the fallback.
    let late = replaced(
        &replaced(EXPLICIT_CONSTRUCTOR_FALLBACK, FALLBACK_BLOCK, "let $AR;"),
        "$KEY_2=new $AR($AS);",
        "$KEY_2=new $AR($AS);if($KEY_1.RTCPeerConnection||$KEY_1.mozRTCPeerConnection){$AR=$KEY_1.RTCPeerConnection||$KEY_1.mozRTCPeerConnection;}else{$AR=$KEY_1.webkitRTCPeerConnection;}",
    );
    refused(&late, COLLECTOR, COLLECTOR_NAME);
}
