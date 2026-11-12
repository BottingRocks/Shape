use shape::naming::name_threads;

const MEMORY_BREAKDOWN_SOURCE: &str = include_str!("fixtures/naming/memory_attribution_format/memory-breakdown-baseline.js");
const MEMORY_BREAKDOWN_ALTERNATE: &str = include_str!("fixtures/naming/memory_attribution_format/memory-breakdown-alternate.js");
const START: usize = 434135;
const END: usize = 434662;
const TARGET: &str = "$THREAD_5_137515_2";
const ROLE: &str = "formatMemoryAttributionBreakdown";
const EVALUATOR: &str = "memory_attribution_formatter";

fn claim(source: &str) -> bool {
    let result = name_threads(source, true).expect("whole original scope must parse");
    let target = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == TARGET && o.binding.declaration.start as usize == START)
        .expect("the actual unresolved target must remain indexed");
    target.applied
        && target.role_name.as_deref() == Some(ROLE)
        && target.evaluator.as_deref() == Some(EVALUATOR)
}
fn changed(from: &str, to: &str) -> String {
    let body = &MEMORY_BREAKDOWN_SOURCE[START..END];
    assert!(
        body.contains(from),
        "mutation must hit original full-span target: {from}"
    );
    let replacement = body.replace(from, to);
    assert_ne!(replacement, body);
    format!("{}{}{}", &MEMORY_BREAKDOWN_SOURCE[..START], replacement, &MEMORY_BREAKDOWN_SOURCE[END..])
}
#[test]
fn actual_whole_seed_scopes_prove_exact_unresolved_targets() {
    assert!(claim(MEMORY_BREAKDOWN_SOURCE));
    for (source, starts) in [(MEMORY_BREAKDOWN_SOURCE, [434135, 690793]), (MEMORY_BREAKDOWN_ALTERNATE, [489699, 705271])] {
        let result = name_threads(source, true).unwrap();
        for start in starts {
            let target = result
                .report
                .outcomes
                .iter()
                .find(|o| o.binding.declaration.start == start)
                .unwrap();
            assert!(
                target.applied
                    && target.role_name.as_deref() == Some(ROLE)
                    && target.evaluator.as_deref() == Some(EVALUATOR),
                "{target:?}"
            );
        }
    }
}
#[test]
fn whole_scope_alpha_and_direct_container_variants() {
    let body = &MEMORY_BREAKDOWN_SOURCE[START..END];
    let alpha = body
        .replace("$CB", "input")
        .replace("$CC", "entry")
        .replace("$CD", "output")
        .replace("$CE", "attribute")
        .replace("$CF", "record")
        .replace("$CG", "container");
    let source = format!("{}{}{}", &MEMORY_BREAKDOWN_SOURCE[..START], alpha, &MEMORY_BREAKDOWN_SOURCE[END..]);
    assert!(claim(&source));
    let begin = body.find("let $CG = {").unwrap();
    let end = body[begin..].find("$CF.j = $CG;").unwrap() + begin + "$CF.j = $CG;".len();
    let declaration = &body[begin..end];
    let direct = declaration
        .replace("let $CG =", "$CF.j =")
        .replace("$CF.j = $CG;", "");
    assert!(claim(&changed(declaration, &direct)));
}
#[test]
fn altered_protocols_refuse_the_actual_original_target() {
    let mutations = [
        ("slice(0, 99)", "slice(0, 100)"),
        (
            r#"RegExp("^http(s)?:\\/\\/")"#,
            r#"RegExp("^http(s)?:\\/\\/", "i")"#,
        ),
        (
            r#"RegExp("^http(s)?:\\/\\/")"#,
            r#"RegExp("http(s)?:\\/\\/")"#,
        ),
        ("e: $CC.bytes", "e: $CC.types"),
        ("$CC.attribution.map", "other.attribution.map"),
        ("i: $CE.scope", "i: $CC.scope"),
        ("$CE.container != null", "$CE.container == null"),
        ("$CE.container.id", "$CE.container.src"),
        ("return $CF;", "evil(); return $CF;"),
        ("return $CF;", "return $CE;"),
        ("$CF.j = $CG;", "$CF.j = other;"),
        ("$CC =>", "async $CC =>"),
        (".map($CC", ".map?.($CC"),
        ("return $CF;", "function donor(){return $CF;}return $CE;"),
        ("return $CF;", "$CE = other; return $CF;"),
        ("$CE =>", "($CE = other) =>"),
    ];
    for (from, to) in mutations {
        assert!(!claim(&changed(from, to)), "{from} -> {to}");
    }
    // A real lexical shadow is inserted into the actual enclosing main scope.
    // The function is moved only through its own body, leaving the target identity.
    let shadow = changed("return $CB.map", "let RegExp=foreign; return $CB.map");
    assert!(!claim(&shadow));
}

const REG_START: usize = 469046;
const REG_END: usize = 469239;
const CTOR_START: u32 = 468715;
fn registry_claims(source: &str) -> (bool, bool) {
    let result = name_threads(source, true).expect("actual registry scope must parse");
    let registered = result
        .report
        .outcomes
        .iter()
        .find(|o| {
            o.binding.declaration.start == REG_START as u32 && o.binding.name == "$THREAD_5_160335"
        })
        .expect("actual registry method must remain indexed");
    let constructor = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.declaration.start == CTOR_START && o.binding.name == "$THREAD_3_115185")
        .expect("actual owned constructor must remain indexed");
    (
        registered.applied
            && registered.role_name.as_deref() == Some("registerExtension")
            && registered.evaluator.as_deref() == Some("shared_extension_registry"),
        constructor.applied
            && constructor.role_name.as_deref() == Some("ExtensionRegistry")
            && constructor.evaluator.as_deref() == Some("shared_extension_registry_constructor"),
    )
}
fn registry_changed(from: &str, to: &str) -> String {
    let body = &MEMORY_BREAKDOWN_SOURCE[REG_START..REG_END];
    assert!(
        body.contains(from),
        "mutation must alter actual method: {from}"
    );
    let modified = body.replace(from, to);
    assert_ne!(modified, body);
    format!("{}{}{}", &MEMORY_BREAKDOWN_SOURCE[..REG_START], modified, &MEMORY_BREAKDOWN_SOURCE[REG_END..])
}
#[test]
fn original_scratch_registration_and_scoped_alpha_are_complete() {
    assert_eq!(registry_claims(MEMORY_BREAKDOWN_SOURCE), (true, true));
    let body = &MEMORY_BREAKDOWN_SOURCE[REG_START..REG_END];
    let alpha = body
        .replace("$CC", "key")
        .replace("$CD", "detect")
        .replace("$CE", "serialize")
        .replace("$CF", "deserialize")
        .replace("$CG", "record");
    let changed = format!("{}{}{}", &MEMORY_BREAKDOWN_SOURCE[..REG_START], alpha, &MEMORY_BREAKDOWN_SOURCE[REG_END..]);
    assert_eq!(registry_claims(&changed), (true, true));
}
#[test]
fn malformed_own_method_cannot_supply_constructor_gate_evidence() {
    for (from, to) in [
        ("detector: $CD", "detector: $CE"),
        (
            "this.extensions[$CC] = $CG;",
            "other.extensions[$CC] = $CG;",
        ),
        (
            "this.extensions[$CC] = $CG;",
            "evil();this.extensions[$CC] = $CG;",
        ),
        (
            "this.extensions[$CC] = $CG;",
            "this.extensions[$CC] = other;",
        ),
        (
            "this.extensions[$CC] = $CG;",
            "this.extensions[$CC] = $CG;$CG = other;",
        ),
        (
            "this.extensions[$CC] = $CG;",
            "function writer(){$CG=other;}this.extensions[$CC] = $CG;",
        ),
    ] {
        assert_eq!(
            registry_claims(&registry_changed(from, to)),
            (false, false),
            "{from}->{to}"
        );
    }
}
#[test]
fn constructor_still_requires_its_own_guard_and_initializer() {
    let ctor = &MEMORY_BREAKDOWN_SOURCE[CTOR_START as usize..468971];
    let mutated = ctor.replace("Object.create(null)", "Object.create({})");
    assert_ne!(ctor, mutated);
    // Restore the method's byte start by keeping the mutation byte length fixed.
    let mutated = mutated.replace("Object.create({})", "Object.create(0   )");
    assert_eq!(ctor.len(), mutated.len());
    let changed = format!(
        "{}{}{}",
        &MEMORY_BREAKDOWN_SOURCE[..CTOR_START as usize],
        mutated,
        &MEMORY_BREAKDOWN_SOURCE[468971..]
    );
    assert_eq!(registry_claims(&changed), (true, false));
}
