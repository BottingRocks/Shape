//! Focused tests for the rolling-frontier rule `recordPixelVectorSample`. Every fixture is
//! a small, portable, reachable `main` wrapper.

use shape::naming::name_threads;

fn named(source: &str, name: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| o.applied && o.proposed_name.as_deref() == Some(name))
}

fn unchanged(source: &str) -> bool {
    name_threads(source, true).unwrap().source == source
}

// let copy = record;
// if (state.a.some(stored => { let equal; if (copy == null || stored == null ||
//   copy.length !== stored.length) { equal = false; } else { let cursor = 0;
//   for (;;) { if (cursor >= copy.length) { equal = true; break; }
//     if (copy[cursor] === stored[cursor]) { ++cursor; continue; }
//     equal = false; break; } } return equal; })) return void 0;
// state.b = Number(state.b) + 1;
// if (state.a.length > 5) return void 0;
// else { state.a.push(Array.prototype.slice.call(copy)); return void 0; }
const SAMPLE: &str = r#"function main(){function $THREAD_6_74912(record,state){let copy=record;if(state.a.some(stored=>{let equal;if(copy==null||stored==null||copy.length!==stored.length){equal=false;}else{let cursor=0;for(;;){if(cursor>=copy.length){equal=true;break;}if(copy[cursor]===stored[cursor]){++cursor;continue;}equal=false;break;}}return equal;})){return void 0;}state.b=Number(state.b)+1;if(state.a.length>5){return void 0;}else{state.a.push(Array.prototype.slice.call(copy));return void 0;}}return $THREAD_6_74912;}main();"#;

#[test]
fn record_sample_proves_dedupe_bump_and_bound() {
    assert!(named(SAMPLE, "recordPixelVectorSample"), "{SAMPLE}");
    // Generated-id and alpha-renamed variant: distinct member keys, bound and
    // every local/parameter spelling change together.
    let alpha = SAMPLE
        .replace("$THREAD_6_74912", "$THREAD_2_31281")
        .replace("state.a", "state.samples")
        .replace("state.b", "state.count")
        .replace("record", "sample")
        .replace("copy", "held")
        .replace("stored", "other")
        .replace("equal", "same")
        .replace("cursor", "i")
        .replace(">5", ">12");
    assert!(named(&alpha, "recordPixelVectorSample"), "{alpha}");
    // A readable source name is preserved, not renamed.
    let human = SAMPLE.replace("$THREAD_6_74912", "keepUniqueVectorSample");
    assert!(unchanged(&human), "{human}");
}

#[test]
fn record_sample_refuses_changed_members_guards_and_shapes() {
    for (from, to) in [
        // Counter slot read differs from the counter member written.
        ("Number(state.b)+1", "Number(state.a)+1"),
        // Counter member collapses onto the list member.
        ("state.b=Number(state.b)+1", "state.a=Number(state.a)+1"),
        // Loose length comparison.
        ("copy.length!==stored.length", "copy.length!=stored.length"),
        // Loose element comparison.
        (
            "copy[cursor]===stored[cursor]",
            "copy[cursor]==stored[cursor]",
        ),
        // Null short-circuit guard removed.
        (
            "copy==null||stored==null||copy.length!==stored.length",
            "copy.length!==stored.length",
        ),
        // End guard accepts the end index.
        ("if(cursor>=copy.length)", "if(cursor>copy.length)"),
        // Element compare uses a foreign second index.
        (
            "copy[cursor]===stored[cursor]",
            "copy[cursor]===stored[index]",
        ),
        // Some predicate replaced by a different iterator method.
        ("state.a.some(stored=>", "state.a.filter(stored=>"),
        // Loop gains a test/init, so it is not the guarded infinite scan.
        ("for(;;)", "for(let k=0;;)"),
        // Predicate returns a constant instead of the accumulated equality.
        ("return equal;", "return true;"),
        // Counter increment removed (statement list no longer matches).
        ("state.b=Number(state.b)+1;", ""),
        // Bound is not a positive numeric literal.
        ("state.a.length>5", "state.a.length>limit"),
        // Cap guard loses its else branch.
        (
            "if(state.a.length>5){return void 0;}else{",
            "if(state.a.length>5){return void 0;}{",
        ),
        // Appends the raw formal vector instead of an Array.prototype.slice copy.
        (
            "state.a.push(Array.prototype.slice.call(copy))",
            "state.a.push(copy)",
        ),
        // Slice is invoked through .apply, not receiver-preserving .call.
        ("Array.prototype.slice.call", "Array.prototype.slice.apply"),
        // Appended value is not the Array.prototype.slice path at all.
        ("Array.prototype.slice.call(copy)", "copy.slice()"),
        // Predicate formal has a default initializer.
        ("stored=>{", "stored=other=>{"),
        // Copy local is reassigned after its declaration.
        ("let copy=record;", "let copy=record;copy=other;"),
    ] {
        assert!(
            !named(&SAMPLE.replace(from, to), "recordPixelVectorSample"),
            "{from} -> {to}"
        );
    }
}

#[test]
fn record_sample_requires_plain_formals_and_refuses_a_shadowed_name() {
    // A default/rest formal is not a plain two-slot signature.
    let one_param = SAMPLE.replace("(record,state)", "(record)");
    assert!(!named(&one_param, "recordPixelVectorSample"));

    // An existing binding of the target name blocks the rename.
    let collision = SAMPLE.replace(
        "function main(){",
        "function main(){let recordPixelVectorSample=1;use(recordPixelVectorSample);",
    );
    assert!(!named(&collision, "recordPixelVectorSample"));

    // Unrelated readable functions keep their names.
    let preserved = r#"function main(){function readableOne(){return 1;}function readableTwo(){return 2;}return [readableOne,readableTwo];}main();"#;
    assert!(unchanged(preserved));
}
