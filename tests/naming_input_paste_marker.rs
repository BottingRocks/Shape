use shape::naming::name_threads;
const SOURCE: &str = include_str!("fixtures/input_paste_marker/captured-slot-factory.js");
const NAME: &str = "$THREAD_5_85895";
const ROLE: &str = "countInputPasteAndMarkInactiveTarget";

fn check(source: &str, name: &str, accepted: bool) {
    let source = format!(
        "function main() {{ let harnessA=0, harnessB=1, harnessC=2; {source}\n return 0; }} main();"
    );
    let result = name_threads(&source, true).expect("valid full factory model");
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == name)
        .expect("indexed callback");
    assert_eq!(
        // Negative cases retain the scoped-origin evaluator boundary: the older
        // opaque-receiver rule may independently prove a different origin.
        if accepted {
            matches!(
                outcome.evaluator.as_deref(),
                Some("events_input_paste_marker" | "events_input_paste_marker_hoisted_capture")
            )
        } else {
            outcome.evaluator.as_deref() == Some("events_input_paste_marker")
        },
        accepted,
        "{outcome:?}"
    );
    if accepted {
        assert!(outcome.applied);
        assert_eq!(outcome.role_name.as_deref(), Some(ROLE));
        assert!(
            outcome
                .explanation
                .iter()
                .any(|e| e.contains("explicitly unproved")
                    || e.contains("no native DOM/event identity claimed"))
        );
    }
    let mut replay = source.clone();
    let mut edits = result.report.edits.clone();
    edits.sort_by_key(|e| std::cmp::Reverse(e.start));
    for e in edits {
        replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
    }
    assert_eq!(replay, result.source);
}

#[test]
fn source_backed_factory_and_alpha_protocol() {
    check(SOURCE, NAME, true);
    let alpha = SOURCE
        .replace(NAME, "$THREAD_77_900")
        .replace("$BP", "eventRecord")
        .replace("$KEY_4", "pasteSlot")
        .replace("$KEY_0", "inactiveSlot")
        .replace("$KEY_18", "documentPort")
        .replace("$KEY_13", "observerPort");
    check(&alpha, "$THREAD_77_900", true);
}

#[test]
fn capture_values_preserve_coercion_without_boolean_or_number_assumptions() {
    let changed = SOURCE
        .replace("let $KEY_4 = 0;", "let $KEY_4 = '7';")
        .replace("let $KEY_0 = false;", "let $KEY_0 = 19;");
    check(&changed, NAME, true);
}

#[test]
fn exact_body_semantic_negatives() {
    for (old, new) in [
        ("tagName === \"INPUT\"", "tagName === \"TEXTAREA\""),
        (
            "inputType === \"insertFromPaste\"",
            "inputType === \"insertText\"",
        ),
        ("tagName ===", "tagName =="),
        ("inputType ===", "inputType !=="),
        ("$BP.target !==", "$BP.target !="),
        ("else if ($BP.target", "if ($BP.target"),
        ("++$KEY_4;", "++$BP;"),
        ("++$KEY_4;", "++$KEY_0;"),
        ("++$KEY_4;", "$KEY_4++;"),
        ("++$KEY_4;", "$KEY_4 += 1;"),
        ("$KEY_0 = true;", "$KEY_4 = true;"),
        ("$KEY_0 = true;", "$KEY_0 = 1;"),
        ("$KEY_13.error(e)", "$KEY_13.error($BP)"),
        ("$KEY_13.error(e)", "$KEY_18.error(e)"),
        ("++$KEY_4;", "++$KEY_4; extra();"),
        ("$KEY_13.error(e);", "$KEY_13.error(e); extra();"),
        (
            "} catch (e) {\n\t\t\t\t\t\t$KEY_13.error(e);",
            "} catch (e) {\nlet $KEY_13 = {}; $KEY_13.error(e);",
        ),
        (".activeElement", ".lastElementChild"),
        (
            "$KEY_18 = $KEY_12.global.document;",
            "$KEY_18 = $KEY_12.global.navigator;",
        ),
    ] {
        assert_eq!(SOURCE.matches(old).count(), 1, "mutation {old}");
        check(&SOURCE.replace(old, new), NAME, false);
    }
}

#[test]
fn early_initialization_and_shadow_negatives() {
    for (old, new) in [
        ("let $KEY_4 = 0;", "let $KEY_4;"),
        ("let $KEY_0 = false;", "let $KEY_0;"),
        ("let $KEY_4 = 0;", "if (condition) { var $KEY_4 = 0; }"),
        (
            "let $KEY_4 = 0;",
            "$AK(() => $THREAD_5_85895()); let $KEY_4 = 0;",
        ),
        (
            "function $THREAD_5_85895($BP) {",
            "function $THREAD_5_85895($BP) { let $KEY_4 = 0;",
        ),
        (
            "let $KEY_4 = 0;\n\t\t\t$AK(() => {",
            "$AK(() => { let $KEY_4 = 0;",
        ),
    ] {
        // The fourth mutation is a different unresolved binding and is not an
        // exposure of this callback; use the actual hoisted exposure below.
        if new.starts_with("$AK(() => $THREAD") {
            continue;
        }
        assert_eq!(SOURCE.matches(old).count(), 1);
        // Moving into callback before the function still initializes correctly.
        let accepted = new == "$AK(() => { let $KEY_4 = 0;";
        check(&SOURCE.replace(old, new), NAME, accepted);
    }
    let early = SOURCE.replace("let $KEY_4 = 0;", "").replace(
        "$KEY_16 = $THREAD_5_85895;",
        "$KEY_16 = $THREAD_5_85895; let $KEY_4 = 0;",
    );
    check(&early, NAME, false);
    // Read the actual hoisted callback before its local counter initializes.
    let early = SOURCE.replace("let $KEY_4 = 0;", "").replace(
        "$AK(() => {",
        "$AK(() => { $THREAD_5_85895(); let $KEY_4 = 0;",
    );
    check(&early, NAME, false);
}

#[test]
fn meaningful_names_remain_protected() {
    let source = SOURCE.replace(NAME, "existingInputHandler");
    let result = name_threads(&format!("function main(){{{source}}}main();"), true).unwrap();
    assert!(result.source.contains("function existingInputHandler("));
}

#[test]
fn straight_line_initless_mutable_slots() {
    let source = include_str!("fixtures/input_paste_marker/ordered-slot-factory.js");
    for name in ["$THREAD_5_86532", "$THREAD_5_86532_2"] {
        check(source, name, true);
        check(
            &source.replace("$KEY_17 = 0;", "if (condition) { $KEY_17 = 0; }"),
            name,
            false,
        );
        check(&source.replace("$KEY_4 = false;", ""), name, false);
        check(
            &source.replace("$DG.target !==", "$DG.target ==="),
            name,
            false,
        );
    }
}

#[test]
fn late_capture_keeps_accepted_hoisted_metadata_and_initialization_proof() {
    let source = "function main(){function factory(ports,context,register){let alias=context;let receiver,count,inactive;receiver=ports[1];let doc=alias.global.document;inactive=false;count=0;register(()=>{function $THREAD_91_1(event){try{if(event.target.tagName==='INPUT'){if(event.inputType==='insertFromPaste'){++count;}else if(event.target!==doc.activeElement){inactive=true;}}}catch(error){receiver.error(error);}}let handler=$THREAD_91_1;registerHandler(handler);});}factory(ports,context,register);}main();";
    for (source, accepted) in [
        (source.to_owned(), true),
        (
            source
                .replace("receiver", "errorPort")
                .replace("count", "pasteCount"),
            true,
        ),
        (source.replace("count=0;", "if(ready)count=0;"), false),
        (
            source
                .replace("count=0;", "")
                .replace("let handler=", "$THREAD_91_1(event);count=0;let handler="),
            false,
        ),
        (
            source.replace("let handler=", "receiver=other;let handler="),
            false,
        ),
        (source.replace("inactive=false;", ""), false),
    ] {
        let result = name_threads(&source, true).unwrap();
        let outcome = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == "$THREAD_91_1")
            .unwrap();
        if accepted {
            assert!(outcome.applied);
            assert_eq!(
                outcome.evaluator.as_deref(),
                Some("events_input_paste_marker_hoisted_capture")
            );
            assert_eq!(outcome.role_name.as_deref(), Some(ROLE));
            assert_eq!(
                outcome.explanation,
                vec![
                    "whole exact INPUT/insertFromPaste try/catch protocol; unchanged event, own prefix counter increment and inactive-target true store, same catch error forwarded; every writer accounted for and exact executed initialization reaches callable exposures; receivers remain opaque, no native DOM/event identity claimed"
                ]
            );
        } else {
            assert!(
                !matches!(
                    outcome.evaluator.as_deref(),
                    Some("events_input_paste_marker" | "events_input_paste_marker_hoisted_capture")
                ),
                "{outcome:?}"
            );
        }
    }
}

#[test]
fn strict_direct_precedes_hoisted_and_scoped_fallback_keeps_metadata() {
    const DIRECT: &str = "whole try/catch body reads the unchanged formal target.tagName strictly equal to INPUT; insertFromPaste strictly increments its own captured slot with prefix ++, otherwise target strictly unequal to captured activeElement assigns true to a separate slot; the exact caught error goes to the stable captured error receiver; scoped origins initialize before callback exposure, slot value types are unrestricted; native DOM identity and external event purpose are explicitly unproved";
    const HOISTED: &str = "whole exact INPUT/insertFromPaste try/catch protocol; unchanged event, own prefix counter increment and inactive-target true store, same catch error forwarded; every writer accounted for and exact executed initialization reaches callable exposures; receivers remain opaque, no native DOM/event identity claimed";
    let direct = "function main(){function factory(ports,context,register){let alias=context;let receiver=ports[1],doc=alias.global.document;let count=0,inactive=false;register(()=>{function $THREAD_92_1(event){try{if(event.target.tagName==='INPUT'){if(event.inputType==='insertFromPaste'){++count;}else if(event.target!==doc.activeElement){inactive=true;}}}catch(error){receiver.error(error);}}registerHandler($THREAD_92_1);});}factory(ports,context,register);}main();";
    let hoisted = direct.replace(
        "let count=0,inactive=false;",
        "let count,inactive;count=0;inactive=false;",
    );
    let scoped = direct.replace(
        "let count=0,inactive=false;",
        "let count,inactive;count='7';inactive=19;",
    );
    for (source, evaluator, explanation) in [
        (direct.to_owned(), "events_input_paste_marker", DIRECT),
        (
            direct.replace("count=0,inactive=false", "count='7',inactive=19"),
            "events_input_paste_marker",
            DIRECT,
        ),
        (
            hoisted,
            "events_input_paste_marker_hoisted_capture",
            HOISTED,
        ),
        (scoped.clone(), "events_input_paste_marker", DIRECT),
        (
            scoped
                .replace("alias", "contextAlias")
                .replace("receiver", "errorPort")
                .replace("count", "pasteCount"),
            "events_input_paste_marker",
            DIRECT,
        ),
    ] {
        let result = name_threads(&source, true).unwrap();
        let outcome = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == "$THREAD_92_1")
            .unwrap();
        assert!(outcome.applied, "{outcome:?}");
        assert_eq!(outcome.evaluator.as_deref(), Some(evaluator));
        assert_eq!(outcome.proposed_name.as_deref(), Some(ROLE));
        assert_eq!(outcome.role_name.as_deref(), Some(ROLE));
        assert_eq!(outcome.explanation, vec![explanation]);
    }
    for (old, new) in [
        ("count='7';", "if(ready){count='7';}"),
        ("inactive=19;", ""),
        ("count='7';inactive=19;register(()=>{function", "inactive=19;register(()=>{$THREAD_92_1(event);count='7';function"),
        ("try{if", "let count=0;try{if"),
        ("receiver.error(error)", "doc.error(error)"),
        ("receiver.error(error)", "receiver.error(event)"),
        ("++count;", "++count;extra();"),
        (
            "registerHandler($THREAD_92_1)",
            "receiver=other;registerHandler($THREAD_92_1)",
        ),
    ] {
        assert_eq!(scoped.matches(old).count(), 1, "mutation {old}");
        let source = scoped.replace(old, new);
        let result = name_threads(&source, true).unwrap();
        let outcome = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == "$THREAD_92_1")
            .unwrap();
        assert!(
            !matches!(
                outcome.evaluator.as_deref(),
                Some("events_input_paste_marker" | "events_input_paste_marker_hoisted_capture")
            ),
            "mutation {old}: {outcome:?}"
        );
    }
}
