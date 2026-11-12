//! Focused, portable coverage for mergeUrlSearchParameter.
//!
//! The exact builder is wrapped in a minimal carrier. Alpha spellings and both source node
//! shapes for split and append branches must still claim. Negatives change operations,
//! inputs, receivers, encoding, keys, ports, hashes, callbacks, or donors and must stay
//! unclaimed by thread_url_query_merge. This target contains no absolute paths or cached
//! expected counts.
use shape::naming::name_threads;

/// The representative builder body preserves its exact UTF-8 source bytes.
const ACTUAL_BODY: &str = r##"
function $THREAD_4_127206($AO, $AP, $AQ) {
			let $KEY_7 = "";
			let $KEY_8 = $KEY_322($AO);
			let $AR;
			if ($AO == null || $AO.trim() === "") {
				$AR = [];
			} else {
				let $AS = document.createElement("a");
				$AS.href = $AO;
				let $AT = $AS.search.slice(1);
				if ($AT === "") {
					$AR = [];
				} else {
					$AT = [$AT, $AT.split];
					$AR = $AT[1].call($AT[0], "&").map($AU => {
						let $KEY_1 = $AU.split("=");
						const $AV = typeof $KEY_1[0] === "undefined" ? "undefined" : $KEY_1[0] === null ? "null" : decodeURIComponent($KEY_1[0].toString()).replace(new RegExp(RegExp("\\+", "g")), " ");
						let $AW;
						if (typeof $KEY_1[1] === "undefined") {
							$AW = "";
						} else {
							$AW = typeof $KEY_1[1] === "undefined" ? "undefined" : $KEY_1[1] === null ? "null" : decodeURIComponent($KEY_1[1].toString()).replace(new RegExp(RegExp("\\+", "g")), " ");
						}
						$AW = [$AV, $AW];
						return $AW;
					});
				}
			}
			let $KEY_9 = typeof $AP === "undefined" ? "undefined" : $AP === null ? "null" : encodeURIComponent($AP.toString()).replace(new RegExp(RegExp("%20", "g")), "+");
			let $KEY_0 = typeof $AQ === "undefined" ? "undefined" : $AQ === null ? "null" : encodeURIComponent($AQ.toString()).replace(new RegExp(RegExp("%20", "g")), "+");
			if ($AR.length > 0) {
				let $KEY_12 = false;
				$AR.forEach(($AU, $AV) => {
					const $AW = typeof $AU[0] === "undefined" ? "undefined" : $AU[0] === null ? "null" : encodeURIComponent($AU[0].toString()).replace(new RegExp(RegExp("%20", "g")), "+");
					const $AX = typeof $AU[1] === "undefined" ? "undefined" : $AU[1] === null ? "null" : encodeURIComponent($AU[1].toString()).replace(new RegExp(RegExp("%20", "g")), "+");
					if ($AW === $KEY_9 && !$KEY_12) {
						$KEY_7 = $KEY_7 + (($AV === 0 ? $KEY_9 : "&" + $KEY_9) + "=" + $KEY_0);
						$KEY_12 = true;
					} else if ($AW !== $KEY_9) {
						$KEY_7 = $KEY_7 + (($AV === 0 ? $AW + "=" : "&" + $AW + "=") + $AX);
					}
				});
				if (!$KEY_12) {
					$KEY_7 = $KEY_7 + ("&" + $KEY_9 + "=" + $KEY_0);
				}
			} else {
				$KEY_7 = $KEY_9 + "=" + $KEY_0;
			}
			return $KEY_8.protocol + "://" + $KEY_8.hostname + ($KEY_8.port.length > 0 ? ":" + $KEY_8.port : "") + $KEY_8.pathname + ($KEY_7.length > 0 ? "?" + $KEY_7 : "") + ($KEY_8.hash.length > 0 ? "#" + $KEY_8.hash : "");
		}
"##;

/// A minimal carrier: only the captured parser and the main scope. The parser
/// is a captured single-origin callable that returns a URL-shaped object; it is
/// not the shape under test.
fn frame(body: &str) -> String {
    let mut source = String::new();
    source.push_str("function main(){\n");
    // One carrier binding keeps the main scope broader than the one-parameter
    // parser scope, exactly as the real capture's outer scope is broader.
    source.push_str("let $carrier = 0;\n");
    source.push_str("function $KEY_322($AO){ let $AS = {protocol:$AO,hostname:$AO,port:$AO,pathname:$AO,search:$AO,hash:$AO,href:$AO}; return $AS; }\n");
    source.push_str(body);
    source.push_str("\nreturn $THREAD_4_127206;}main();");
    source
}

fn actual() -> String {
    frame(ACTUAL_BODY)
}

fn accepted(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.function_roles.iter().any(|role| {
            role.candidates
                .iter()
                .any(|candidate| candidate == "mergeUrlSearchParameter")
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| evaluator == "thread_url_query_merge")
        })
    })
}

fn claims_with(source: &str, evaluator: &str) -> usize {
    name_threads(source, true)
        .map(|result| {
            result
                .report
                .function_roles
                .iter()
                .filter(|role| role.evaluators.iter().any(|e| e == evaluator))
                .count()
        })
        .unwrap_or(0)
}

fn replay(source: &str) -> String {
    let report = name_threads(source, true).unwrap();
    let mut replay = source.to_owned();
    let mut edits = report.report.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    replay
}

fn direct_split() -> String {
    let without_split_trick = ACTUAL_BODY.replace("$AT = [$AT, $AT.split];", "");
    let direct = without_split_trick.replace("$AT[1].call($AT[0], \"&\")", "$AT.split(\"&\")");
    frame(&direct)
}

fn function_callbacks() -> String {
    let mutated = ACTUAL_BODY
        .replace("$AU => {", "function ($AU) {")
        .replace("($AU, $AV) => {", "function ($AU, $AV) {");
    frame(&mutated)
}

fn branch_complete_append() -> String {
    let mutated = ACTUAL_BODY.replace(
        "(($AV === 0 ? $AW + \"=\" : \"&\" + $AW + \"=\") + $AX)",
        "($AV === 0 ? $AW + \"=\" + $AX : \"&\" + $AW + \"=\" + $AX)",
    );
    frame(&mutated)
}

fn if_style_for_each_value() -> String {
    let mutated = ACTUAL_BODY.replace(
        "const $AX = typeof $AU[1] === \"undefined\" ? \"undefined\" : $AU[1] === null ? \"null\" : encodeURIComponent($AU[1].toString()).replace(new RegExp(RegExp(\"%20\", \"g\")), \"+\");",
        "let $AX;\n\t\t\t\t\tif (typeof $AU[1] === \"undefined\") {\n\t\t\t\t\t\t$AX = \"undefined\";\n\t\t\t\t\t} else {\n\t\t\t\t\t\t$AX = $AU[1] === null ? \"null\" : encodeURIComponent($AU[1].toString()).replace(new RegExp(RegExp(\"%20\", \"g\")), \"+\");\n\t\t\t\t\t}",
    );
    frame(&mutated)
}

fn wrong_old_entry_value() -> String {
    let mutated = ACTUAL_BODY.replace(
        "(($AV === 0 ? $AW + \"=\" : \"&\" + $AW + \"=\") + $AX)",
        "(($AV === 0 ? $AW + \"=\" : \"&\" + $AW + \"=\") + $KEY_0)",
    );
    frame(&mutated)
}

fn wrong_injected_replacement_value() -> String {
    let mutated = ACTUAL_BODY.replace(
        "(($AV === 0 ? $KEY_9 : \"&\" + $KEY_9) + \"=\" + $KEY_0)",
        "(($AV === 0 ? $KEY_9 : \"&\" + $KEY_9) + \"=\" + $AX)",
    );
    frame(&mutated)
}

fn donor_field_branch() -> String {
    let mutated = ACTUAL_BODY.replace(
        "const $AX = typeof $AU[1] === \"undefined\" ? \"undefined\" : $AU[1] === null ? \"null\" : encodeURIComponent($AU[1].toString()).replace(new RegExp(RegExp(\"%20\", \"g\")), \"+\");",
        "const $AX = typeof $AU[0] === \"undefined\" ? \"undefined\" : $AU[0] === null ? \"null\" : encodeURIComponent($AU[0].toString()).replace(new RegExp(RegExp(\"%20\", \"g\")), \"+\");",
    );
    frame(&mutated)
}

fn hoisted_map_callback() -> String {
    let marker = ".map($AU => {";
    let start = ACTUAL_BODY.find(marker).expect("map callback marker");
    let body_start = start + marker.len();
    let close = ACTUAL_BODY[body_start..]
        .find("});")
        .expect("map callback close");
    let callback_body = &ACTUAL_BODY[body_start..body_start + close];
    let mutated = format!(
        "{}.map($decodePair);{}",
        &ACTUAL_BODY[..start],
        &ACTUAL_BODY[body_start + close + 3..]
    );
    let helper = format!("let $decodePair = ($AU) => {{{callback_body}}};\n");
    frame(&format!("{helper}{mutated}"))
}

fn nested_parser() -> String {
    let mutated = ACTUAL_BODY.replace(
        "let $KEY_8 = $KEY_322($AO);",
        "let $KEY_8 = (($Z) => { let $AS = {protocol:$Z,hostname:$Z,port:$Z,pathname:$Z,search:$Z,hash:$Z,href:$Z}; return $AS; })($AO);",
    );
    frame(&mutated)
}

fn partial_return() -> String {
    let marker = "return $KEY_8.protocol";
    let start = ACTUAL_BODY.find(marker).expect("rebuild marker");
    let end = ACTUAL_BODY[start..].find(';').expect("rebuild close") + start + 1;
    let mutated = format!(
        "{}return $KEY_7;{}",
        &ACTUAL_BODY[..start],
        &ACTUAL_BODY[end..]
    );
    frame(&mutated)
}

#[test]
fn actual_representative_body_is_claimed_once() {
    let source = actual();
    assert!(accepted(&source), "{source}");
    assert_eq!(claims_with(&source, "thread_url_query_merge"), 1);
    assert_eq!(replay(&source), name_threads(&source, true).unwrap().source);
}

#[test]
fn alpha_and_node_shape_variants_are_claimed() {
    let alpha = actual().replace('$', "$Z_");
    for source in [
        alpha,
        direct_split(),
        function_callbacks(),
        branch_complete_append(),
        if_style_for_each_value(),
    ] {
        assert!(accepted(&source), "{source}");
        assert_eq!(replay(&source), name_threads(&source, true).unwrap().source);
    }
}

#[test]
fn wrong_input_receiver_encoding_key_port_and_hash_are_refused() {
    let sources = [
        // wrong input: the anchor parses the injected name instead of the URL
        frame(&ACTUAL_BODY.replace("$AS.href = $AO;", "$AS.href = $AP;")),
        // wrong receiver: the rebuilt URL is not the captured parser result
        frame(&ACTUAL_BODY.replace("$KEY_8.protocol", "$AR.protocol")),
        // wrong encoding: the injected components use a different pattern
        frame(&ACTUAL_BODY.replace("RegExp(\"%20\", \"g\")", "RegExp(\"%2B\", \"g\")")),
        // wrong key: the equal-name branch compares the encoded value
        frame(&ACTUAL_BODY.replace(
            "if ($AW === $KEY_9 && !$KEY_12)",
            "if ($AW === $KEY_0 && !$KEY_12)",
        )),
        // wrong port: the optional port loses its separator
        frame(&ACTUAL_BODY.replace(
            "($KEY_8.port.length > 0 ? \":\" + $KEY_8.port : \"\")",
            "($KEY_8.port.length > 0 ? $KEY_8.port : \"\")",
        )),
        // wrong hash: the fragment reads the parsed search
        frame(&ACTUAL_BODY.replace(
            "($KEY_8.hash.length > 0 ? \"#\" + $KEY_8.hash : \"\")",
            "($KEY_8.hash.length > 0 ? \"#\" + $KEY_8.search : \"\")",
        )),
    ];
    for source in sources {
        assert!(!accepted(&source), "accepted negative:\n{source}");
    }
}

#[test]
fn existing_value_and_injected_value_identities_are_not_interchangeable() {
    for source in [
        wrong_old_entry_value(),
        wrong_injected_replacement_value(),
        donor_field_branch(),
    ] {
        assert!(!accepted(&source), "accepted near miss:\n{source}");
        assert_eq!(claims_with(&source, "thread_url_query_merge"), 0);
    }
}

#[test]
fn callback_donor_and_nested_parser_are_refused() {
    for source in [hoisted_map_callback(), nested_parser(), partial_return()] {
        assert!(!accepted(&source), "accepted negative:\n{source}");
        assert_eq!(claims_with(&source, "thread_url_query_merge"), 0);
    }
}

#[test]
fn prior_name_collision_and_replay_preserve_contracts() {
    // A prior readable name is retained and the role is still proven.
    let prior = actual().replace("$THREAD_4_127206", "mergeUrlSearchParameter");
    let prior_report = name_threads(&prior, true).unwrap();
    assert!(prior_report.report.function_roles.iter().any(|role| {
        role.current_name.as_deref() == Some("mergeUrlSearchParameter")
            && role
                .evaluators
                .iter()
                .any(|e| e == "thread_url_query_merge")
    }));

    // An existing unsuffixed binding with the role name blocks the edit.
    let collision = actual().replace(
        "function main(){",
        "function main(){let mergeUrlSearchParameter=42;consume(mergeUrlSearchParameter);",
    );
    let collision_report = name_threads(&collision, true).unwrap();
    let outcome = collision_report
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == "$THREAD_4_127206")
        .unwrap();
    assert_eq!(
        outcome.proposed_name.as_deref(),
        Some("mergeUrlSearchParameter")
    );
    assert!(!outcome.applied);
    assert_eq!(outcome.reason.as_deref(), Some("NameCollision"));

    // A byte edit that only renames captured locals is still the same proof.
    let edited = actual()
        .replace("$KEY_8", "$KEY_800")
        .replace("$AO", "$KEY_900");
    assert!(accepted(&edited));
    assert_eq!(replay(&edited), name_threads(&edited, true).unwrap().source);
}
