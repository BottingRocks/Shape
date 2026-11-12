//! Focused, portable coverage for the extracted `.put` alias spelling of the
//! mouseup sample recorder:
//!
//! ```js
//! let ring = new Ring(30);
//! let method = ring.put;
//! let row = { d: [true, false] };
//! method.call(ring, row);
//! ```
//!
//! The canonical witness is the exact extracted-put mouseup body. The
//! direct `.put` siblings keep their existing names, and the same-input
//! mousedown/click bodies prove that event classification is unchanged.
//! Negatives are meaningful receiver, member, alias-write, ownership, branch,
//! ordering, nesting, arity and event near misses that must stay unclaimed.
//!
//! The portable sources are wrapped in a minimal carrier; the full cached
//! seeds are not read by this target.
use shape::naming::name_threads;

const MOUSE_UP: &str = "recordMouseUpSample";
const MOUSE_DOWN: &str = "captureMouseDownEvent";
const MOUSE_CLICK: &str = "captureClickEvent";
const UP_EVALUATOR: &str = "function_finder::interaction_mouse_up_sample_recorder";
const DOWN_EVALUATOR: &str = "function_finder::interaction_mouse_down_sample_recorder";
const CLICK_EVALUATOR: &str = "function_finder::interaction_click_sample_recorder";

/// Exact mouseup recorder body using an extracted put alias.
const EXTRACTED_PUT_MOUSEUP: &str = r#"function $THREAD_5_109765($CF) {
							let $CG, $CH;
							try {
								$CH = $CF.target;
								$CG = null;
								let $CI, $CJ;
								if ($CH != null) {
									$CJ = $CH.id;
									$CI = $CH.name;
									$CG = $CH.offsetHeight;
									$CH = $CH.offsetWidth;
								} else {
									$CG = null;
									$CH = null;
									$CI = null;
									$CJ = null;
								}
								$CJ = {
									id: $CJ,
									name: $CI,
									height: $CG,
									width: $CH
								};
								$CH = $CF;
								let $CK = {
									instanceOfUIEvent: $CF instanceof UIEvent,
									markedAsTrusted: !!$CF.isTrusted
								};
								$CH = $CF.target;
								const $CL = $CH instanceof Element ? $CH.getBoundingClientRect() : null;
								$CH = 109944;
								$CG = 8;
								const $CM = $CL == null || $CF.clientX == null ? 0 : Math.round($CF.clientX - $CL.left);
								$CG = 109995;
								$CH = 0;
								const $CN = $CL == null || $CF.clientY == null ? 0 : Math.round($CF.clientY - $CL.top);
								let $CO = $KEY_33;
								let $CP = $KEY_33.put;
								let $CQ = {
									a: $KEY_23(),
									b: $CF.pageX == null ? null : Math.round($CF.pageX)
								};
								$CG = 110197;
								$CH = 0;
								$CQ.c = $CF.pageY == null ? null : Math.round($CF.pageY);
								$CQ.d = [true, false];
								$CQ.e = $CK.instanceOfUIEvent;
								$CQ.f = $CK.markedAsTrusted;
								$CQ.g = $CJ.height;
								$CQ.h = $CJ.id;
								$CQ.i = $CJ.name;
								$CQ.j = $CM;
								$CQ.k = $CN;
								$CQ.l = $CJ.width;
								$CQ.m = $CF.button;
								$CP.call($CO, $CQ);
								$KEY_32 = true;
							} catch (e) {
								$KEY_28.error(e);
							}
						}"#;

/// Exact direct-put mousedown recorder body.
const EXTRACTED_PUT_MOUSEDOWN: &str = r#"function $THREAD_5_162946($CF) {
							try {
								let $CG, $CH, $CI, $CJ;
								if ($CF.target != null) {
									$CJ = $CF.target.id;
									$CG = $CF.target.name;
									$CH = $CF.target.offsetHeight;
									$CI = $CF.target.offsetWidth;
								} else {
									$CG = null;
									$CH = null;
									$CI = null;
									$CJ = null;
								}
								$CJ = {
									id: $CJ,
									name: $CG,
									height: $CH,
									width: $CI
								};
								let $CK = {
									instanceOfUIEvent: $CF instanceof UIEvent,
									markedAsTrusted: !!$CF.isTrusted
								};
								let $CL = $CF.target;
								$CL = $CL instanceof Element ? $CL.getBoundingClientRect() : null;
								const $CM = $CL == null || $CF.clientX == null ? 0 : Math.round($CF.clientX - $CL.left);
								$CL = $CL == null || $CF.clientY == null ? 0 : Math.round($CF.clientY - $CL.top);
								$KEY_33.put({
									a: $KEY_23(),
									b: $CF.pageX == null ? null : Math.round($CF.pageX),
									c: $CF.pageY == null ? null : Math.round($CF.pageY),
									d: [false, true],
									e: $CK.instanceOfUIEvent,
									f: $CK.markedAsTrusted,
									g: $CJ.height,
									h: $CJ.id,
									i: $CJ.name,
									j: $CM,
									k: $CL,
									l: $CJ.width,
									m: $CF.button
								});
								$KEY_32 = true;
							} catch (e) {
								$KEY_28.error(e);
							}
						}"#;

/// Exact direct-put click recorder body.
const EXTRACTED_PUT_CLICK: &str = r#"function $THREAD_3_89823($CF) {
							try {
								let $CG, $CH, $CI, $CJ;
								if ($CF.target != null) {
									$CH = $CF.target.id;
									$CG = $CF.target.name;
									$CJ = $CF.target.offsetHeight;
									$CI = $CF.target.offsetWidth;
								} else {
									$CG = null;
									$CH = null;
									$CI = null;
									$CJ = null;
								}
								$CH = {
									id: $CH,
									name: $CG,
									height: $CJ,
									width: $CI
								};
								let $CK = {
									instanceOfUIEvent: $CF instanceof UIEvent,
									markedAsTrusted: !!$CF.isTrusted
								};
								let $CL = $CF.target;
								$CL = $CL instanceof Element ? $CL.getBoundingClientRect() : null;
								const $CM = $CL == null || $CF.clientX == null ? 0 : Math.round($CF.clientX - $CL.left);
								$CL = $CL == null || $CF.clientY == null ? 0 : Math.round($CF.clientY - $CL.top);
								$KEY_33.put({
									a: $KEY_23(),
									b: $CF.pageX == null ? null : Math.round($CF.pageX),
									c: $CF.pageY == null ? null : Math.round($CF.pageY),
									d: [false, false],
									e: $CK.instanceOfUIEvent,
									f: $CK.markedAsTrusted,
									g: $CH.height,
									h: $CH.id,
									i: $CH.name,
									j: $CM,
									k: $CL,
									l: $CH.width,
									m: $CF.button
								});
								$KEY_32 = true;
							} catch (e) {
								$KEY_28.error(e);
							}
						}"#;

fn function_name(body: &str) -> &str {
    body.split_once('(')
        .map(|(prefix, _)| prefix.split_whitespace().last().unwrap_or(""))
        .expect("function body")
}

fn carrier(handlers: &[(&str, &str)]) -> String {
    let mut source = String::from(
        "(function main(){\n\
         let $KEY_33={put(){},toArray(){return[]}};\n\
         let $KEY_34={put(){}};\n\
         let $KEY_23=()=>0;\n\
         let $KEY_32=false;\n\
         let $KEY_28={error(){}};\n\
         let $KEY_6={addEventListener(){}};\n",
    );
    for (_, body) in handlers {
        source.push_str(body);
        source.push('\n');
    }
    for (event, body) in handlers {
        source.push_str(&format!(
            "$KEY_6.addEventListener(\"{event}\",{},false);\n",
            function_name(body)
        ));
    }
    source.push_str("})();");
    source
}

fn mouseup(body: &str) -> String {
    carrier(&[("mouseup", body)])
}

fn claims(source: &str, name: &str, evaluator: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.candidates.iter().any(|candidate| candidate == name)
                && role.evaluators.iter().any(|found| found == evaluator)
        })
        .count()
}

fn applied(source: &str, name: &str, evaluator: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some(name)
                && outcome.evaluator.as_deref() == Some(evaluator)
        })
        .count()
}

fn refused(source: &str, name: &str) -> bool {
    claims(source, name, UP_EVALUATOR) == 0 && applied(source, name, UP_EVALUATOR) == 0
}

#[test]
fn witness_fixtures_are_the_exact_extracted_put_bodies() {
    assert_eq!(EXTRACTED_PUT_MOUSEUP.len(), 1690);
    assert_eq!(EXTRACTED_PUT_MOUSEDOWN.len(), 1397);
    assert_eq!(EXTRACTED_PUT_CLICK.len(), 1397);
    assert!(EXTRACTED_PUT_MOUSEUP.contains("let $CP = $KEY_33.put;"));
    assert!(EXTRACTED_PUT_MOUSEUP.contains("$CP.call($CO, $CQ);"));
}

#[test]
fn actual_extracted_put_mouseup_alias_is_claimed_and_renamed() {
    let source = mouseup(EXTRACTED_PUT_MOUSEUP);
    assert_eq!(claims(&source, MOUSE_UP, UP_EVALUATOR), 1, "{source}");
    assert_eq!(applied(&source, MOUSE_UP, UP_EVALUATOR), 1, "{source}");
    let renamed = name_threads(&source, true).unwrap().source;
    assert!(
        renamed.contains("function recordMouseUpSample("),
        "{renamed}"
    );
    assert!(
        !renamed.contains("function $THREAD_5_109765("),
        "{renamed}"
    );
}

#[test]
fn alias_registered_mouseup_handler_is_claimed() {
    let source = mouseup(EXTRACTED_PUT_MOUSEUP).replace(
        "$KEY_6.addEventListener(\"mouseup\",$THREAD_5_109765,false);",
        "let $KEY_11=$THREAD_5_109765;$KEY_6.addEventListener(\"mouseup\",$KEY_11,false);",
    );
    assert!(source.contains("let $KEY_11=$THREAD_5_109765;"), "{source}");
    assert_eq!(claims(&source, MOUSE_UP, UP_EVALUATOR), 1, "{source}");
    assert_eq!(applied(&source, MOUSE_UP, UP_EVALUATOR), 1, "{source}");
}

#[test]
fn alpha_renamed_alias_is_claimed() {
    let body = EXTRACTED_PUT_MOUSEUP
        .replace("$CP", "putAlias")
        .replace("$CO", "ringAlias")
        .replace("$CQ", "rowSample");
    assert!(body.contains("let putAlias = $KEY_33.put;"), "{body}");
    let source = mouseup(&body);
    assert_eq!(claims(&source, MOUSE_UP, UP_EVALUATOR), 1, "{source}");
    assert_eq!(applied(&source, MOUSE_UP, UP_EVALUATOR), 1, "{source}");
}

#[test]
fn direct_put_mouseup_base_is_preserved() {
    let direct = EXTRACTED_PUT_MOUSEUP
        .replace("let $CO = $KEY_33;", "")
        .replace("let $CP = $KEY_33.put;", "")
        .replace("$CP.call($CO, $CQ);", "$KEY_33.put($CQ);");
    assert!(direct.contains("$KEY_33.put($CQ);"), "{direct}");
    let source = mouseup(&direct);
    assert_eq!(claims(&source, MOUSE_UP, UP_EVALUATOR), 1, "{source}");
    assert_eq!(applied(&source, MOUSE_UP, UP_EVALUATOR), 1, "{source}");
}

#[test]
fn same_input_direct_mousedown_and_click_keep_their_roles() {
    let source = carrier(&[
        ("mousedown", EXTRACTED_PUT_MOUSEDOWN),
        ("click", EXTRACTED_PUT_CLICK),
    ]);
    assert_eq!(
        claims(&source, MOUSE_DOWN, DOWN_EVALUATOR),
        1,
        "{source}"
    );
    assert_eq!(applied(&source, MOUSE_DOWN, DOWN_EVALUATOR), 1, "{source}");
    assert_eq!(
        claims(&source, MOUSE_CLICK, CLICK_EVALUATOR),
        1,
        "{source}"
    );
    assert_eq!(
        applied(&source, MOUSE_CLICK, CLICK_EVALUATOR),
        1,
        "{source}"
    );
    assert!(refused(&source, MOUSE_UP), "{source}");
}

#[test]
fn alias_and_direct_siblings_coexist() {
    let direct = EXTRACTED_PUT_MOUSEUP
        .replace("let $CO = $KEY_33;", "")
        .replace("let $CP = $KEY_33.put;", "")
        .replace("$CP.call($CO, $CQ);", "$KEY_33.put($CQ);");
    let source = carrier(&[
        ("mouseup", direct.as_str()),
        ("mousedown", EXTRACTED_PUT_MOUSEDOWN),
        ("click", EXTRACTED_PUT_CLICK),
    ]);
    assert_eq!(claims(&source, MOUSE_UP, UP_EVALUATOR), 1, "{source}");
    assert_eq!(applied(&source, MOUSE_UP, UP_EVALUATOR), 1, "{source}");
    assert_eq!(applied(&source, MOUSE_DOWN, DOWN_EVALUATOR), 1, "{source}");
    assert_eq!(applied(&source, MOUSE_CLICK, CLICK_EVALUATOR), 1, "{source}");
}

#[test]
fn event_controls_which_alias_role_is_claimed() {
    let source = carrier(&[("mousedown", EXTRACTED_PUT_MOUSEUP)]);
    assert_eq!(claims(&source, MOUSE_DOWN, DOWN_EVALUATOR), 1, "{source}");
    assert_eq!(applied(&source, MOUSE_DOWN, DOWN_EVALUATOR), 1, "{source}");
    assert!(refused(&source, MOUSE_UP), "{source}");

    let source = carrier(&[("pointerup", EXTRACTED_PUT_MOUSEUP)]);
    assert!(refused(&source, MOUSE_UP), "{source}");
    assert!(refused(&source, MOUSE_DOWN), "{source}");
    assert!(refused(&source, MOUSE_CLICK), "{source}");
}

#[test]
fn alias_near_misses_are_refused() {
    let cases = [
        // wrong `this`: the receiver is another stable ring.
        ("$CP.call($CO, $CQ);", "$CP.call($KEY_34, $CQ);"),
        // the receiver is an alias of another stable ring.
        (
            "let $CO = $KEY_33;",
            "let $CO = $KEY_34;",
        ),
        // wrong member: the extracted value is not the exact `.put`.
        ("let $CP = $KEY_33.put;", "let $CP = $KEY_33.push;"),
        // no generic `.call` widening: the extracted value is `.call` itself.
        ("let $CP = $KEY_33.put;", "let $CP = $KEY_33.call;"),
        // overwritten alias: a second write means it is not single-origin.
        (
            "$CP.call($CO, $CQ);",
            "$CP = $KEY_33.put;\n$CP.call($CO, $CQ);",
        ),
        // foreign ring: the alias belongs to another ring than the receiver.
        ("let $CP = $KEY_33.put;", "let $CP = $KEY_34.put;"),
        // unstable ring: a second write means the ring is not stable.
        (
            "$CP.call($CO, $CQ);",
            "$KEY_33 = $KEY_34;\n$CP.call($CO, $CQ);",
        ),
        // missing row argument: `.call(receiver)` is not the row call.
        ("$CP.call($CO, $CQ);", "$CP.call($CO);"),
        // extra argument: `.call(receiver, row, extra)` is not the row call.
        ("$CP.call($CO, $CQ);", "$CP.call($CO, $CQ, $CQ);"),
        // donor: the only alias call is nested, not owned by the handler.
        (
            "$CP.call($CO, $CQ);",
            "function donor() { let $CP = $KEY_33.put; $CP.call($CO, $CQ); }",
        ),
    ];
    for (from, to) in cases {
        let body = EXTRACTED_PUT_MOUSEUP.replace(from, to);
        assert_ne!(body, EXTRACTED_PUT_MOUSEUP, "near miss {from:?} did not apply");
        let source = mouseup(&body);
        assert!(
            refused(&source, MOUSE_UP),
            "accepted near miss {from:?} -> {to:?}:\n{source}"
        );
    }
}

#[test]
fn late_alias_definition_is_refused() {
    let body = EXTRACTED_PUT_MOUSEUP
        .replace("let $CP = $KEY_33.put;", "")
        .replace(
            "$CP.call($CO, $CQ);",
            "$CP.call($CO, $CQ);\nlet $CP = $KEY_33.put;",
        );
    assert!(body.contains("$CP.call($CO, $CQ);\nlet $CP = $KEY_33.put;"));
    let source = mouseup(&body);
    assert!(refused(&source, MOUSE_UP), "{source}");
}

#[test]
fn branch_near_miss_is_refused() {
    // The base case already proves the alias; putting the whole definition
    // and call under a conditional must not be claimed by the bounded proof.
    let body = EXTRACTED_PUT_MOUSEUP
        .replace("let $CO = $KEY_33;", "if ($KEY_32) { let $CO = $KEY_33;")
        .replace("$CP.call($CO, $CQ);", "$CP.call($CO, $CQ); }");
    assert!(body.contains("if ($KEY_32) {"));
    let source = mouseup(&body);
    assert!(refused(&source, MOUSE_UP), "{source}");
}
