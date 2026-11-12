use shape::naming::name_threads;
fn has(body:&str)->bool {name_threads(body,true).unwrap().report.outcomes.iter().any(|o|o.applied&&o.proposed_name.as_deref()==Some("decodeUriComponent"))}
#[test]
fn complete_uri_forwarding() {
 let src="function main(){function $THREAD_9_917(value){return decodeURIComponent(value);}return $THREAD_9_917;}main();";
 assert!(has(src));assert!(has(&src.replace("value","component")));
 for (a,b) in [("decodeURIComponent(value)","decodeURI(value)"),("decodeURIComponent(value)","decodeURIComponent(other)"),("decodeURIComponent(value)","decodeURIComponent(value,1)"),("function $THREAD", "let decodeURIComponent=x;function $THREAD"),("return decode", "value=other;return decode"),("(value){", "(value=other){")] {assert!(!has(&src.replace(a,b)),"accepted {a}");}
}
