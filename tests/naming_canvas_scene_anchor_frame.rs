use shape::naming::name_threads;
const SOURCE: &str = include_str!("fixtures/closed-emission-protocols/tuple-reverse-and-storage.js");
fn check(source: &str, accepted: bool) {
    let r = name_threads(source, true).unwrap();
    let start = source.rfind("function $THREAD_3_145245(").unwrap();
    let o = r
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.declaration.start as usize == start)
        .unwrap();
    assert_eq!(
        o.evaluator.as_deref() == Some("owned_canvas_scene_anchor_frame"),
        accepted,
        "{:?}",
        o.reason
    );
    if accepted {
        assert!(o.applied);
        assert_eq!(
            o.role_name.as_deref(),
            Some("drawCanvasAndEmitPixelFrameWithAnchorProbe")
        );
    }
}
#[test]
fn complete_source_scene_and_alpha_geometry() {
    check(SOURCE, true);
    let alpha = SOURCE
        .replace("$KEY_4", "contextSlot")
        .replace("$CR", "pixelRows")
        .replace(".width = 241;", ".width = 242;")
        .replace(
            "getImageData(0, 0, 241, 245)",
            "getImageData(0, 0, 242, 245)",
        );
    check(&alpha, true);
}
#[test]
fn complete_protocol_near_misses() {
    for (old, new) in [
        ("$KEY_4.arc(113.2", "foreignContext.arc(113.2"),
        ("$BX.addColorStop(i / 9", "$BX.addColorStop(i / 8"),
        ("$KEY_252($CR, 34048, 34052)", "$KEY_252($CR, 34048, 34053)"),
        ("if ($CR !== \"submit\")", "if ($CR !== \"change\")"),
        ("$CS.detail", "$CS.data"),
        ("$KEY_16.error($DB)", "$KEY_16.complete($DB)"),
    ] {
        assert!(SOURCE.contains(old), "mutation {old}");
        check(&SOURCE.replace(old, new), false);
    }
}
