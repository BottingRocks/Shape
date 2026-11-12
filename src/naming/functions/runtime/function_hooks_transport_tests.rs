use super::*;

const FULL: &str = include_str!("../../../../tests/fixtures/method_interception/full_scope.js");

fn proof(source: &str) -> bool {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source).expect("fixture parses");
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let Ok(model) = Model::new(
        source,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    ) else {
        return false;
    };
    let site = model
        .all_sites()
        .iter()
        .find(|s| {
            s.binding_symbol
                .is_some_and(|id| scoping.symbol_name(id) == "$THREAD_8_111768")
        })
        .expect("installer declaration");
    proves(&model, *site).is_some()
}

fn changed(from: &str, to: &str) -> String {
    // Generated spellings repeat in unrelated scopes. Mutate the original full
    // installer declaration, retaining the actual enclosing source untouched.
    let span = 6755..10897;
    let own = &FULL[span.clone()];
    if own.contains(from) {
        let mut source = FULL.to_string();
        source.replace_range(span, &own.replacen(from, to, 1));
        source
    } else {
        // Deliberate capture-initializer/escape mutations live outside the body.
        assert!(FULL.contains(from), "mutation source absent: {from}");
        FULL.replacen(from, to, 1)
    }
}

#[test]
fn full_scope_postloop_record_copy_and_canonical_guard() {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, FULL).unwrap();
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        FULL,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    let site = *model
        .all_sites()
        .iter()
        .find(|s| s.span.start == 6755)
        .unwrap();
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        panic!()
    };
    let mut scan = Scan::default();
    scan_statements(&f.body.as_ref().unwrap().statements, &mut scan);
    // The exact blocked canonical conjunct: the reset adds a fifth loop item.
    let cache = scan
        .loops
        .iter()
        .find(|s| {
            loop_body(s).is_some_and(|ss| ss.len() == 5 && plain_assignment(&ss[0]).is_some())
        })
        .unwrap();
    assert_eq!(loop_body(cache).unwrap().len(), 5);
    assert!(function_interceptor(&model, site, f, f.body.as_ref().unwrap()).is_none());
    let (actual, captures) = normalized(&model, site).expect("normalize full scope");
    assert_eq!(Some(actual), expected(), "whole protocol grammar");
    let owner = model.owning_site(site.node_id).unwrap();
    let AstKind::Function(parent) = model.nodes.kind(owner.node_id) else {
        panic!()
    };
    for id in &captures {
        let origin = parent
            .body
            .as_ref()
            .unwrap()
            .statements
            .iter()
            .find(|s| initializes(&model, s, *id));
        println!(
            "capture symbol={id:?} name={} declaration={:?} definite_origin={:?}",
            model.scoping.symbol_name(*id),
            model.scoping.symbol_span(*id),
            origin.map(GetSpan::span)
        );
    }
    assert!(proof(FULL), "closed new protocol with real capture context");
}

#[test]
fn scoped_alpha_and_capture_alpha_preserve_protocol() {
    assert!(proof(
        &FULL
            .replace("$KEY_373", "transportCall")
            .replace("$KEY_241", "ownedRegistry")
            .replace("$AX", "selectedCache")
            .replace("$AY", "transportedCache")
            .replace("$KEY_9", "beforeChannel")
            .replace("$KEY_213", "externalConstruct")
    ));
}

#[test]
fn refuse_owner_reset_reference_and_order_mutations() {
    for (from, to) in [
        ("if ($AT !== $AX.object)", "if ($AN !== $AX.object)"),
        ("$AX = void 0;", "$AX = sideEffect();"),
        ("$AY = $AX;", "$AY = $AN;"),
        (
            "$AX = null;\n\t\t\t\t\t$AY = null;",
            "$AX = null;\n\t\t\t\t\t$AY = $AX.result;",
        ),
        ("$AZ >= $KEY_241[$AO].length", "$AZ > $KEY_241[$AO].length"),
        ("++$AZ;", "--$AZ;"),
        ("$KEY_10 = $AY.original;", "$KEY_10 = $AY.result;"),
        ("$AY.result != null", "$AX.original != null"),
        ("$AY = $AX;", "capture(() => $AX); $AY = $AX;"),
        ("$AY = $AX;", "let $AX = null; $AY = $AX;"),
    ] {
        assert!(
            !proof(&changed(from, to)),
            "accepted mutation {from} => {to}"
        );
    }
}

#[test]
fn refuse_wrapper_copy_publication_semantics() {
    for (from, to) in [
        ("$AP === void 0", "$AP == null"),
        ("$KEY_7 = false;", "$KEY_7 = true;"),
        ("this instanceof $THREAD_0_69749", "this instanceof Object"),
        ("$KEY_213($KEY_10, $BH)", "$KEY_213($BH, $KEY_10)"),
        (
            "$KEY_127($KEY_10, this, $BH)",
            "$KEY_127($KEY_10, $AN, $BH)",
        ),
        (
            "$KEY_127($KEY_10, this, $BH)",
            "$KEY_213($KEY_10, this, $BH)",
        ),
        ("$KEY_9.notify({", "$KEY_4.notify({"),
        ("$KEY_4.notify($BJ)", "$KEY_9.notify($BJ)"),
        ("threw: true,", "threw: false,"),
        ("$BG.bypassResult.throw", "$BG.bypassResult.value"),
        ("$KEY_4.notify($BJ)", "$KEY_4.notify?.($BJ)"),
        (
            "$KEY_373(Array.prototype.indexOf, $KEY_171, $BC[i]) === -1",
            "$KEY_373(Array.prototype.indexOf, $KEY_171, $BC[i]) === 1",
        ),
        ("$BD.configurable !== false", "$BD.configurable === false"),
        (
            "Object.defineProperty($AT, $AO, $AR)",
            "Object.defineProperty($AN, $AO, $AR)",
        ),
        ("original: $KEY_10", "original: $AN"),
        ("onBeforeInvoke: $KEY_9", "onBeforeInvoke: $KEY_4"),
        ("$KEY_92.push($BB)", "$KEY_92.push($BA)"),
        ("$KEY_139.push($BA)", "$KEY_139.push($BB)"),
        ("$KEY_10, \"prototype\"", "$KEY_10, \"other\""),
    ] {
        assert!(
            !proof(&changed(from, to)),
            "accepted mutation {from} => {to}"
        );
    }
}

#[test]
fn refuse_missing_origin_shadow_and_early_invocation() {
    assert!(!proof(&changed(
        "$KEY_373 = $H(Object.call);",
        "$KEY_373 = $H(Object.call); $KEY_373 = $H(Object.apply);"
    )));
    assert!(!proof(&changed(
        "$KEY_373 = $H(Object.call);",
        "$KEY_373 = $H(Object.call); function replaceHelper(){ $KEY_373 = null; }"
    )));
    assert!(!proof(&changed(
        "$KEY_373 = $H(Object.call);",
        "$H = foreign; $KEY_373 = $H(Object.call);"
    )));
    assert!(!proof(&changed(
        "$AU === false && $AV === false",
        "$AU == false && $AV === false"
    )));
    assert!(!proof(&changed(
        "$AU === false && $AV === false",
        "$AU === false || $AV === false"
    )));
    assert!(!proof(&changed(
        "$KEY_373 = $H(Object.call);",
        "$KEY_373 = $H(Object.apply);"
    )));
    assert!(!proof(
        &FULL
            .replace("$KEY_213($KEY_10, $BH)", "$KEY_127($KEY_10, $BH)")
            .replace(
                "$KEY_127($KEY_10, this, $BH)",
                "$KEY_213($KEY_10, this, $BH)"
            )
    ));
    assert!(!proof(&changed(
        "let $KEY_129 = $THREAD_8_111768;",
        "let $KEY_129 = $THREAD_8_111768; early(); function early(){ $KEY_129({}, 'early'); }"
    )));
    assert!(!proof(&changed(
        "$KEY_373 = $H(Object.call);",
        "/* missing call-through origin */"
    )));
    assert!(!proof(&changed(
        "let $KEY_129 = $THREAD_8_111768;",
        "let $KEY_129 = $THREAD_8_111768; $KEY_129({}, 'early');"
    )));
    assert!(!proof(&changed(
        "let $KEY_129 = $THREAD_8_111768;",
        "let $KEY_129 = $THREAD_8_111768; publish($KEY_129);"
    )));
    for global in ["Object", "Array", "Function"] {
        let start = "function $THREAD_8_111768($AN, $AO, $AP) {";
        assert!(!proof(&changed(
            start,
            &format!("{start} let {global} = foreign;")
        )));
    }
    // A standalone body has no captured origin proof.
    let begin = FULL.find("function $THREAD_8_111768(").unwrap();
    let end = FULL[begin..].find("\n\t\tfunction ").unwrap() + begin;
    assert!(!proof(&format!(
        "function scope(){{ {} }}",
        &FULL[begin..end]
    )));
}
