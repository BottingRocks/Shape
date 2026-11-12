use shape::naming::labels::build_label_reference;
use shape::naming::{name_threads, name_threads_with_labels};

fn named(source: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .outcomes
            .iter()
            .any(|o| o.applied && o.proposed_name.as_deref() == Some(name))
    })
}

fn replay_matches(source: &str) -> bool {
    let Ok(report) = name_threads(source, true) else {
        return false;
    };
    let mut edits = report.report.edits.clone();
    edits.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
    let mut replay = source.to_owned();
    for edit in edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    replay == report.source
}

// The wide XOR/shift mixer body the independent stream-cipher evaluator proves
// from its own structure.
fn mixer() -> String {
    let mut ops = String::new();
    for k in 1..=16 {
        if k > 1 {
            ops.push('^');
        }
        ops.push_str(&format!("(a[i]^b[i])>>>{k}"));
    }
    ops
}

// A reachable zero-input factory: a pure numeric prelude, one nested mixer
// function, and that function returned directly. `main` calls the factory so
// every site is reachable exactly like the seed.
fn factory(outer: &str, inner: &str, prelude: &str) -> String {
    format!(
        "function main(){{function {outer}(){{{prelude}function {inner}(a,b,c,d){{let out=Array(a.length);let i=0;while(i<out.length){{out[i]={};++i;}}Math.ceil(a.length/64);return out;}}return {inner};}}return {outer};}}main();",
        mixer()
    )
}

fn cipher_source() -> String {
    factory("$THREAD_1_1", "$THREAD_2_2", "let base=Math.pow(2,32);")
}

#[test]
fn stream_cipher_factory_is_proven() {
    let base = cipher_source();
    assert!(
        named(&base, "applyStreamCipher"),
        "inner not independently proven"
    );
    assert!(named(&base, "createChaChaLikeStreamCipher"), "{base}");
    assert!(replay_matches(&base));
    for source in [
        // Alpha-renamed copy of the same owned body.
        factory(
            "$THREAD_5_200_2",
            "$THREAD_6_300",
            "let $KEY_7=Math.pow(2,32);",
        ),
        // `const` and explicit pure arithmetic over two unshadowed pow calls.
        factory(
            "$THREAD_3_7",
            "$THREAD_4_8",
            "const scale=Math.pow(2,16)*Math.pow(2,16);",
        ),
        // `var`, an earlier proven prelude constant, and a bitwise fold.
        factory(
            "$THREAD_9_1",
            "$THREAD_10_2",
            "var p = 1 << 2; let base = Math.pow(p + 2, 32) | 0;",
        ),
    ] {
        assert!(named(&source, "createChaChaLikeStreamCipher"), "{source}");
        assert!(replay_matches(&source));
    }
}

#[test]
fn stream_cipher_factory_refuses_weaker_near_misses() {
    let base = cipher_source();
    for (from, to) in [
        // Changed prelude: no unshadowed numeric `Math.pow` remains.
        ("Math.pow(2,32)", "Math.pow(2)"),
        ("Math.pow(2,32)", "Math.pow(2,32,1)"),
        ("Math.pow(2,32)", "Math.random()"),
        ("Math.pow(2,32)", "2+2"),
        // Changed formals: the factory no longer takes zero inputs.
        ("function $THREAD_1_1(){", "function $THREAD_1_1(input){"),
        ("function $THREAD_1_1(){", "function $THREAD_1_1(...input){"),
        // Changed child arity breaks its independent four-slot proof.
        (
            "function $THREAD_2_2(a,b,c,d)",
            "function $THREAD_2_2(a,b,c)",
        ),
        // Extra effectful initializer/statement.
        (
            "let base=Math.pow(2,32);",
            "let base=Math.pow(2,32);useValue(base);",
        ),
        (
            "let base=Math.pow(2,32);",
            "let base=Math.pow(2,32), extra=useValue(base);",
        ),
        // Changed return: not that nested function, or not returned directly.
        ("return $THREAD_2_2;", "return base;"),
        ("return $THREAD_2_2;", "return $THREAD_2_2();"),
        // Child rebound through an indirection, an assignment, or a
        // function-expression declaration.
        (
            "return $THREAD_2_2;",
            "let cipher=$THREAD_2_2;return cipher;",
        ),
        (
            "return $THREAD_2_2;",
            "$THREAD_2_2=null;return $THREAD_2_2;",
        ),
        (
            "function $THREAD_2_2(a,b,c,d){",
            "let $THREAD_2_2=function(a,b,c,d){",
        ),
        // Unsupported declarations: class, destructuring, arrow initializer,
        // and a second nested function.
        (
            "function $THREAD_2_2(a,b,c,d){",
            "class Box{}function $THREAD_2_2(a,b,c,d){",
        ),
        ("let base=Math.pow(2,32);", "let [base]=[Math.pow(2,32)];"),
        ("let base=Math.pow(2,32);", "let base=()=>Math.pow(2,32);"),
        (
            "let base=Math.pow(2,32);",
            "let base=Math.pow(2,32);function extra(){}",
        ),
        // A resource declaration is not a plain local prelude binding.
        ("let base=Math.pow(2,32);", "using base=Math.pow(2,32);"),
        // Shadowed `Math` is not the global numeric prelude API.
        (
            "function main(){",
            "function main(){let Math={pow:function(){return 1;}};",
        ),
    ] {
        let source = base.replace(from, to);
        assert!(
            !named(&source, "createChaChaLikeStreamCipher"),
            "accepted {from} -> {to}"
        );
    }
    // A weaker mixer is not independently proven, so its holder is refused.
    let weak = base.replace("Math.ceil(a.length/64);", "");
    assert!(!named(&weak, "applyStreamCipher"));
    assert!(!named(&weak, "createChaChaLikeStreamCipher"));
    // Naming the child `applyStreamCipher` in source is a spelling, never proof.
    let spelled = base
        .replace("Math.ceil(a.length/64);", "")
        .replace("$THREAD_2_2", "applyStreamCipher");
    assert!(!named(&spelled, "createChaChaLikeStreamCipher"));
}

#[test]
fn inherited_label_alone_is_not_proof() {
    // The same structurally weak mixer; only the labeled copy names it
    // `applyStreamCipher`, so the independent evaluator still refuses it.
    let weak = |inner: &str| {
        format!(
            "function main(){{function $THREAD_1_1(){{let base=Math.pow(2,32);function {inner}(a,b,c,d){{let out=Array(a.length);let i=0;while(i<out.length){{out[i]=(a[i]^b[i])>>>1;++i;}}return out;}}return {inner};}}return $THREAD_1_1;}}main();"
        )
    };
    let original = weak("$THREAD_2_2");
    let labeled = weak("applyStreamCipher");
    let reference = build_label_reference(&original, &labeled).unwrap();
    let report = name_threads_with_labels(&original, true, Some(&reference)).unwrap();
    // The child holds only an inherited reference label ...
    assert!(report.report.outcomes.iter().any(|o| {
        o.proposed_name.as_deref() == Some("applyStreamCipher")
            && o.evaluator.as_deref() == Some("inherited_structural_label")
    }));
    // ... which is not an independent source proof, so the factory stays unnamed.
    assert!(!report
        .report
        .outcomes
        .iter()
        .any(|o| o.applied && o.proposed_name.as_deref() == Some("createChaChaLikeStreamCipher")));
}

#[test]
fn existing_names_are_preserved_and_collisions_refuse() {
    let existing = factory(
        "createChaChaLikeStreamCipher",
        "applyStreamCipher",
        "let base=Math.pow(2,32);",
    );
    assert_eq!(name_threads(&existing, true).unwrap().source, existing);
    let collision = cipher_source().replace(
        "function main(){",
        "function main(){let createChaChaLikeStreamCipher=17;useValue(createChaChaLikeStreamCipher);",
    );
    assert!(!named(&collision, "createChaChaLikeStreamCipher"));
}
