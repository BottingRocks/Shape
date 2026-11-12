use super::*;
const FOUR_CLEANUP_DECLARATIONS: &str = include_str!("../../../../../tests/fixtures/naming/captured_handle_termination/stateful-parent.js");

const START: usize = 368218;
const END: usize = 368364;
const ROLE: &str = "terminateCapturedHandleIfPresentThenComplete";
fn claim_at(source: &str, start: u32) -> bool {
    let began = Instant::now();
    let allocator = Allocator::default();
    let program =
        crate::analysis::parse_program(&allocator, source).expect("whole actual scope must parse");
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        source,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .expect("whole actual model");
    let site = *model
        .all_sites()
        .iter()
        .find(|s| s.span.start == start)
        .expect("same exact target must remain indexed");
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        panic!("same target must be a function")
    };
    let id = function
        .id
        .as_ref()
        .expect("target keeps its declaration identifier");
    assert_eq!(
        id.name, "$THREAD_4_51062",
        "refusal cannot use a different binding"
    );
    assert_eq!(site.binding_symbol, id.symbol_id.get());
    let indexed = began.elapsed();
    let result = evaluate(&model, site, &Facts::default());
    eprintln!(
        "target={start} parse_model={indexed:?} own_evaluator={:?}",
        began.elapsed() - indexed
    );
    if let Some(p) = result.first() {
        assert_eq!(p.name, ROLE);
        assert_eq!(p.evaluator, "guarded_handle_completion");
    }
    !result.is_empty()
}
fn claim(source: &str) -> bool {
    claim_at(source, START as u32)
}
fn changed(from: &str, to: &str) -> String {
    let body = &FOUR_CLEANUP_DECLARATIONS[START..END];
    assert!(body.contains(from), "mutation hits own target: {from}");
    format!("{}{}{}", &FOUR_CLEANUP_DECLARATIONS[..START], body.replace(from, to), &FOUR_CLEANUP_DECLARATIONS[END..])
}
#[test]
fn actual_whole_scope_and_scoped_alpha() {
    for start in [368218, 789487, 1200102, 1521869] {
        assert!(claim_at(FOUR_CLEANUP_DECLARATIONS, start));
    }
    let start = 332704;
    let end = 368447;
    let alpha = FOUR_CLEANUP_DECLARATIONS[start..end].replace("$KEY_41", "handleAlpha");
    let target = start + alpha.find("function $THREAD_4_51062()").unwrap();
    let source = format!("{}{}{}", &FOUR_CLEANUP_DECLARATIONS[..start], alpha, &FOUR_CLEANUP_DECLARATIONS[end..]);
    assert!(
        claim_at(&source, target as u32),
        "must prove the changed declaration, not another bundle"
    );
}
#[test]
fn altered_whole_protocol_refuses_same_target() {
    for (from, to) in [
        ("$KEY_41 != null", "$KEY_41"),
        ("$KEY_41 != null", "$KEY_41 == null"),
        ("$KEY_41 != null", "$KEY_28 != null"),
        ("$KEY_41.terminate()", "$KEY_28.terminate()"),
        ("$KEY_41.terminate()", "$KEY_41.terminate(1)"),
        ("$KEY_28.complete()", "$KEY_28.complete(1)"),
        ("$KEY_28.complete()", "$KEY_41.complete()"),
        (
            "$KEY_41.terminate();",
            "$KEY_28.complete();$KEY_41.terminate();",
        ),
        ("$KEY_41.terminate();", "return $KEY_41.terminate();"),
        ("$KEY_28.complete();", "return $KEY_28.complete();"),
        ("$KEY_28.complete();", ""),
        ("$KEY_41.terminate();", "$KEY_41?.terminate();"),
        ("$KEY_28.complete();", "extra();$KEY_28.complete();"),
        ("$KEY_41.terminate();", "label: $KEY_41.terminate();"),
        (
            "$KEY_41.terminate();",
            "try{$KEY_41.terminate();}catch(e){}",
        ),
        (
            "$KEY_28.complete();",
            "function donor(){$KEY_28.complete();}",
        ),
    ] {
        assert!(!claim(&changed(from, to)), "{from}->{to}");
    }
}
#[test]
fn captures_preinit_writers_and_reentrant_exposures_refuse() {
    // Keep all mutations after START to retain the exact target identity.
    for (from, to) in [
        (
            "$KEY_8 = $THREAD_4_51062;",
            "$KEY_8 = $THREAD_4_51062;$KEY_41 = other;",
        ),
        (
            "$KEY_8 = $THREAD_4_51062;",
            "$KEY_8 = $THREAD_4_51062;function writer(){$KEY_41=other;}",
        ),
        (
            "$KEY_8 = $THREAD_4_51062;",
            "$KEY_8 = $THREAD_4_51062;$KEY_28 = other;",
        ),
        (
            "$KEY_8 = $THREAD_4_51062;",
            "$KEY_8 = $THREAD_4_51062;$KEY_5 = other;",
        ),
        (
            "$KEY_8 = $THREAD_4_51062;",
            "$KEY_8 = $THREAD_4_51062;expose($THREAD_4_51062);",
        ),
        (
            "$KEY_8 = $THREAD_4_51062;",
            "$KEY_8 = $THREAD_4_51062;register(()=>$KEY_8());",
        ),
        (
            "$KEY_8 = $THREAD_4_51062;",
            "$KEY_8 = $THREAD_4_51062;if(branch){$KEY_8=foreign;}",
        ),
    ] {
        let tail = &FOUR_CLEANUP_DECLARATIONS[END..];
        assert!(tail.contains(from));
        let s = format!("{}{}", &FOUR_CLEANUP_DECLARATIONS[..END], tail.replacen(from, to, 1));
        assert!(!claim(&s), "{to}");
    }
    // Prefix edits shift START: find the same generated binding by its shifted
    // byte identity, never accept a parser failure or a missing target as refusal.
    for (from, to) in [
        ("let $KEY_41;", "let $KEY_41 = foreign;"),
        ("$KEY_41 = new $KEY_5(", "$KEY_41 = fakeFactory("),
        ("let $KEY_28 = $DF[1];", "let $KEY_28 = foreign;"),
        ("$KEY_8();", "$KEY_8(1);"),
        ("if (typeof $KEY_8 === \"function\")", "if ($KEY_8)"),
        ("let $KEY_8, $KEY_26;", "let $KEY_8 = foreign, $KEY_26;"),
    ] {
        // Select the exact enclosing owner, avoiding unrelated same spellings.
        let prefix = &FOUR_CLEANUP_DECLARATIONS[..START];
        let marker = prefix.rfind("function $THREAD_3_62619(").unwrap();
        let own = &prefix[marker..];
        assert!(own.contains(from), "{from}");
        let replacement = own.replacen(from, to, 1);
        let delta = replacement.len() as i64 - own.len() as i64;
        let s = format!("{}{}{}", &prefix[..marker], replacement, &FOUR_CLEANUP_DECLARATIONS[START..]);
        assert!(!claim_at(&s, (START as i64 + delta) as u32), "{from}->{to}");
    }
}
// Every mutation below keeps the complete original source and resolves the
// same first cleanup declaration. Globals carry no native identity in this rule.
fn owner_prefix_mutation(from: &str, to: &str) -> (String, u32) {
    let prefix = &FOUR_CLEANUP_DECLARATIONS[..START];
    let marker = prefix.rfind("function $THREAD_3_62619(").unwrap();
    let own = &prefix[marker..];
    assert!(own.contains(from), "{from}");
    let replacement = own.replacen(from, to, 1);
    let target = START as i64 + replacement.len() as i64 - own.len() as i64;
    (
        format!("{}{}{}", &prefix[..marker], replacement, &FOUR_CLEANUP_DECLARATIONS[START..]),
        target as u32,
    )
}
#[test]
fn same_target_preinit_branch_alias_and_synchronous_registration_controls() {
    for (from, to) in [
        ("let $KEY_41;", "$THREAD_4_51062();let $KEY_41;"),
        ("let $KEY_41;", "register($THREAD_4_51062);let $KEY_41;"),
        (
            "let $KEY_41;",
            "if (branch) expose($THREAD_4_51062);let $KEY_41;",
        ),
        ("let $KEY_28 = $DF[1];", "let $KEY_28;"),
        (
            "let $KEY_5 = $DG.global.Worker;",
            "let Worker=foreign;let $KEY_5=Worker;",
        ),
    ] {
        let (source, target) = owner_prefix_mutation(from, to);
        assert!(!claim_at(&source, target), "{from}->{to}");
    }
    let tail = &FOUR_CLEANUP_DECLARATIONS[END..];
    for suffix in [
        "if(branch){$KEY_41=foreign;}",
        "let alias=$KEY_8;use(alias);",
        "register(()=>$KEY_8());",
        "function deferred(){$KEY_28=foreign;}",
    ] {
        let source = format!(
            "{}{}",
            &FOUR_CLEANUP_DECLARATIONS[..END],
            tail.replacen(
                "$KEY_8 = $THREAD_4_51062;",
                &format!("$KEY_8 = $THREAD_4_51062;{suffix}"),
                1
            )
        );
        assert!(!claim(&source), "{suffix}");
    }
}
#[test]
fn handle_store_must_finish_before_cleanup_exposure() {
    let write = &FOUR_CLEANUP_DECLARATIONS[350817..353387];
    assert!(write.starts_with("$KEY_41 = new $KEY_5("));
    let mut source = format!("{}void 0{}", &FOUR_CLEANUP_DECLARATIONS[..350817], &FOUR_CLEANUP_DECLARATIONS[353387..]);
    source = source.replacen(
        "$KEY_8 = $THREAD_4_51062;",
        &format!("$KEY_8 = $THREAD_4_51062;{write};"),
        1,
    );
    let target = (START - (353387 - 350817) + "void 0".len()) as u32;
    assert!(
        !claim_at(&source, target),
        "a sole late constructor store is not dominance"
    );
}
#[test]
fn private_expression_self_name_is_not_an_external_function_binding() {
    let prefix = "const externalCleanup = ";
    let body = format!("{prefix}{};", &FOUR_CLEANUP_DECLARATIONS[START..END]);
    let tail = FOUR_CLEANUP_DECLARATIONS[END..].replacen("$KEY_8 = $THREAD_4_51062;", "$KEY_8 = externalCleanup;", 1);
    let source = format!("{}{}{}", &FOUR_CLEANUP_DECLARATIONS[..START], body, tail);
    assert!(!claim_at(&source, (START + prefix.len()) as u32));
}
#[test]
fn unrelated_shadowed_global_does_not_supply_native_identity() {
    // The rule never relies on Math/Worker being unbound globals. A shadow of
    // an unrelated global cannot supply or invalidate the opaque own protocol.
    let (source, target) = owner_prefix_mutation("let $KEY_41;", "let Math=foreign;let $KEY_41;");
    assert!(claim_at(&source, target));
}
