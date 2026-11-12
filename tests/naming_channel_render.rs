use shape::naming::name_threads;
const SOURCE: &str = include_str!("fixtures/program/channel-and-frame-protocols/channel-render.js");
const TARGETS: &[&str] = &["$THREAD_6_52468", "$THREAD_6_52468_2"];
fn check(source: &str, expected: bool) {
    let result = name_threads(source, true).unwrap();
    if expected {
        let children: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.binding.name == "$THREAD_3_160307")
            .collect();
        assert_eq!(children.len(), 4);
        assert!(
            children.iter().all(|o| o.evaluator.as_deref() != Some("graphics_sine_power_channel_render")),
            "parent protocol must not name opaque child payloads"
        );
        for child in &children {
            assert!(child.applied);
            assert_eq!(child.evaluator.as_deref(), Some("encoding_audio_result_emitter"));
            assert_eq!(child.role_name.as_deref(), Some("emitSeededChannelResult"));
        }
        if source == SOURCE {
            let expected: serde_json::Value = serde_json::from_str(include_str!(
                "fixtures/program/channel-and-frame-protocols/accepted-audio-children.json"
            )).unwrap();
            assert_eq!(serde_json::to_value(&children).unwrap(), expected);
        }
    }
    for target in TARGETS {
        let o = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == *target)
            .unwrap();
        assert_eq!(
            o.applied && o.evaluator.as_deref() == Some("graphics_sine_power_channel_render"),
            expected,
            "{} {:?}",
            o.function_id,
            o.reason
        );
    }
}
fn changed(from: &str, to: &str) -> String {
    let mut s = SOURCE.to_owned();
    for (start, end) in [(256345, 258329), (247293, 249327)] {
        assert!(s[start..end].contains(from), "missing {from}");
        let body = s[start..end].replace(from, to);
        s.replace_range(start..end, &body);
    }
    s
}
#[test]
fn render_parent_positive_and_alpha() {
    check(SOURCE, true);
    check(
        &SOURCE
            .replace("$HO", "stateAlpha")
            .replace("$HP", "channelAlpha"),
        true,
    );
}
#[test]
fn render_parent_exact_effect_negatives() {
    for (a, b) in [
        ("createBuffer(1, 9,", "createBuffer(1, 8,"),
        ("getChannelData(0)", "getChannelData(1)"),
        (
            "let $KEY_2 = $HX.getChannelData(0)",
            "eval(\"opaqueWrite = 1\"); let $KEY_2 = $HX.getChannelData(0)",
        ),
        ("($HQ - $HQ >>> 0)", "($HQ - ($HQ >>> 0))"),
        (
            "$KEY_3.sampleRate * $KEY_337($HO)",
            "$KEY_3.length * $KEY_337($HO)",
        ),
        ("i = i + 3", "i = i + 2"),
        ("Math.sin(i)", "Math.cos(i)"),
        ("Math.pow($HT, $HU)", "Math.pow($HU, $HT)"),
        ("$HV.buffer = $KEY_3", "$HV.buffer = $HO"),
        ("$KEY_6 = $THREAD_3_160307", "$KEY_6 = foreignCallback"),
        ("$KEY_6($KEY_3)", "$KEY_6($HO)"),
        ("$HW.then($KEY_6)", "$HW.then($HO)"),
        ("$HX.renderedBuffer", "$HX.otherBuffer"),
        ("$KEY_17.error($HX)", "$KEY_32.error($HX)"),
        ("$HV.start();", "$HV.start(); extraEffect();"),
    ] {
        check(&changed(a, b), false);
    }
}
#[test]
fn render_parent_constructor_and_exposure_negatives() {
    check(
        &SOURCE.replace(
            "$KEY_47 = new $KEY_8(1, 9, 8003);",
            "$KEY_47 = otherContext;",
        ),
        false,
    );
    let mut early = SOURCE.to_owned();
    for target in TARGETS {
        early = early.replace(
            &format!("function {target}()"),
            &format!("$KEY_3.enqueue({target}); function {target}()"),
        );
    }
    check(&early, false);
}
