use shape::naming::name_threads;

// The pattern is declared without initializer and assigned after the predicate
// text, matching the recovered capture. The alias proves that no predicate use
// can occur before the assignment.
const LATE: &str = r#"let matcher=RegExp("{\\s*\\[\\s*native\\s+code\\s*]\\s*}\\s*$");let pattern;
function $THREAD_71_8123(value){if(typeof value === "function" && value.toString === Function.prototype.toString){return !!pattern.exec(value.toString());}else{return false;}}
let invoke=$THREAD_71_8123;pattern=matcher;invoke(Function.prototype.toString);"#;

// A direct initializer is deliberately not this rule's family: the shared
// runtime already proves the ordered form. Here the producer text follows the
// use, so neither proof may accept it.
const INIT_ONLY: &str = r#"function main(){let pattern=matcher;let matcher=RegExp("{\\s*\\[\\s*native\\s+code\\s*]\\s*}\\s*$");
function $THREAD_71_8123(value){if(typeof value === "function" && value.toString === Function.prototype.toString){return !!pattern.exec(value.toString());}else{return false;}}
$THREAD_71_8123(Function.prototype.toString);return pattern;}main();"#;

const COLLISION: &str = r#"function main(){let isNativeFunction=42;let isNativeFunction2=43;let matcher=RegExp("{\\s*\\[\\s*native\\s+code\\s*]\\s*}\\s*$");let pattern;
function $THREAD_900_777_2(value){if(typeof value === "function" && value.toString === Function.prototype.toString){return !!pattern.exec(value.toString());}else{return false;}}
pattern=matcher;$THREAD_900_777_2(Function.prototype.toString);return [isNativeFunction,isNativeFunction2];}main();"#;

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.outcomes.iter().any(|o| {
            o.applied && o.binding.name == target && o.proposed_name.as_deref() == Some(name)
        })
    })
}

/// A negative source must still parse and analyze; only the naming proof is
/// expected to fail. This keeps refusal tests from passing vacuously.
fn refused(source: &str, target: &str, name: &str) {
    let result = name_threads(source, true).unwrap_or_else(|error| {
        panic!("negative source must parse and analyze: {error}\n{source}")
    });
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes:\n{source}"));
    assert!(
        !(outcome.applied && outcome.proposed_name.as_deref() == Some(name)),
        "accepted {target} as {name}:\n{source}"
    );
}

fn refuses(template: &str, target: &str, name: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        refused(&template.replace(from, to), target, name);
    }
}

#[test]
fn late_native_producer_is_named_and_alpha_round_trips() {
    assert!(named(LATE, "$THREAD_71_8123", "isNativeFunction"));

    let alpha = LATE
        .replace("$THREAD_71_8123", "$THREAD_900_777_2")
        .replace("matcher", "$KEY_900")
        .replace("pattern", "$KEY_901")
        .replace("value", "$KEY_902")
        .replace("invoke", "$KEY_903");
    assert!(named(&alpha, "$THREAD_900_777_2", "isNativeFunction"));

    // No alias at all: the sole predicate reference is already after the
    // assignment, so the use-order proof also holds.
    let direct = LATE.replace(
        "let invoke=$THREAD_71_8123;pattern=matcher;invoke(Function.prototype.toString);",
        "pattern=matcher;$THREAD_71_8123(Function.prototype.toString);",
    );
    assert!(named(&direct, "$THREAD_71_8123", "isNativeFunction"));
}

#[test]
fn native_predicate_requires_exact_guard_operations_and_returns() {
    refuses(
        LATE,
        "$THREAD_71_8123",
        "isNativeFunction",
        &[
            ("typeof value", "typeof other"),
            ("=== \"function\"", "=== \"object\""),
            ("value.toString ===", "other.toString ==="),
            ("&&", "||"),
            ("Function.prototype.toString", "Function.prototype.call"),
            ("pattern.exec", "pattern.test"),
            ("return !!pattern", "return !pattern"),
            ("return false", "return true"),
            (
                "pattern.exec(value.toString())",
                "pattern.exec(value.toString().bind(value))",
            ),
            (
                "pattern.exec(value.toString())",
                "pattern.exec(value.toString(),1)",
            ),
        ],
    );
}

#[test]
fn native_producer_requires_straight_line_scoped_origin_and_use_order() {
    refused(INIT_ONLY, "$THREAD_71_8123", "isNativeFunction");
    refuses(
        LATE,
        "$THREAD_71_8123",
        "isNativeFunction",
        &[
            ("pattern=matcher;", "if(invokeCondition){pattern=matcher;}"),
            (
                "pattern=matcher;",
                "for(;invokeCondition;){pattern=matcher;}",
            ),
            ("pattern=matcher;", "pattern=matcher;pattern=matcher;"),
            ("pattern=matcher;invoke(", "invoke("),
            ("pattern=matcher;invoke(", "invoke("),
            ("native", "host"),
            ("RegExp(", "new RegExp("),
            (
                "let matcher=RegExp",
                "let RegExp=function(){};let matcher=RegExp",
            ),
            (
                "function $THREAD_71_8123(value)",
                "function $THREAD_71_8123(value,other)",
            ),
            (
                "function $THREAD_71_8123(value)",
                "function $THREAD_71_8123({value})",
            ),
            (
                "function $THREAD_71_8123(value)",
                "function $THREAD_71_8123(...value)",
            ),
            (
                "let invoke=$THREAD_71_8123;",
                "let invoke=$THREAD_71_8123;invoke=other;",
            ),
            (
                "pattern=matcher;invoke(",
                "invoke(Function.prototype.toString);pattern=matcher;//",
            ),
        ],
    );
    // Flags make the native-code match stateful and are not the stable pattern.
    let flagged = LATE.replace(r#"$");let pattern;"#, r#"$","g");let pattern;"#);
    refused(&flagged, "$THREAD_71_8123", "isNativeFunction");
}

#[test]
fn collision_blocks_suffixed_generated_role() {
    // The role and its suffixed spelling are taken in the same scope; neither
    // may be applied over an existing binding.
    refused(COLLISION, "$THREAD_900_777_2", "isNativeFunction");
    refused(COLLISION, "$THREAD_900_777_2", "isNativeFunction2");
}
