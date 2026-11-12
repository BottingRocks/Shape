use shape::naming::name_threads;
const SOURCE: &str = include_str!("fixtures/program/channel-and-frame-protocols/scaled-animation-frame.js");
fn check(source: &str, bindings: &[&str], name: &str, expected: bool) {
    let result = name_threads(source, true).unwrap();
    for binding in bindings {
        let outcome = result.report.outcomes.iter().find(|o| o.binding.name == *binding).unwrap();
        assert_eq!(outcome.applied && outcome.proposed_name.as_deref().is_some_and(|n| n.starts_with(name)), expected, "{} {:?}",outcome.function_id,outcome.reason);
    }
}
fn mutate(spans: &[(usize,usize)], from: &str, to: &str) -> String {
    let mut s=SOURCE.to_owned();
    for &(start,end) in spans.iter().rev() {
        assert!(s[start..end].contains(from), "missing mutation {from}");
        let body=s[start..end].replace(from,to);s.replace_range(start..end,&body);
    }
    s
}
const FRAMES: &[&str] = &["$THREAD_1_134770", "$THREAD_1_134770_2"];
#[test] fn frame_positive_alpha() {
    check(SOURCE,FRAMES,"recordScaledFrameAndRequestCallback",true);
    check(&SOURCE.replace("$CE", "frameTimestampAlpha"),FRAMES,"recordScaledFrameAndRequestCallback",true);
}
#[test] fn frame_negatives() {
    for (a,b) in [("Math.round", "Math.floor"),("$CE / $KEY_13", "$KEY_13 / $CE"),("$KEY_10.length", "$KEY_13.length"),("< 20", "<= 20"),("$KEY_4.requestAnimationFrame", "$KEY_10.requestAnimationFrame"),("$KEY_20)", "$KEY_4)"),("$KEY_10.push", "extra(); $KEY_10.push")] {
        check(&mutate(&[(355214,355392),(356688,356868)],a,b),FRAMES,"recordScaledFrameAndRequestCallback",false);
    }
}
