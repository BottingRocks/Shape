use super::*;

const REVERSE: &str = include_str!("../../../tests/fixtures/program/lazy-registry/native-reverse.js");
const FORWARD: &str =
    include_str!("../../../tests/fixtures/program/lazy-registry/native-forward.js");
const ORDINARY_REVERSE: &str =
    include_str!("../../../tests/fixtures/program/lazy-registry/ordinary-reverse.js");
const ORDINARY_FORWARD: &str =
    include_str!("../../../tests/fixtures/program/lazy-registry/ordinary-forward.js");
const STAGED: &str =
    include_str!("../../../tests/fixtures/program/lazy-registry/staged-carrier.js");

// Every refusal first parses and constructs the same valid Model as positives.
fn inspect<R>(source: &str, check: impl FnOnce(&Model<'_, '_>) -> R) -> R {
    let allocator = oxc_allocator::Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source).expect("fixture must parse");
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        source,
        &program,
        &scoping,
        &nodes,
        crate::signals::schema::ProgramPropertiesLimits::default(),
    )
    .expect("fixture must build Model");
    check(&model)
}
fn prove(source: &str) -> Vec<FinderClaim> {
    // These legacy refusals test native purpose; a neutral own-protocol claim
    // may remain valid when only a native installer obligation is broken.
    inspect(source, |model| claims(model).into_iter().filter(|claim| matches!(claim,
        FinderClaim::ClassBinding { role: "owned_lazy_hook_registry_class", .. })).collect())
}
fn accepts(source: &str) {
    // Keep the dependency failure separate from the final family assertion.
    // A negative that passes only because this real positive lost its scoped
    // descriptor registry would otherwise be vacuous.
    inspect(source, |model| {
        let factories: Vec<_> = model
            .all_sites()
            .iter()
            .filter_map(|site| {
                let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
                    return None;
                };
                returned_class(model, f).map(|class| (*site, class))
            })
            .collect();
        assert_eq!(factories.len(), 1, "one exact returned-class factory");
        let (factory, class) = factories[0];
        let (constructor, register) = methods(class).expect("own class methods");
        assert!(
            forwards_arguments(model, constructor).is_some(),
            "own arguments forwarding"
        );
        let calls = model.calls_to(factory);
        assert_eq!(calls.len(), 1, "one actual factory/base transport");
        let base = symbol(model, calls[0].arguments[0].as_expression().unwrap()).unwrap();
        assert!(
            crate::naming::classes::callback_notifier::proves_base(model, base),
            "shared notifier proof including subclass transport"
        );
        let installation = call(&body(register).unwrap().statements[0]).unwrap();
        let hook = stable_function(model, &installation.callee, calls[0].span.start)
            .expect("ordered installer origin");
        let receiver_proofs: Vec<_> = owned(model, hook)
            .filter_map(|kind| {
                let AstKind::CallExpression(c) = kind else {
                    return None;
                };
                let (channel, "register") = member(&c.callee)? else {
                    return None;
                };
                let (receiver, "onBeforeInvoke") = member(channel)? else {
                    return None;
                };
                hook_receiver(model, receiver, calls[0].span.start)
            })
            .collect();
        assert!(
            !receiver_proofs.is_empty(),
            "scoped receiver registry and complete shared descriptor proof"
        );
        assert!(
            installer(model, &installation.callee, calls[0].span.start).is_some(),
            "installer owns receiver-consuming accumulator callback"
        );
    });
    let claims = prove(source);
    assert_eq!(claims.len(), 1, "expected one lazy class");
    assert!(
        matches!(&claims[0], FinderClaim::ClassBinding { role: "owned_lazy_hook_registry_class", evidence, .. } if evidence.iter().any(|s| s.contains("verified notifier base")))
    );
}
fn refuses(source: &str) {
    assert!(prove(source).is_empty(), "unexpected lazy proof");
}
fn changed(source: &str, old: &str, new: &str) -> String {
    assert!(source.contains(old), "mutation anchor absent: {old}");
    source.replacen(old, new, 1)
}

#[test]
fn lazy_registry_native_reverse_and_forward_real_installation() {
    accepts(REVERSE);
    accepts(FORWARD);
}

#[test]
fn lazy_registry_staged_owned_receiver_dispatch() {
    accepts(STAGED);
    // The existing independent direct-object path still proves this callback.
    let direct = changed(STAGED, "let $BO = {};", "");
    let direct = changed(&direct, "$BO.target = $BM.thisObj;", "");
    accepts(&changed(
        &direct,
        "$KEY_4($BO);",
        "$KEY_4({ target: $BM.thisObj });",
    ));
}

#[test]
fn lazy_registry_staged_carrier_rejects_mutation_escape_and_wrong_origins() {
    for (old, new) in [
        ("let $BO = {};", "let $BO = { extra: 1 };"),
        ("let $BO = {};", "let $BO = donor;"),
        ("$BO.target = $BM.thisObj;", "$BO.target = other.thisObj;"),
        ("$BO.target = $BM.thisObj;", "$BO.extra = $BM.thisObj;"),
        (
            "$BO.target = $BM.thisObj;",
            "if (false) { $BO.target = $BM.thisObj; }",
        ),
        (
            "$BO.target = $BM.thisObj;",
            "function donor() { $BO.target = $BM.thisObj; }",
        ),
        ("$KEY_4($BO);", "external($BO); $KEY_4($BO);"),
        ("$KEY_4($BO);", "let copy = $BO; $KEY_4(copy);"),
        ("$KEY_4($BO);", "$BO = {}; $KEY_4($BO);"),
        ("$KEY_4($BO);", "$BO.other = 1; $KEY_4($BO);"),
        ("$KEY_4($BO);", "$KEY_4(...[$BO]);"),
        ("$KEY_4($BO);", "$KEY_4(other);"),
        ("$KEY_4($BO);", "return $BN; $KEY_4($BO);"),
        ("$KEY_4($BO);", "$KEY_4($BO); return null;"),
    ] {
        refuses(&changed(STAGED, old, new));
    }
}

#[test]
fn lazy_registry_stable_constructor_transport_and_mutation_refusal() {
    let alias = changed(
        REVERSE,
        "let $BP = $THREAD_8_110436($THREAD_1_2521);",
        "let $BP = $THREAD_8_110436($THREAD_1_2521); let constructorCopy = $BP;",
    );
    accepts(&alias.replace("new $BP()", "new constructorCopy()"));
    refuses(&changed(
        REVERSE,
        "let $BP = $THREAD_8_110436($THREAD_1_2521);",
        "let $BP = $THREAD_8_110436($THREAD_1_2521); $BP = other;",
    ));
    refuses(&changed(
        REVERSE,
        "let $BP = $THREAD_8_110436($THREAD_1_2521);",
        "let $BP = $THREAD_8_110436($THREAD_1_2521); $BP.prototype.register = other;",
    ));
    refuses(&changed(
        REVERSE,
        "let $BP = $THREAD_8_110436($THREAD_1_2521);",
        "let $BP = $THREAD_8_110436($THREAD_1_2521); external($BP);",
    ));
}

#[test]
fn lazy_registry_ordinary_reverse_and_forward_real_installation() {
    // Both complete prototype installation excerpts and original call arguments
    // are retained. The shared notifier owns both distinct base protocols.
    accepts(ORDINARY_REVERSE);
    accepts(ORDINARY_FORWARD);
}

fn refuses_ordinary_base(source: &str) {
    inspect(source, |model| {
        let factory = model
            .all_sites()
            .iter()
            .find(|site| {
                matches!(model.nodes.kind(site.node_id), AstKind::Function(f) if returned_class(model, f).is_some())
            })
            .expect("real lazy factory retained");
        let calls = model.calls_to(*factory);
        assert_eq!(calls.len(), 1, "real base transport retained");
        let base = symbol(model, calls[0].arguments[0].as_expression().unwrap()).unwrap();
        assert!(
            !crate::naming::classes::callback_notifier::proves_base(model, base),
            "malformed ordinary base must fail the shared dependency proof"
        );
        let claims = claims(model);
        assert!(claims.iter().all(|claim| matches!(claim,
            FinderClaim::ClassBinding {
                role: "owned_pre_register_hook_forwarder_class", evidence, ..
            } if evidence.iter().any(|s| s.contains("explicitly unproved")))),
            "malformed native base may only retain the neutral own protocol");
    });
}

#[test]
fn lazy_registry_malformed_ordinary_reverse_base_refusals() {
    for (old, new) in [
        ("this.callbacks = [];", "this.callbacks = {};"),
        ("$THREAD_5_99971.prototype.register = $THREAD_9_93980;", ""),
        (
            "$THREAD_5_99971.prototype.notify = $THREAD_2_99666;",
            "if (false) { $THREAD_5_99971.prototype.notify = $THREAD_2_99666; }",
        ),
        (
            "$BU[$BV - 1].call(void 0, $BT, $BW)",
            "$BU[$BV - 1].call(void 0, $BW, $BT)",
        ),
        (
            "let $BR = $THREAD_3_76512($THREAD_5_99971);",
            "$THREAD_5_99971.prototype.extra = other; let $BR = $THREAD_3_76512($THREAD_5_99971);",
        ),
    ] {
        refuses_ordinary_base(&changed(ORDINARY_REVERSE, old, new));
    }
}

#[test]
fn lazy_registry_malformed_ordinary_forward_base_refusals() {
    for (old, new) in [
        ("this.callbacks = [];", "this.callbacks = {};"),
        (
            "$THREAD_5_68382.prototype.unregister = $THREAD_1_86713;",
            "",
        ),
        (
            "this.callbacks[i].call(void 0, $AH, $AI)",
            "this.callbacks[i].call(this, $AH, $AI)",
        ),
        (
            "$THREAD_5_68382.prototype.notify = $THREAD_5_11594;",
            "$THREAD_5_68382.prototype.notify = $THREAD_2_112022;",
        ),
        (
            "let $AF = $THREAD_6_103742($THREAD_5_68382);",
            "external($THREAD_5_68382); let $AF = $THREAD_6_103742($THREAD_5_68382);",
        ),
    ] {
        refuses_ordinary_base(&changed(ORDINARY_FORWARD, old, new));
    }
}

#[test]
fn lazy_registry_order_receiver_formal_and_extra_effects() {
    for (old, new) in [
        (
            "$KEY_28();\n\t\t\t\t\tsuper.register($BS);",
            "super.register($BS); $KEY_28();",
        ),
        ("super.register($BS);", "super.register(null);"),
        ("super.register($BS);", "super.register(...[$BS]);"),
        ("super.register($BS);", "other.register($BS);"),
        ("$KEY_28();", "$KEY_28($BS);"),
        (
            "return this;\n\t\t\t\t}\n\t\t\t}\n\t\t\treturn $THREAD_7_64696;",
            "return null;\n\t\t\t\t}\n\t\t\t}\n\t\t\treturn $THREAD_7_64696;",
        ),
        ("super(...arguments);", "super();"),
        ("super(...arguments);", "super(...other);"),
        ("$KEY_28();", "$KEY_28(); sideEffect();"),
        (
            "return $THREAD_7_64696;",
            "sideEffect(); return $THREAD_7_64696;",
        ),
        (
            "return $THREAD_7_64696;",
            "class Wrong extends $BR {} return Wrong;",
        ),
        ("extends $BR", "extends Object"),
        (
            "$THREAD_8_110436($THREAD_1_2521)",
            "$THREAD_8_110436(Object)",
        ),
    ] {
        refuses(&changed(REVERSE, old, new));
    }
}

#[test]
fn lazy_registry_mutable_alias_base_hook_and_wrong_installation_receiver() {
    for (old, new) in [
        (
            "let $KEY_28 = $THREAD_9_78581;",
            "let $KEY_28 = $THREAD_9_78581; $KEY_28 = other;",
        ),
        (
            "let $BP = $THREAD_8_110436",
            "$THREAD_1_2521 = other; let $BP = $THREAD_8_110436",
        ),
        (
            "$KEY_190 = $KEY_195(window.HTMLFormElement.prototype, \"submit\");",
            "$KEY_190 = {onBeforeInvoke: other};",
        ),
        (
            "$KEY_190 = $KEY_195(window.HTMLFormElement.prototype, \"submit\");",
            "$KEY_190 = $KEY_195(window.HTMLFormElement.prototype, \"submit\"); $KEY_190 = other;",
        ),
        (
            "$KEY_190.onBeforeInvoke.register",
            "$KEY_190.onBeforeGet.register",
        ),
        (
            "$KEY_190.onBeforeInvoke.register",
            "unrelated.onBeforeInvoke.register",
        ),
        (
            "let $KEY_195 = $THREAD_1_75932;",
            "let $KEY_195 = $THREAD_1_75932; $KEY_195 = other;",
        ),
    ] {
        refuses(&changed(REVERSE, old, new));
    }
}

#[test]
fn lazy_registry_nested_donor_installer_does_not_supply_own_registration() {
    // The actual called function merely declares a donor containing all the
    // original behavior. Nearest-callable ownership must reject it.
    let source = changed(
        REVERSE,
        "function $THREAD_9_78581() {",
        "function $THREAD_9_78581() { function donor() {",
    );
    let source = changed(
        &source,
        "let $KEY_28 = $THREAD_9_78581;",
        "} let $KEY_28 = $THREAD_9_78581;",
    );
    refuses(&source);
}

#[test]
fn lazy_registry_spoofed_descriptor_factory_is_not_a_hook_origin() {
    let source = changed(REVERSE, "let $KEY_195 = $THREAD_1_75932;", "function fake(object, key) { return {onBeforeInvoke: {register(callback) {}}}; } let $KEY_195 = fake;");
    refuses(&source);
}

#[test]
fn lazy_registry_unresolved_registry_is_not_a_scoped_interceptor_origin() {
    refuses(&changed(REVERSE, ", $KEY_140,", ","));
    refuses(&changed(FORWARD, ", $KEY_199,", ","));
}

#[test]
fn lazy_registry_full_review_counterexamples_and_installed_multi_hop() {
    // Exact counterexample bytes are retained, alongside the same mutations on
    // the complete context-backed positive so no refusal relies on omissions.
    refuses(include_str!(
        "../../../tests/fixtures/program/lazy-registry/review-unreachable-install.js"
    ));
    refuses(include_str!(
        "../../../tests/fixtures/program/lazy-registry/review-returned-constructor-mutation.js"
    ));
    accepts(include_str!(
        "../../../tests/fixtures/program/lazy-registry/native-reverse-multi-hop.js"
    ));
    refuses(include_str!(
        "../../../tests/fixtures/program/lazy-registry/native-reverse-second-alias-mutation.js"
    ));
    refuses(&changed(
        REVERSE,
        "function $THREAD_9_78581() {",
        "function $THREAD_9_78581() { return;",
    ));
}

#[test]
fn lazy_registry_installer_execution_and_guard_near_misses() {
    for prefix in [
        "return;",
        "throw new Error();",
        "if (true) { return; }",
        "while (true) {}",
        "try { return; } finally {}",
    ] {
        refuses(&changed(
            REVERSE,
            "function $THREAD_9_78581() {",
            &format!("function $THREAD_9_78581() {{ {prefix}"),
        ));
    }
    for (old, new) in [
        (
            "if ($KEY_190?.onBeforeInvoke != null) {",
            "return; if ($KEY_190?.onBeforeInvoke != null) {",
        ),
        (
            "if ($KEY_190?.onBeforeInvoke != null) {",
            "if (false && $KEY_190?.onBeforeInvoke != null) {",
        ),
        ("$KEY_351 = false;", "$KEY_351 = true;"),
        ("$KEY_351 = true;", "$KEY_351 = other;"),
        ("$KEY_28();", "$KEY_28(); return this;"),
    ] {
        refuses(&changed(REVERSE, old, new));
    }
}

#[test]
fn lazy_registry_callback_requires_executable_receiver_dispatch_and_returns() {
    let opening = "$KEY_190.onBeforeInvoke.register(($BT, $BU) => {";
    for prefix in [
        "return $BU;",
        "throw new Error();",
        "if (true) { return $BU; }",
        "while (true) {}",
    ] {
        refuses(&changed(REVERSE, opening, &format!("{opening} {prefix}")));
    }
    for (old, new) in [
        (
            "$KEY_3({ target: $BT.thisObj });",
            "if (false) { $KEY_3({ target: $BT.thisObj }); }",
        ),
        (
            "$KEY_3({ target: $BT.thisObj });",
            "function donor() { $KEY_3({ target: $BT.thisObj }); }",
        ),
        (
            "$KEY_3({ target: $BT.thisObj });",
            "return null; $KEY_3({ target: $BT.thisObj });",
        ),
        (
            "$KEY_3({ target: $BT.thisObj });",
            "$KEY_3({ target: other.thisObj });",
        ),
        (
            "$KEY_3({ target: $BT.thisObj });",
            "$KEY_3({ target: $BT.thisObj }); return null;",
        ),
    ] {
        refuses(&changed(REVERSE, old, new));
    }
}

fn staged_channel_source() -> String {
    changed(
        REVERSE,
        "if ($KEY_190?.onBeforeInvoke != null) {",
        "let selectedChannel; if ($KEY_190 == null) { selectedChannel = void 0; } else { selectedChannel = $KEY_190.onBeforeInvoke; } if (selectedChannel != null) {",
    )
}

#[test]
fn lazy_registry_lowered_nullable_channel_retains_complete_proof() {
    accepts(&staged_channel_source());
    // The direct route remains independent of the staged-channel proof.
    accepts(REVERSE);
}

#[test]
fn lazy_registry_lowered_channel_requires_closed_ordered_local_flow() {
    let source = staged_channel_source();
    for (old, new) in [
        ("let selectedChannel;", "let selectedChannel = donor;"),
        ("let selectedChannel;", "var selectedChannel;"),
        (
            "let selectedChannel;",
            "function captureChannel() { return selectedChannel; } let selectedChannel;",
        ),
        (
            "let selectedChannel;",
            "external(selectedChannel); let selectedChannel;",
        ),
        ("$KEY_190 == null", "unrelated == null"),
        ("$KEY_190 == null", "$KEY_190 != null"),
        ("selectedChannel = void 0;", "selectedChannel = other;"),
        (
            "selectedChannel = void 0;",
            "function donor() { selectedChannel = void 0; }",
        ),
        (
            "selectedChannel = $KEY_190.onBeforeInvoke;",
            "selectedChannel = other.onBeforeInvoke;",
        ),
        (
            "selectedChannel = $KEY_190.onBeforeInvoke;",
            "selectedChannel = $KEY_190.onAfterInvoke;",
        ),
        (
            "selectedChannel = $KEY_190.onBeforeInvoke;",
            "let selectedChannel = $KEY_190.onBeforeInvoke;",
        ),
        (
            "selectedChannel = $KEY_190.onBeforeInvoke;",
            "if (false) { selectedChannel = $KEY_190.onBeforeInvoke; }",
        ),
        (
            "if (selectedChannel != null)",
            "selectedChannel = other; if (selectedChannel != null)",
        ),
        (
            "if (selectedChannel != null)",
            "unrelated.notify(selectedChannel); if (selectedChannel != null)",
        ),
        (
            "if (selectedChannel != null)",
            "if (false && selectedChannel != null)",
        ),
        (
            "$KEY_351 = true;",
            "selectedChannel = other; $KEY_351 = true;",
        ),
    ] {
        refuses(&changed(&source, old, new));
    }
}

#[test]
fn lazy_registry_cached_full_sources_keep_actual_base_and_installer_context() {
    // Byte spans select diagnostic witnesses only. The production proof takes
    // no source name, vendor, position or comparison-map input. Full cached
    // programs retain bootstrap, descriptor registry, aliases and consumers.
    for (source, factory_start, class_start) in [
        (
            include_str!(
                "../../../tests/fixtures/program/lazy-registry/branch-carrier-full.js"
            ),
            487665,
            487700,
        ),
        (
            include_str!("../../../tests/fixtures/program/lazy-registry/ordinary-reverse-full.js"),
            487393,
            487428,
        ),
    ] {
        inspect(source, |model| {
            let site = *model
                .all_sites()
                .iter()
                .find(|s| s.span.start == factory_start)
                .expect("actual cached factory");
            let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
                panic!("actual factory is an ordinary function");
            };
            let calls = model.calls_to(site);
            assert_eq!(calls.len(), 1, "actual ordered base transport");
            let base = symbol(model, calls[0].arguments[0].as_expression().unwrap()).unwrap();
            assert!(
                ordered_base(model, base, calls[0].span.start).is_some(),
                "ordered base origin"
            );
            assert!(
                crate::naming::classes::callback_notifier::proves_base(model, base),
                "shared complete native/ordinary notifier dependency"
            );
            let class = returned_class(model, f).expect("same-factory class return");
            assert_eq!(class.span.start, class_start);
            let (_, register) = methods(class).expect("complete own methods");
            let install = call(&body(register).unwrap().statements[0]).unwrap();
            let installer_site = stable_function(model, &install.callee, calls[0].span.start)
                .expect("actual ordered captured installer origin");
            let AstKind::Function(installer_function) = model.nodes.kind(installer_site.node_id)
            else {
                panic!("ordinary installer");
            };
            assert!(
                installer_phase(model, installer_function, calls[0].span.start).is_some(),
                "closed normal-completion installation phase"
            );
            assert!(
                installer(model, &install.callee, calls[0].span.start).is_some(),
                "scoped typed receiver, ordered channel guard and owned callback protocol"
            );
            assert!(
                matches!(super::prove(model, site, f),
                Some(FinderClaim::ClassBinding { role: "owned_lazy_hook_registry_class", class_span, .. })
                    if class_span == class.span),
                "actual complete lazy proof"
            );
        });
    }
}

#[test]
#[ignore = "explicit cached affected witness only; LAZY_REGISTRY_WITNESS supplies input"]
fn lazy_registry_cached_affected_witness() {
    let path = std::env::var("LAZY_REGISTRY_WITNESS").expect("explicit cached source");
    let source = std::fs::read_to_string(path).unwrap();
    inspect(&source, |model| {
        for site in model.all_sites() {
            let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
                continue;
            };
            let Some(class) = returned_class(model, f) else {
                continue;
            };
            let Some((_, register)) = methods(class) else {
                continue;
            };
            let Some(install) = body(register)
                .and_then(|b| b.statements.first())
                .and_then(call)
            else {
                continue;
            };
            for c in model.calls_to(*site) {
                let base = symbol(model, c.arguments[0].as_expression().unwrap()).unwrap();
                eprintln!(
                    "class={:?} ordered_base={} notifier={} stable_installer={} installer={}",
                    class.span,
                    ordered_base(model, base, c.span.start).is_some(),
                    crate::naming::classes::callback_notifier::proves_base(model, base),
                    stable_function(model, &install.callee, c.span.start).is_some(),
                    installer(model, &install.callee, c.span.start).is_some()
                );
                if let Some(hook) = stable_function(model, &install.callee, c.span.start) {
                    if let AstKind::Function(inst) = model.nodes.kind(hook.node_id) {
                        eprintln!(
                            "phase={}",
                            installer_phase(model, inst, c.span.start).is_some()
                        );
                    }
                    for kind in owned(model, hook) {
                        let AstKind::CallExpression(reg) = kind else {
                            continue;
                        };
                        let Some((channel, "register")) = member(&reg.callee) else {
                            continue;
                        };
                        let Some((receiver, "onBeforeInvoke")) = member(channel) else {
                            continue;
                        };
                        eprintln!(
                            "receiver={:?} proof={}",
                            receiver.span(),
                            hook_receiver(model, receiver, c.span.start).is_some()
                        );
                    }
                }
            }
        }
        assert_eq!(claims(model).len(), 1, "one complete actual lazy class");
    });
}

const DUAL_OUTPUT: &str =
    include_str!("../../../tests/fixtures/program/lazy-registry/dual-output.js");

#[test]
fn lazy_registry_two_output_lookup_transports_keep_complete_origin() {
    accepts(DUAL_OUTPUT);
    let alpha = DUAL_OUTPUT
        .replace("$AO", "walkGuard")
        .replace("$AP", "walkResult")
        .replace("$AV", "cacheGuard")
        .replace("$AW", "cacheResult");
    accepts(&alpha);
}

#[test]
fn lazy_registry_two_output_transport_refuses_divergence_and_unowned_donors() {
    for (old, new) in [
        ("$AP = $AO;", "$AP = other;"),
        ("$AP = $AO;", "function donor() { $AP = $AO; }"),
        ("$AP = $AO;", "$AP = $AO; $AP = other;"),
        ("$AP = null;", "$AP = other;"),
        ("$AP = void 0;", "$AP = other;"),
        ("$AP = $AO;", "external($AP); $AP = $AO;"),
        ("$AP = $AO;", "$AP = $AO; external($AO);"),
        ("$AW = $AV;", "$AW = sibling;"),
        ("$AW = $AV;", "function donor() { $AW = $AV; }"),
        ("$AW = null;", "$AW = other;"),
        ("$AW = $AV;", "$AW = $AV; $AW = other;"),
        ("$AW = $AV;", "$AW = $AV; external($AV);"),
        ("$AP.containingObj", "$AP.otherOwner"),
        ("$AP = $AP.desc;", "$AP = $AP.desc; external($AP);"),
        (
            "$KEY_7 = $AW.original;",
            "$KEY_7 = $AW.original; external($AW);",
        ),
        (
            "$AP.value = $THREAD_7_124281;",
            "$AP.value = $THREAD_7_124281; $AP.value = other;",
        ),
        ("let $AO, $AP;", ""),
        ("onBeforeInvoke: $KEY_0", "onBeforeGet: $KEY_0"),
        ("super.register($AL);", "other.register($AL);"),
        (
            "let $KEY_215 = $THREAD_7_6923;",
            "let $KEY_215 = $THREAD_7_6923; $KEY_215 = other;",
        ),
        (
            "let $AI = $THREAD_7_72452($THREAD_1_54839);",
            "$THREAD_1_54839 = other; let $AI = $THREAD_7_72452($THREAD_1_54839);",
        ),
        (
            "let $AI = $THREAD_7_72452($THREAD_1_54839);",
            "new $AI(); let $AI = $THREAD_7_72452($THREAD_1_54839);",
        ),
        ("function bootstrap()", "function bootstrap(Object)"),
        (
            "this.callbacks.push($AK);",
            "this.otherCallbacks.push($AK);",
        ),
    ] {
        refuses(&changed(DUAL_OUTPUT, old, new));
    }
}

#[test]
fn lazy_registry_two_output_transport_refuses_preinitialization() {
    let source = changed(DUAL_OUTPUT, "let $AO, $AP;", "");
    refuses(&changed(
        &source,
        "if ($AO == null)",
        "let $AO, $AP; if ($AO == null)",
    ));
}

fn boolean_channel_source() -> String {
    changed(
        REVERSE,
        "if ($KEY_190?.onBeforeInvoke != null) {",
        "let selectedChannel; if ($KEY_190 != null) { selectedChannel = $KEY_190.onBeforeInvoke != null; } else { selectedChannel = $KEY_190 != null; } if (selectedChannel) {",
    )
}

#[test]
fn lazy_registry_boolean_channel_closed_flow_and_semantic_negatives() {
    let source = boolean_channel_source();
    accepts(&source);
    accepts(REVERSE);
    accepts(&staged_channel_source());
    for (old, new) in [
        ("let selectedChannel;", "let selectedChannel = true;"),
        ("let selectedChannel;", "var selectedChannel;"),
        (
            "let selectedChannel;",
            "function capture() { return selectedChannel; } let selectedChannel;",
        ),
        ("if ($KEY_190 != null)", "if (other != null)"),
        ("if ($KEY_190 != null)", "if ($KEY_190 == null)"),
        (
            "selectedChannel = $KEY_190 != null;",
            "selectedChannel = other != null;",
        ),
        (
            "selectedChannel = $KEY_190 != null;",
            "selectedChannel = true;",
        ),
        (
            "selectedChannel = $KEY_190 != null;",
            "selectedChannel = false;",
        ),
        (
            "selectedChannel = $KEY_190.onBeforeInvoke != null;",
            "selectedChannel = other.onBeforeInvoke != null;",
        ),
        (
            "selectedChannel = $KEY_190.onBeforeInvoke != null;",
            "selectedChannel = $KEY_190.onBeforeSet != null;",
        ),
        (
            "selectedChannel = $KEY_190.onBeforeInvoke != null;",
            "selectedChannel = $KEY_190.onBeforeInvoke == null;",
        ),
        (
            "selectedChannel = $KEY_190.onBeforeInvoke != null;",
            "function donor() { selectedChannel = $KEY_190.onBeforeInvoke != null; }",
        ),
        (
            "if (selectedChannel)",
            "external(selectedChannel); if (selectedChannel)",
        ),
        (
            "if (selectedChannel)",
            "selectedChannel = true; if (selectedChannel)",
        ),
        ("if (selectedChannel)", "if (selectedChannel != null)"),
        ("if (selectedChannel)", "if (false && selectedChannel)"),
        (
            "$KEY_351 = true;",
            "selectedChannel = false; $KEY_351 = true;",
        ),
    ] {
        refuses(&changed(&source, old, new));
    }
}

#[test]
fn lazy_registry_boolean_channel_real_failed_guard_and_two_vendor_origins() {
    for (source, class_start) in [
        (
            include_str!(
                "../../../tests/fixtures/program/lazy-registry/constructor-transport-full.js"
            ),
            415194,
        ),
        (
            include_str!(
                "../../../tests/fixtures/program/lazy-registry/constructor-transport-full-alternate.js"
            ),
            439187,
        ),
    ] {
        inspect(source, |model| {
            let (site, f, class) = model
                .all_sites()
                .iter()
                .find_map(|site| {
                    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
                        return None;
                    };
                    let class = returned_class(model, f)?;
                    (class.span.start == class_start).then_some((*site, f, class))
                })
                .expect("own returned class factory");
            let calls = model.calls_to(site);
            assert_eq!(calls.len(), 1);
            let before = calls[0].span.start;
            let base = symbol(model, calls[0].arguments[0].as_expression().unwrap()).unwrap();
            assert!(ordered_base(model, base, before).is_some());
            assert!(crate::naming::classes::callback_notifier::proves_base(
                model, base
            ));
            let (constructor, register) = methods(class).unwrap();
            assert!(forwards_arguments(model, constructor).is_some());
            let install = call(&body(register).unwrap().statements[0]).unwrap();
            let hook = stable_function(model, &install.callee, before).unwrap();
            let AstKind::Function(inst) = model.nodes.kind(hook.node_id) else {
                panic!("installer");
            };
            let phase = installer_phase(model, inst, before).expect("ordered lifecycle gate");
            let mut diagnosed = false;
            for (index, statement) in phase.iter().enumerate() {
                let Statement::IfStatement(guard) = statement else {
                    continue;
                };
                let Some([statement]) = block_statements(&guard.consequent) else {
                    continue;
                };
                let Some(reg) = call(statement) else {
                    continue;
                };
                let Some((channel, "register")) = member(&reg.callee) else {
                    continue;
                };
                let Some((receiver, "onBeforeInvoke")) = member(channel) else {
                    continue;
                };
                let receiver_symbol = symbol(model, receiver).unwrap();
                if staged_channel_variant(
                    model,
                    &phase[..index],
                    &guard.test,
                    receiver_symbol,
                    true,
                )
                .is_none()
                {
                    continue;
                }
                assert!(!channel_guard(model, &guard.test, receiver_symbol));
                assert!(
                    staged_channel_variant(
                        model,
                        &phase[..index],
                        &guard.test,
                        receiver_symbol,
                        false
                    )
                    .is_none(),
                    "precise original nullable-carrier guard rejection"
                );
                assert!(
                    hook_receiver(model, receiver, before).is_some(),
                    "complete typed descriptor/cache/wrapper dependency"
                );
                eprintln!("class={:?} original_nullable_guard=false boolean_guard=true typed_receiver=true", class.span);
                diagnosed = true;
            }
            assert!(
                diagnosed,
                "real boolean guard found in actual captured installer"
            );
            assert!(
                installer(model, &install.callee, before).is_some(),
                "complete callback accumulator contract"
            );
            assert!(
                matches!(super::prove(model, site, f), Some(FinderClaim::ClassBinding { class_span, .. }) if class_span == class.span)
            );
        });
    }
}

// Full original input; diagnostics isolate independent proof obligations.
#[test]
fn lazy_registry_post_loop_copy_guard_reproduction() {
    post_loop_copy_accepts(
        include_str!("../../../tests/fixtures/program/lazy-registry/post-loop-copy-full.js"),
        455890,
    );
    post_loop_copy_accepts(
        include_str!("../../../tests/fixtures/program/lazy-registry/post-loop-copy-full-alternate.js"),
        472337,
    );
    post_loop_copy_accepts(
        include_str!("../../../tests/fixtures/program/lazy-registry/post-loop-copy-full-heldout.js"),
        491011,
    );
}
fn post_loop_copy_accepts(source: &str, start: u32) {
    inspect(source, |model| {
        let site = model
            .all_sites()
            .iter()
            .find(|s| s.span.start == start)
            .copied()
            .unwrap();
        let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
            panic!()
        };
        let class = returned_class(model, f).unwrap();
        let (_, register) = methods(class).unwrap();
        let installation = call(&body(register).unwrap().statements[0]).unwrap();
        let before = model.calls_to(site)[0].span.start;
        let actual = symbol(
            model,
            model.calls_to(site)[0].arguments[0]
                .as_expression()
                .unwrap(),
        )
        .unwrap();
        let hook = stable_function(model, &installation.callee, before).unwrap();
        let AstKind::Function(installer_function) = model.nodes.kind(hook.node_id) else {
            panic!()
        };
        eprintln!(
            "base={} installer_phase={}",
            crate::naming::classes::callback_notifier::proves_base(model, actual),
            installer_phase(model, installer_function, before).is_some()
        );
        for k in owned(model, hook) {
            let AstKind::CallExpression(c) = k else {
                continue;
            };
            let Some((channel, "register")) = member(&c.callee) else {
                continue;
            };
            let Some((receiver, "onBeforeInvoke")) = member(channel) else {
                continue;
            };
            let callback =
                stable_function(model, c.arguments[0].as_expression().unwrap(), c.span.start)
                    .unwrap();
            eprintln!(
                "register@{} hook_receiver={} callback_protocol={}",
                c.span.start,
                hook_receiver(model, receiver, before).is_some(),
                callback_protocol(model, callback, hook).is_some()
            );
        }
        assert!(
            super::prove(model, site, f).is_some(),
            "full own lazy registry proof"
        );
    });
}

#[test]
fn lazy_registry_post_loop_copy_post_loop_copy_alpha_and_refusals() {
    let source = include_str!("../../../tests/fixtures/program/lazy-registry/post-loop-copy-full.js");
    post_loop_copy_accepts(
        &source
            .replace("$AX", "$ZX")
            .replace("$AY", "$ZY"),
        455890,
    );
    for (old, new) in [
        ("$AY = $AX;", "$AY = foreignRecord;"),
        ("$AY = $AX;", "function donor() { $AY = $AX; }"),
        ("$AY = $AX;", "$AY = $AX; external($AY);"),
        (
            "$AY = $AX;",
            "$AY = $AX; function capture() { return $AX; }",
        ),
        ("$AY = $AX;", "$AY = $AX; $AY = other;"),
        ("$AX = void 0;", "$AX = effect();"),
        ("$KEY_280[$AO][$AZ]", "$KEY_280[otherKey][$AZ]"),
        (
            "if ($AT !== $AX.object)",
            "if (otherReceiver !== $AX.object)",
        ),
        (
            "$KEY_147.onBeforeInvoke.register",
            "$KEY_147.onBeforeSet.register",
        ),
        ("$KEY_154 = true;", "$KEY_154 = false;"),
        ("super.register($AO);", "super.register(otherArgument);"),
    ] {
        refuses(&changed(source, old, new));
    }
}
