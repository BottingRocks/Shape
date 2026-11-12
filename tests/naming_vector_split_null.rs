use shape::naming::name_threads;

const SOURCE: &str = include_str!("fixtures/program/vector-split-null/owned.js");
const ROLE: &str = "recordPixelVectorSample";

fn claims(source: &str) -> bool {
    name_threads(source, true)
        .expect("fixture must parse")
        .report
        .function_roles
        .iter()
        .any(|role| {
            role.candidates.iter().any(|name| name == ROLE)
                && role
                    .evaluators
                    .iter()
                    .any(|name| name == "graphics_unique_vector_history")
        })
}

#[test]
fn positive_split_null_owned_loop_and_exact_replay() {
    for source in [
        SOURCE.to_string(),
        SOURCE
            .replace("$KEY_0", "vector")
            .replace("$AX", "missing")
            .replace("$AY", "cursor")
            .replace("$AZ", "equal")
            .replace("$THREAD_2_100633", "$THREAD_91_812"),
        SOURCE.replace("let $AX;", "var $AX;"),
    ] {
        assert!(claims(&source), "{source}");
        let result = name_threads(&source, true).unwrap();
        assert!(result
            .report
            .outcomes
            .iter()
            .any(|o| o.applied && o.role_name.as_deref() == Some(ROLE)));
        let mut replay = source.clone();
        let mut edits: Vec<_> = result.report.edits.iter().collect();
        edits.sort_by_key(|e| std::cmp::Reverse(e.start));
        for edit in edits {
            replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
        }
        assert_eq!(replay, result.source);
    }
}

#[test]
fn negative_split_null_wrong_branches_bindings_and_extra_effects() {
    for (from, to) in [
        ("$KEY_0 == null", "$AW == null"),
        ("$AX = true", "$AX = false"),
        ("$AX = $AW == null", "$AX = $KEY_0 == null"),
        ("$AX = $AW == null", "$AX = $AW != null"),
        ("$AX = $AW == null", "$AX = $AW === null"),
        ("$AX ||", "$AW ||"),
        ("$AX ||", "$AX &&"),
        ("let $AX;", "let $AX = false;"),
        ("$AX = true;", "$AX = true; effect();"),
        ("$AX = $AW == null;", "$AX = $AW == null; effect();"),
        (
            "$KEY_0.length !== $AW.length",
            "$KEY_0.length === $AW.length",
        ),
        ("$AY >= $KEY_0.length", "$AY > $KEY_0.length"),
        ("$KEY_0[$AY] === $AW[$AY]", "$KEY_0[$AY] == $AW[$AY]"),
        ("++$AY", "--$AY"),
        ("$AZ = true", "$AZ = false"),
        ("$AZ = false", "$AZ = true"),
        ("return $AZ;", "return $AX;"),
        ("$AV.b = Number($AV.b) + 1", "$AV.b = Number($AV.a) + 1"),
        (
            "Array.prototype.slice.call($KEY_0)",
            "Array.prototype.slice.call($AV)",
        ),
        ("$AV.a.push(", "$AV.b.push("),
        ("$AV.a.length > 5", "$AV.b.length > 5"),
        ("$AV.a.some(", "$AV.b.some("),
    ] {
        let source = SOURCE.replace(from, to);
        assert_ne!(source, SOURCE, "mutation must hit: {from}");
        assert!(!claims(&source), "{source}");
    }
}

#[test]
fn negative_split_null_cross_callable_donors_shadowing_and_clobbers() {
    for (from, to) in [
        ("$AX = true;", "function donor(){$AX = true;}"),
        ("$AX = $AW == null;", "function donor(){$AX = $AW == null;}"),
        ("$AX = true;", "let $AX = true;"),
        ("$AX = $AW == null;", "let $AX = $AW == null;"),
        ("$AX = true;", "$AX = true; $AX = false;"),
        ("$AX = $AW == null;", "if(other){$AX = $AW == null;}"),
        ("$AX = true;", "if(other){$AX = true;}"),
        ("let $AX;", "let $AX; function donor(){$AX=false;}"),
        ("$AX = true;", "$KEY_0 = []; $AX = true;"),
        ("$AX = $AW == null;", "$AW = []; $AX = $AW == null;"),
        ("let $AX;", "const donor=()=>{let $AX;return $AX;};let $AX;"),
    ] {
        let source = SOURCE.replace(from, to);
        assert_ne!(source, SOURCE);
        assert!(!claims(&source), "{source}");
    }
}
