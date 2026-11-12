use shape::naming::name_threads;

// Whole-body owner shape reduced to inert source with the same scoped
// identities. Nothing here is executed; the fixture is only parsed.
const POSITIVE: &str = r#"function host(){
let factory, config, receiver, registry, groups, groupIndex, groupSpec, errorLogger, makeOptions, sinkFactory, valueSink, groupConnector, unsubscribers, queueA, pendingFlag, pendingList;
function $THREAD_71_8123(){
    queueA = factory(config, receiver.global.setTimeout);
    let queueB = factory(config, task => task());
    let startB = queueB.start;
    if (typeof setTimeout !== "function") {
        startB();
    } else if (pendingFlag) {
        setTimeout(startB, 0);
    } else if (pendingList) {
        pendingList.push(startB);
    }
    let startA = queueA.start;
    if (typeof setTimeout !== "function") {
        startA();
    } else if (pendingFlag) {
        setTimeout(startA, 0);
    } else if (pendingList) {
        pendingList.push(startA);
    }
    registry.forEach((entry, index) => {
        let idx = index;
        let ctor = entry.ctor;
        let priority = entry.priority;
        let debugName = entry.debugName;
        if (ctor == null) {
            return void 0;
        } else {
            try {
                let errorConfig = { errorLogger: errorLogger.bind(null, index, false) };
                let subscription = ctor(makeOptions(errorConfig, receiver), queueA.enqueue, queueB.enqueue, priority, value => valueSink(value, idx), idx, debugName).subscribe(sinkFactory(idx));
                unsubscribers.push(() => subscription.unsubscribe());
            } catch (caught) {
                errorLogger(idx, false, caught);
            }
            return void 0;
        }
    });
    groups.forEach((group, index) => {
        let groupIndices = groupIndex[index];
        let groupSpecEntry = groupSpec[index];
        let sinks = groupIndices.map(sinkFactory);
        let groupError = { errorLogger: void 0 };
        groupError.errorLogger = errorLogger.bind(null, groupIndices[0], true);
        sinks = groupConnector(sinks, group, makeOptions(groupError, receiver), queueA.enqueue, queueB.enqueue, groupSpecEntry, value => valueSink(value, groupIndices[0]));
        unsubscribers.push(sinks);
    });
    return queueA.onEmpty;
}
return $THREAD_71_8123;
}
host();"#;

// Alpha renaming: generated thread identity and every scoped capture/local
// spelling changes. Nothing about the rule may depend on seed spellings.
const ALPHA: &str = r#"function host(){
let $KEY_90, $KEY_91, $KEY_92, $KEY_93, $KEY_94, $KEY_95, $KEY_96, $KEY_97, $KEY_98, $KEY_99, $KEY_100, $KEY_101, $KEY_102, $KEY_103, $KEY_104, $KEY_105;
function $THREAD_900_777_2(){
    $KEY_103 = $KEY_90($KEY_91, $KEY_92.global.setTimeout);
    let $KEY_106 = $KEY_90($KEY_91, $KEY_107 => $KEY_107());
    let $KEY_108 = $KEY_106.start;
    if (typeof setTimeout !== "function") {
        $KEY_108();
    } else if ($KEY_104) {
        setTimeout($KEY_108, 0);
    } else if ($KEY_105) {
        $KEY_105.push($KEY_108);
    }
    let $KEY_109 = $KEY_103.start;
    if (typeof setTimeout !== "function") {
        $KEY_109();
    } else if ($KEY_104) {
        setTimeout($KEY_109, 0);
    } else if ($KEY_105) {
        $KEY_105.push($KEY_109);
    }
    $KEY_93.forEach(($KEY_110, $KEY_111) => {
        let $KEY_112 = $KEY_111;
        let $KEY_113 = $KEY_110.ctor;
        let $KEY_114 = $KEY_110.priority;
        let $KEY_115 = $KEY_110.debugName;
        if ($KEY_113 == null) {
            return void 0;
        } else {
            try {
                let $KEY_116 = { errorLogger: $KEY_97.bind(null, $KEY_111, false) };
                let $KEY_117 = $KEY_113($KEY_99($KEY_116, $KEY_92), $KEY_103.enqueue, $KEY_106.enqueue, $KEY_114, $KEY_118 => $KEY_101($KEY_118, $KEY_112), $KEY_112, $KEY_115).subscribe($KEY_100($KEY_112));
                $KEY_102.push(() => $KEY_117.unsubscribe());
            } catch ($KEY_119) {
                $KEY_97($KEY_112, false, $KEY_119);
            }
            return void 0;
        }
    });
    $KEY_94.forEach(($KEY_120, $KEY_121) => {
        let $KEY_122 = $KEY_95[$KEY_121];
        let $KEY_123 = $KEY_96[$KEY_121];
        let $KEY_124 = $KEY_122.map($KEY_100);
        let $KEY_125 = { errorLogger: void 0 };
        $KEY_125.errorLogger = $KEY_97.bind(null, $KEY_122[0], true);
        $KEY_124 = $KEY_98($KEY_124, $KEY_120, $KEY_99($KEY_125, $KEY_92), $KEY_103.enqueue, $KEY_106.enqueue, $KEY_123, $KEY_126 => $KEY_101($KEY_126, $KEY_122[0]));
        $KEY_102.push($KEY_124);
    });
    return $KEY_103.onEmpty;
}
return $THREAD_900_777_2;
}
host();"#;

// A captured provider namespace carries the timer member. This is consumer
// input, not a native/static callee claim, and the `.call(void 0, ...)` form is
// still the same three-way fallback.
const NAMESPACE_TIMER: &str = r#"function host(){
let factory, config, receiver, hostWindow, registry, groups, groupIndex, groupSpec, errorLogger, makeOptions, sinkFactory, valueSink, groupConnector, unsubscribers, queueA, pendingFlag, pendingList;
function $THREAD_71_8123(){
    queueA = factory(config, receiver.global.setTimeout);
    let queueB = factory(config, task => task());
    let startB = queueB.start;
    if (typeof hostWindow.setTimeout !== "function") {
        startB();
    } else if (pendingFlag) {
        hostWindow.setTimeout.call(void 0, startB, 0);
    } else if (pendingList) {
        pendingList.push(startB);
    }
    let startA = queueA.start;
    if (typeof hostWindow.setTimeout !== "function") {
        startA();
    } else if (pendingFlag) {
        hostWindow.setTimeout.call(void 0, startA, 0);
    } else if (pendingList) {
        pendingList.push(startA);
    }
    registry.forEach((entry, index) => {
        let idx = index;
        let ctor = entry.ctor;
        let priority = entry.priority;
        let debugName = entry.debugName;
        if (ctor == null) {
            return void 0;
        } else {
            try {
                let errorConfig = { errorLogger: errorLogger.bind(null, index, false) };
                let subscription = ctor(makeOptions(errorConfig, receiver), queueA.enqueue, queueB.enqueue, priority, value => valueSink(value, idx), idx, debugName).subscribe(sinkFactory(idx));
                unsubscribers.push(() => subscription.unsubscribe());
            } catch (caught) {
                errorLogger(idx, false, caught);
            }
            return void 0;
        }
    });
    groups.forEach((group, index) => {
        let groupIndices = groupIndex[index];
        let groupSpecEntry = groupSpec[index];
        let sinks = groupIndices.map(sinkFactory);
        let groupError = { errorLogger: void 0 };
        groupError.errorLogger = errorLogger.bind(null, groupIndices[0], true);
        sinks = groupConnector(sinks, group, makeOptions(groupError, receiver), queueA.enqueue, queueB.enqueue, groupSpecEntry, value => valueSink(value, groupIndices[0]));
        unsubscribers.push(sinks);
    });
    return queueA.onEmpty;
}
return $THREAD_71_8123;
}
host();"#;

// The recovered bodies pre-declare one callback temporary and assign it in the
// original position. Hoisting the subscription local changes the registry
// callback from five statements to six; hoisting the group options object
// changes the group callback from seven statements to eight. Both are ordinary
// single-write free locals and must stay resolved by scoped symbol identity.
const HOISTED_GROUP: (&str, &str) = (
    "        groupError.errorLogger = errorLogger.bind(null, groupIndices[0], true);\n        sinks = groupConnector(sinks, group, makeOptions(groupError, receiver),",
    "        groupError.errorLogger = errorLogger.bind(null, groupIndices[0], true);\n        let options = makeOptions(groupError, receiver);\n        sinks = groupConnector(sinks, group, options,",
);

// The deferred five-members inline the group error object directly in the
// connector's options call. The group callback drops from seven statements to
// five while preserving the same scoped errorLogger bind, groupIndices[0]
// index, `true` marker, receiver and options factory.
const INLINE_GROUP: (&str, &str) = (
    "        let groupError = { errorLogger: void 0 };\n        groupError.errorLogger = errorLogger.bind(null, groupIndices[0], true);\n        sinks = groupConnector(sinks, group, makeOptions(groupError, receiver),",
    "        sinks = groupConnector(sinks, group, makeOptions({ errorLogger: errorLogger.bind(null, groupIndices[0], true) }, receiver),",
);

const INLINE_GROUP_ALPHA: (&str, &str) = (
    "        let $KEY_125 = { errorLogger: void 0 };\n        $KEY_125.errorLogger = $KEY_97.bind(null, $KEY_122[0], true);\n        $KEY_124 = $KEY_98($KEY_124, $KEY_120, $KEY_99($KEY_125, $KEY_92),",
    "        $KEY_124 = $KEY_98($KEY_124, $KEY_120, $KEY_99({ errorLogger: $KEY_97.bind(null, $KEY_122[0], true) }, $KEY_92),",
);

/// Registry callback pre-declares `let subscription;` and assigns it inside the
/// try; everything else is the base body.
fn hoisted_subscription() -> String {
    POSITIVE
        .replace(
            "        let idx = index;\n        let ctor = entry.ctor;",
            "        let idx = index;\n        let subscription;\n        let ctor = entry.ctor;",
        )
        .replace(
            "                let subscription = ctor(",
            "                subscription = ctor(",
        )
}

/// Group callback hoists `let options = makeOptions(groupError, receiver);` and
/// passes the local to the connector; everything else is the base body.
fn hoisted_options() -> String {
    POSITIVE.replace(HOISTED_GROUP.0, HOISTED_GROUP.1)
}

/// Both proven callback temporaries in one owner body.
fn hoisted_both() -> String {
    hoisted_subscription().replace(HOISTED_GROUP.0, HOISTED_GROUP.1)
}

/// Both temporaries under a fully generated/renamed spelling.
fn alpha_both() -> String {
    ALPHA
        .replace(
            "        let $KEY_112 = $KEY_111;\n        let $KEY_113 = $KEY_110.ctor;",
            "        let $KEY_112 = $KEY_111;\n        let $KEY_200;\n        let $KEY_113 = $KEY_110.ctor;",
        )
        .replace(
            "                let $KEY_117 = $KEY_113(",
            "                $KEY_200 = $KEY_113(",
        )
        .replace("$KEY_117.unsubscribe()", "$KEY_200.unsubscribe()")
        .replace(
            "        $KEY_125.errorLogger = $KEY_97.bind(null, $KEY_122[0], true);\n        $KEY_124 = $KEY_98($KEY_124, $KEY_120, $KEY_99($KEY_125, $KEY_92),",
            "        $KEY_125.errorLogger = $KEY_97.bind(null, $KEY_122[0], true);\n        let $KEY_201 = $KEY_99($KEY_125, $KEY_92);\n        $KEY_124 = $KEY_98($KEY_124, $KEY_120, $KEY_201,",
        )
}

/// The deferred inline group-error literal with the base registry callback.
fn inline_group_error() -> String {
    let source = POSITIVE.replace(INLINE_GROUP.0, INLINE_GROUP.1);
    assert_ne!(source, POSITIVE, "inline group-error anchor missing");
    assert!(
        source.contains(
            "makeOptions({ errorLogger: errorLogger.bind(null, groupIndices[0], true) }, receiver)"
        ),
        "inline group-error literal missing after replacement"
    );
    source
}

/// Inline group error plus the pre-declared subscription temporary, matching the
/// two deferred members that hoist both callback temporaries.
fn hoisted_subscription_inline_group() -> String {
    let base = hoisted_subscription();
    let source = base.replace(INLINE_GROUP.0, INLINE_GROUP.1);
    assert_ne!(
        source, base,
        "inline group-error anchor missing from hoisted subscription body"
    );
    source
}

/// The inline group-error literal under a fully generated/renamed spelling.
fn alpha_inline_group() -> String {
    let source = ALPHA.replace(INLINE_GROUP_ALPHA.0, INLINE_GROUP_ALPHA.1);
    assert_ne!(source, ALPHA, "alpha inline group-error anchor missing");
    assert!(
        source.contains("$KEY_99({ errorLogger: $KEY_97.bind(null, $KEY_122[0], true) }, $KEY_92)"),
        "alpha inline group-error literal missing after replacement"
    );
    source
}

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
    let result = name_threads(source, true)
        .unwrap_or_else(|error| panic!("negative source must parse and analyze: {error}\n{source}"));
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
fn collector_start_whole_body_is_named() {
    for source in [POSITIVE, ALPHA] {
        assert!(
            named(source, "$THREAD_71_8123", "startRegisteredCollectors")
                || named(source, "$THREAD_900_777_2", "startRegisteredCollectors"),
            "did not name collector registry starter fixture:\n{source}"
        );
    }
    assert!(
        named(NAMESPACE_TIMER, "$THREAD_71_8123", "startRegisteredCollectors"),
        "did not name the captured-namespace timer variant"
    );
}

#[test]
fn collector_start_requires_two_queues_from_one_provided_factory() {
    refuses(
        POSITIVE,
        "$THREAD_71_8123",
        "startRegisteredCollectors",
        &[
            (
                "let queueB = factory(config, task => task());",
                "let queueB = otherFactory(config, task => task());",
            ),
            (
                "queueA = factory(config, receiver.global.setTimeout);",
                "queueA = factory(otherConfig, receiver.global.setTimeout);",
            ),
            (
                "queueA = factory(config, receiver.global.setTimeout);",
                "queueA = factory(config, receiver.global.setTimeout, extra);",
            ),
            (
                "let queueB = factory(config, task => task());",
                "let queueB = factory(config, task => other());",
            ),
            (
                "let queueB = factory(config, task => task());",
                "let queueB = factory(config, task => task(task));",
            ),
            (
                "queueA = factory(config, receiver.global.setTimeout);",
                "let queueA = factory(config, receiver.global.setTimeout);",
            ),
        ],
    );
}

#[test]
fn collector_start_requires_exact_three_way_fallback() {
    refuses(
        POSITIVE,
        "$THREAD_71_8123",
        "startRegisteredCollectors",
        &[
            (
                "    } else if (pendingFlag) {\n        setTimeout(startB, 0);\n    } else if (pendingList) {\n        pendingList.push(startB);\n    }",
                "    }",
            ),
            (
                "setTimeout(startB, 0);",
                "setInterval(startB, 0);",
            ),
            (
                "setTimeout(startB, 0);",
                "setTimeout(startB, 10);",
            ),
            (
                "setTimeout(startB, 0);",
                "setTimeout(startA, 0);",
            ),
            (
                "pendingList.push(startB);",
                "pendingList.push(startA);",
            ),
            (
                "pendingList.push(startB);",
                "pendingList.unshift(startB);",
            ),
            (
                "if (typeof setTimeout !== \"function\") {\n        startB();",
                "if (typeof setTimeout !== \"function\") {\n        startA();",
            ),
            (
                "if (typeof setTimeout !== \"function\") {\n        startB();",
                "if (typeof setTimeout === \"function\") {\n        startB();",
            ),
            (
                "let startB = queueB.start;",
                "let startB = queueA.start;",
            ),
            (
                "let startB = queueB.start;",
                "let startB = queueB.stop;",
            ),
        ],
    );
}

#[test]
fn collector_start_requires_registry_row_protocol() {
    refuses(
        POSITIVE,
        "$THREAD_71_8123",
        "startRegisteredCollectors",
        &[
            ("let ctor = entry.ctor;", "let ctor = entry.factory;"),
            ("let priority = entry.priority;", "let priority = entry.weight;"),
            ("let debugName = entry.debugName;", "let debugName = entry.label;"),
            ("if (ctor == null)", "if (priority == null)"),
            ("if (ctor == null)", "if (ctor != null)"),
            (
                "makeOptions(errorConfig, receiver)",
                "makeOptions(errorConfig, registry)",
            ),
            (
                "queueA.enqueue, queueB.enqueue, priority",
                "queueB.enqueue, queueA.enqueue, priority",
            ),
            (
                "value => valueSink(value, idx)",
                "value => valueSink(idx, value)",
            ),
            (
                "subscribe(sinkFactory(idx))",
                "subscribe(sinkFactory(index))",
            ),
            (
                "errorLogger.bind(null, index, false)",
                "errorLogger.bind(null, idx, false)",
            ),
            (
                "errorLogger.bind(null, index, false)",
                "errorLogger.bind(null, index, true)",
            ),
            (
                "errorLogger(idx, false, caught)",
                "errorLogger(index, false, caught)",
            ),
            (
                "unsubscribers.push(() => subscription.unsubscribe());",
                "unsubscribers.push(() => subscription.close());",
            ),
        ],
    );
}

#[test]
fn collector_start_requires_group_row_protocol() {
    refuses(
        POSITIVE,
        "$THREAD_71_8123",
        "startRegisteredCollectors",
        &[
            (
                "let groupIndices = groupIndex[index];",
                "let groupIndices = groupSpec[index];",
            ),
            (
                "let groupSpecEntry = groupSpec[index];",
                "let groupSpecEntry = groupIndex[index];",
            ),
            (
                "let sinks = groupIndices.map(sinkFactory);",
                "let sinks = groupIndices.filter(sinkFactory);",
            ),
            (
                "errorLogger.bind(null, groupIndices[0], true)",
                "errorLogger.bind(null, groupIndices[1], true)",
            ),
            (
                "errorLogger.bind(null, groupIndices[0], true)",
                "errorLogger.bind(null, groupIndices[0], false)",
            ),
            (
                "makeOptions(groupError, receiver)",
                "makeOptions(groupError, registry)",
            ),
            (
                "groupConnector(sinks, group, makeOptions(groupError, receiver), queueA.enqueue, queueB.enqueue, groupSpecEntry,",
                "groupConnector(group, sinks, makeOptions(groupError, receiver), queueA.enqueue, queueB.enqueue, groupSpecEntry,",
            ),
            (
                "value => valueSink(value, groupIndices[0])",
                "value => valueSink(value, groupIndices[1])",
            ),
            (
                "unsubscribers.push(sinks);",
                "unsubscribers.push(group);",
            ),
        ],
    );
}

#[test]
fn collector_start_requires_first_queue_on_empty_and_exact_order() {
    refuses(
        POSITIVE,
        "$THREAD_71_8123",
        "startRegisteredCollectors",
        &[
            ("return queueA.onEmpty;", "return queueB.onEmpty;"),
            ("return queueA.onEmpty;", "return queueA.onDrain;"),
            (
                "function $THREAD_71_8123(){",
                "function $THREAD_71_8123(extra){",
            ),
            (
                "function $THREAD_71_8123(){",
                "function $THREAD_71_8123(){ let local = factory;",
            ),
            (
                "    let queueB = factory(config, task => task());\n    let startB = queueB.start;",
                "    let startB = queueB.start;\n    let queueB = factory(config, task => task());",
            ),
            (
                "    registry.forEach((entry, index) => {",
                "    groups.forEach((entry, index) => {",
            ),
        ],
    );
}

#[test]
fn collector_start_does_not_rename_readable_bindings() {
    let human = POSITIVE.replace("$THREAD_71_8123", "startRegisteredCollectors");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
}

#[test]
fn collector_start_proves_hoisted_callback_temporaries() {
    for source in [
        hoisted_subscription(),
        hoisted_options(),
        hoisted_both(),
    ] {
        assert!(
            named(&source, "$THREAD_71_8123", "startRegisteredCollectors"),
            "did not name hoisted callback-temporary variant:\n{source}"
        );
    }
    let alpha = alpha_both();
    assert!(
        named(&alpha, "$THREAD_900_777_2", "startRegisteredCollectors"),
        "did not name alpha-renamed hoisted variant:\n{alpha}"
    );
}

#[test]
fn collector_start_refuses_hoisted_temporary_rebinding() {
    // Wrong receiver, wrong connector slot, and a second write on the hoisted
    // options local.
    refuses(
        &hoisted_both(),
        "$THREAD_71_8123",
        "startRegisteredCollectors",
        &[
            (
                "let options = makeOptions(groupError, receiver);",
                "let options = makeOptions(groupError, registry);",
            ),
            (
                "groupConnector(sinks, group, options, queueA.enqueue",
                "groupConnector(sinks, group, queueA.enqueue, options",
            ),
            (
                "let options = makeOptions(groupError, receiver);",
                "let options = makeOptions(groupError, receiver);\n        options = registry;",
            ),
        ],
    );
    // Wrong assignment target and a second write on the hoisted subscription
    // local.
    refuses(
        &hoisted_subscription(),
        "$THREAD_71_8123",
        "startRegisteredCollectors",
        &[
            (
                "                subscription = ctor(",
                "                priority = ctor(",
            ),
            (
                "                subscription = ctor(",
                "                subscription = subscription = ctor(",
            ),
        ],
    );
    // The hoisted initializers must stay direct calls, not nested donors.
    refuses(
        &hoisted_both(),
        "$THREAD_71_8123",
        "startRegisteredCollectors",
        &[(
            "let options = makeOptions(groupError, receiver);",
            "let options = (() => makeOptions(groupError, receiver))();",
        )],
    );
    let nested_subscription = hoisted_subscription()
        .replace(
            "                subscription = ctor(",
            "                subscription = (() => ctor(",
        )
        .replace(
            ").subscribe(sinkFactory(idx));",
            ").subscribe(sinkFactory(idx)))();",
        );
    refused(&nested_subscription, "$THREAD_71_8123", "startRegisteredCollectors");
}

#[test]
fn collector_start_requires_hoisted_temporaries_to_keep_cleanup() {
    refuses(
        &hoisted_subscription(),
        "$THREAD_71_8123",
        "startRegisteredCollectors",
        &[(
            "unsubscribers.push(() => subscription.unsubscribe());",
            "void 0;",
        )],
    );
    refuses(
        &hoisted_both(),
        "$THREAD_71_8123",
        "startRegisteredCollectors",
        &[
            ("unsubscribers.push(sinks);", "void 0;"),
            (
                "groupError.errorLogger = errorLogger.bind(null, groupIndices[0], true);",
                "void 0;",
            ),
        ],
    );
}

#[test]
fn collector_start_proves_inline_group_error_variants() {
    for source in [
        inline_group_error(),
        hoisted_subscription_inline_group(),
    ] {
        assert!(
            named(&source, "$THREAD_71_8123", "startRegisteredCollectors"),
            "did not name inline group-error variant:\n{source}"
        );
    }
    let alpha = alpha_inline_group();
    assert!(
        named(&alpha, "$THREAD_900_777_2", "startRegisteredCollectors"),
        "did not name alpha-renamed inline group-error variant:\n{alpha}"
    );
}

#[test]
fn collector_start_refuses_inline_group_error_near_misses() {
    let template = inline_group_error();
    refuses(
        &template,
        "$THREAD_71_8123",
        "startRegisteredCollectors",
        &[
            // Wrong receiver in the inline options call.
            (
                "makeOptions({ errorLogger: errorLogger.bind(null, groupIndices[0], true) }, receiver)",
                "makeOptions({ errorLogger: errorLogger.bind(null, groupIndices[0], true) }, registry)",
            ),
            // Wrong index in the bound group error logger.
            (
                "errorLogger.bind(null, groupIndices[0], true)",
                "errorLogger.bind(null, groupIndices[1], true)",
            ),
            // Neutralized group marker.
            (
                "errorLogger.bind(null, groupIndices[0], true)",
                "errorLogger.bind(null, groupIndices[0], false)",
            ),
            // Different error logger identity.
            (
                "errorLogger.bind(null, groupIndices[0], true)",
                "otherLogger.bind(null, groupIndices[0], true)",
            ),
            // Different options factory identity.
            (
                "makeOptions({ errorLogger: errorLogger.bind(null, groupIndices[0], true) }, receiver)",
                "otherOptions({ errorLogger: errorLogger.bind(null, groupIndices[0], true) }, receiver)",
            ),
            // Exactly one errorLogger property; extra properties are refused.
            (
                "{ errorLogger: errorLogger.bind(null, groupIndices[0], true) }",
                "{ errorLogger: errorLogger.bind(null, groupIndices[0], true), extra: void 0 }",
            ),
            // The property must carry the bind call, not an unset value.
            (
                "{ errorLogger: errorLogger.bind(null, groupIndices[0], true) }",
                "{ errorLogger: void 0 }",
            ),
            // Computed keys are not the same literal shape.
            (
                "{ errorLogger: errorLogger.bind(null, groupIndices[0], true) }",
                "{ [\"errorLogger\"]: errorLogger.bind(null, groupIndices[0], true) }",
            ),
            // Direct literal, not an IIFE-derived donor object.
            (
                "makeOptions({ errorLogger: errorLogger.bind(null, groupIndices[0], true) }, receiver)",
                "makeOptions((() => ({ errorLogger: errorLogger.bind(null, groupIndices[0], true) }))(), receiver)",
            ),
            // The options call must stay in connector slot 2.
            (
                "groupConnector(sinks, group, makeOptions({ errorLogger: errorLogger.bind(null, groupIndices[0], true) }, receiver), queueA.enqueue",
                "groupConnector(sinks, group, queueA.enqueue, makeOptions({ errorLogger: errorLogger.bind(null, groupIndices[0], true) }, receiver)",
            ),
            // Two-queue enqueue order is unchanged.
            (
                "queueA.enqueue, queueB.enqueue, groupSpecEntry",
                "queueB.enqueue, queueA.enqueue, groupSpecEntry",
            ),
            // The indexed group value sink stays at groupIndices[0].
            (
                "value => valueSink(value, groupIndices[0])",
                "value => valueSink(value, groupIndices[1])",
            ),
            // The recorded group unsubscribe is the connected sink list.
            (
                "unsubscribers.push(sinks);",
                "unsubscribers.push(group);",
            ),
        ],
    );
    // The inline literal form must still record the group unsubscribe.
    refuses(
        &template,
        "$THREAD_71_8123",
        "startRegisteredCollectors",
        &[("unsubscribers.push(sinks);", "void 0;")],
    );
    // A donor local carrying the same literal is a different shape; only the
    // direct inline literal (or the original two-statement local form) is in
    // scope.
    let donor = template.replace(
        "        let sinks = groupIndices.map(sinkFactory);\n        sinks = groupConnector(sinks, group, makeOptions({ errorLogger: errorLogger.bind(null, groupIndices[0], true) }, receiver),",
        "        let sinks = groupIndices.map(sinkFactory);\n        let groupError = { errorLogger: errorLogger.bind(null, groupIndices[0], true) };\n        sinks = groupConnector(sinks, group, makeOptions(groupError, receiver),",
    );
    assert_ne!(donor, template, "donor anchor missing");
    refused(&donor, "$THREAD_71_8123", "startRegisteredCollectors");
}
