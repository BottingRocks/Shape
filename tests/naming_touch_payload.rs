use shape::naming::name_threads;
fn has(body: &str) -> bool {
 let source=format!("function main(){{let $KEY_25={{}}, $KEY_36={{}};let $KEY_38=function(){{return Date.now();}};{} return $THREAD_3_66000_2;}}main();",body);
 let r=name_threads(&source,true).unwrap();
 r.report.outcomes.iter().any(|o|o.applied&&o.proposed_name.as_deref()==Some("recordTouchSample"))
}
#[test]
fn assigned_touch_record_alias_and_split_clock() {
 let body=include_str!("fixtures/event-timing-and-encoding/touch.js");
 assert!(has(body));
 assert!(has(&body.replace("$AZ","event").replace("$BF","bounds")));
 for (a,b) in [("$BE.clientX - $BF.left","$BE.clientX + $BF.left"),("$BB.push($BF)","$BB.push($BE)"),("$BI.c = $BB","$BI.c = $BA"),("$BH.put($BI)","$BH.put($BA)"),("$KEY_36.error(e)","$KEY_36.error($AZ)"),("let $BH = $KEY_25", "let $BH = $KEY_36"),("$KEY_38()","$KEY_38(1)"),("$BF.top","$BF.left"),("$BH.put($BI)","$BH.put(...$BI)"),("$BB.push($BF)","$BB.push(...$BF)")] {
 assert!(body.contains(a));assert!(!has(&body.replace(a,b)),"accepted {a}");
 }
}
