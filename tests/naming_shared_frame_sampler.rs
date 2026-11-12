use shape::naming::name_threads;
const SAMPLE: &str = r#"function main(){let samples=[];let scale=10;let host=environment;
if(enabled){let again;
function $THREAD_1_7(timestamp){samples.push(Math.round(timestamp/scale));if(samples.length<20){host.requestAnimationFrame(again);}}
again=$THREAD_1_7;host.requestAnimationFrame($THREAD_1_7);
}return samples;}main();"#;
fn claim(source: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| {
            o.applied
                && o.evaluator.as_deref() == Some("shared_browser_frame_sampler")
                && o.role_name.as_deref() == Some("sampleNormalizedAnimationFrame")
        })
}
#[test]
fn rounded_timestamp_and_alpha_variants() {
    for source in [
        SAMPLE.to_string(),
        SAMPLE
            .replace("samples", "frames")
            .replace("scale", "resolution")
            .replace("timestamp", "time")
            .replace("again", "recur")
            .replace("host", "provider")
            .replace("$THREAD_1_7", "$THREAD_91_902_2"),
        SAMPLE.replace("timestamp/scale", "(timestamp)/(scale)"),
    ] {
        assert!(claim(&source), "{source}");
    }
}
#[test]
fn refuses_mismatched_flow_and_nonlocal_callable_donors() {
    for (before, after) in [
        ("Math.round(timestamp/scale)", "Math.round(...other)"),
        (
            "host.requestAnimationFrame(again)",
            "host.requestAnimationFrame(...other)",
        ),
        ("Math.round(timestamp/scale)", "Math.floor(timestamp/scale)"),
        ("timestamp/scale", "other/scale"),
        ("timestamp/scale", "timestamp*scale"),
        ("samples.length<20", "other.length<20"),
        ("samples.length<20", "samples.length<21"),
        (
            "host.requestAnimationFrame(again)",
            "other.requestAnimationFrame(again)",
        ),
        (
            "host.requestAnimationFrame($THREAD_1_7)",
            "other.requestAnimationFrame($THREAD_1_7)",
        ),
        ("again=$THREAD_1_7;", "again=other;"),
        ("again=$THREAD_1_7;", "function donor(){again=$THREAD_1_7;}"),
        ("again=$THREAD_1_7;", "if(other){again=$THREAD_1_7;}"),
        ("again=$THREAD_1_7;", "again=$THREAD_1_7;again=other;"),
        ("let again;", "let again=other;"),
        (
            "host.requestAnimationFrame(again);",
            "host.requestAnimationFrame(again);evil();",
        ),
        ("timestamp){", "timestamp=other){"),
        ("function main(){", "function main(Math){"),
        ("samples.push", "other.push"),
    ] {
        let mutant = SAMPLE.replace(before, after);
        assert_ne!(mutant, SAMPLE);
        assert!(!claim(&mutant), "{mutant}");
    }
    let nested = SAMPLE.replace(
        "samples.push(Math.round(timestamp/scale));",
        "function donor(){samples.push(Math.round(timestamp/scale));}",
    );
    assert!(!claim(&nested));
}
