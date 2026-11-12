use crate::signals::schema::ProgramPropertiesLimits;
use super::*;

fn prove(source: &str) -> Vec<FinderClaim> {
    let wrapped = format!("function bootstrap() {{\n{source}\n}} bootstrap();");
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &wrapped, SourceType::default()).parse();
    assert!(parsed.errors.is_empty(), "{:?}\n{source}", parsed.errors);
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let (scoping, nodes) = semantic.into_scoping_and_nodes();
    let model = Model::new(
        &wrapped,
        &parsed.program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .expect("negative fixtures also build a valid Model");
    claims(&model)
}
fn accepts(source: &str) {
    let claims = prove(source);
    assert_eq!(claims.len(), 1, "{source}");
    assert!(matches!(
        &claims[0],
        FinderClaim::ClassBinding {
            role: "owned_ring_buffer_class",
            ..
        }
    ));
}
fn refuses(source: &str) {
    assert!(prove(source).is_empty(), "{source}");
}
fn fixture(split: bool) -> String {
    let capacity = if split {
        SPLIT_CAPACITY
    } else {
        INLINE_CAPACITY
    };
    format!("{}\nlet Installed = Reference; let instance = new Installed(20); instance.put(1); let snapshot = instance.toArray(); instance.clear();", REFERENCE.replace("CAPACITY", capacity))
}
fn changed(source: &str, from: &str, to: &str) -> String {
    assert!(source.contains(from), "mutation must hit: {from}");
    source.replacen(from, to, 1)
}

#[test]
fn real_inline_and_three_split_excerpts_with_installation_and_consumers() {
    for source in [
        include_str!("../../../tests/fixtures/program/ring-buffer/inline-constructor.js"),
        include_str!("../../../tests/fixtures/program/ring-buffer/split-constructor-baseline.js"),
        include_str!("../../../tests/fixtures/program/ring-buffer/split-constructor-alternate.js"),
        include_str!("../../../tests/fixtures/program/ring-buffer/split-constructor-heldout.js"),
    ] {
        accepts(source);
    }
}

#[test]
fn scoped_formals_temporaries_and_stable_alias_transport() {
    for split in [false, true] {
        let source = fixture(split);
        accepts(&source);
        accepts(
            &source
                .replace("value", "input")
                .replace("capacity", "temporary"),
        );
        accepts(&changed(
            &source,
            "let Installed = Reference;",
            "let First = Reference; const Installed = First;",
        ));
        // Method declaration order carries no semantic role; statement order does.
        accepts(
            &changed(&source, "class Reference {", "class Renamed {")
                .replace("= Reference;", "= Renamed;"),
        );
    }
}

#[test]
fn refuses_partial_split_mutation_escape_and_wrong_capacity() {
    let source = fixture(true);
    for (from, to) in [
        ("else { capacity = value; }", "else {}"),
        ("capacity = value;", "capacity = 3;"),
        ("capacity = Infinity;", "capacity = value;"),
        ("this.n = capacity;", "capacity = 20; this.n = capacity;"),
        (
            "this.n = capacity;",
            "consume(capacity); this.n = capacity;",
        ),
        ("this.n = capacity;", "this.n = capacity; this.n = 20;"),
        ("value === 0", "value === 1"),
        ("typeof value !== 'number'", "typeof value === 'number'"),
        ("this.arr = []; }", "this.arr = []; value = 20; }"),
    ] {
        refuses(&changed(&source, from, to));
    }
}

#[test]
fn refuses_wrong_receiver_snapshot_order_and_donor_bodies() {
    for split in [false, true] {
        let source = fixture(split);
        for (from, to) in [
            ("this.arr[this.i] = value", "this.arr[this.length] = value"),
            ("this.arr[this.i] = value", "this.arr[this.i] = 1"),
            (
                "this.i = Number(this.i) + 1",
                "this.i = Number(this.length) + 1",
            ),
            ("this.length < this.n", "this.length <= this.n"),
            ("this.i >= this.n", "this.i >= this.length"),
            (
                "this.arr.slice(this.i, this.n).concat(this.arr.slice(0, this.i))",
                "this.arr.slice(0, this.i).concat(this.arr.slice(this.i, this.n))",
            ),
            (
                "this.arr.slice(0, this.length)",
                "other.arr.slice(0, this.length)",
            ),
            ("clear() { this.arr = [];", "clear() { other.arr = [];"),
            (
                "this.length = 0; this.i = 0;",
                "this.length = 1; this.i = 0;",
            ),
            (
                "toArray() {",
                "toArray() { function donor() { return other.arr.slice(0, this.length); }",
            ),
            (
                "return this.arr.slice(0, this.length);",
                "return (() => this.arr.slice(0, this.length))();",
            ),
            ("put(value) {", "static put(value) {"),
            ("toArray() {", "toArray() {} toArray() {"),
            ("this.arr = []; }", "this.arr = []; this.n = 99; }"),
        ] {
            refuses(&changed(&source, from, to));
        }
    }
}

#[test]
fn refuses_constructor_alias_writes_escapes_order_and_shadowed_globals() {
    let source = fixture(false);
    for addition in [
        "Reference = Other;",
        "Installed = Other;",
        "Installed.prototype.clear = donor;",
        "consume(Installed);",
        "Reference.put = donor;",
        "let holder = {Installed};",
    ] {
        refuses(&format!("{source}\n{addition}"));
    }
    refuses(&format!("new Reference(2);\n{source}"));
    refuses(&format!("let early = Installed;\n{source}"));
    refuses(&format!("let Infinity = 42;\n{source}"));
    refuses(&format!("let Number = transform;\n{source}"));
    refuses(&changed(
        &source,
        "constructor(value)",
        "constructor(value, extra)",
    ));
    refuses(&changed(
        &source,
        "let Installed = Reference;",
        "let Installed = other.Reference;",
    ));
}

const RESIDUALS: [&str; 2] = [
    include_str!("../../../tests/fixtures/program/ring-buffer/residual-origin-baseline.js"),
    include_str!("../../../tests/fixtures/program/ring-buffer/residual-origin-alternate.js"),
];

#[test]
fn real_split_predicate_variants_retain_carriers_and_consumers() {
    for source in RESIDUALS {
        accepts(source);
    }
    let source = fixture(true).replace(SPLIT_CAPACITY, SPLIT_PREDICATE_CAPACITY);
    accepts(&source);
    accepts(
        &source
            .replace("invalid", "predicate")
            .replace("value", "input"),
    );
    accepts(&changed(
        &source,
        "let Installed = Reference;",
        "const First = Reference; let Installed = First;",
    ));
}

#[test]
fn actual_split_predicate_nearest_misses_have_valid_models() {
    for (source, value, predicate) in [(RESIDUALS[0], "$BP", "$BQ"), (RESIDUALS[1], "$AL", "$AM")] {
        for (from, to) in [
            (format!("{value} === 0"), format!("{value} === 1")),
            (
                format!("{predicate} = true"),
                format!("{predicate} = false"),
            ),
            (
                format!("typeof {value} !== \"number\""),
                format!("typeof {value} === \"number\""),
            ),
            (
                format!("{predicate} = typeof {value} !== \"number\";"),
                String::new(),
            ),
            (
                format!("{predicate} ? Infinity : {value}"),
                format!("{predicate} ? {value} : Infinity"),
            ),
            (
                format!("this.n = {predicate} ? Infinity : {value};"),
                format!("this.n = {predicate} ? Infinity : 20;"),
            ),
            (
                format!("this.n = {predicate}"),
                format!("{predicate} = true; this.n = {predicate}"),
            ),
            (
                format!("this.n = {predicate}"),
                format!("consume({predicate}); this.n = {predicate}"),
            ),
            (
                "this.length < this.n".into(),
                "this.length <= this.n".into(),
            ),
            ("this.i >= this.n".into(), "this.i > this.n".into()),
            (
                "this.arr.slice(this.i, this.n).concat(this.arr.slice(0, this.i))".into(),
                "this.arr.slice(0, this.i).concat(this.arr.slice(this.i, this.n))".into(),
            ),
            (
                "return this.arr.slice(0, this.length);".into(),
                "function donor() { return this.arr.slice(0, this.length); } return [];".into(),
            ),
            ("this.arr[this.i]".into(), "other.arr[this.i]".into()),
            (
                "this.i = Number(this.i) + 1".into(),
                "this.i = Number(this.length) + 1".into(),
            ),
            ("clear() {".into(), "clear() { this.n = 20;".into()),
            ("put(".into(), "extra() {} put(".into()),
        ] {
            refuses(&changed(source, &from, &to));
        }
        let capacity = format!("this.n = {predicate} ? Infinity : {value};");
        // Moving capacity selection before the exhaustive predicate is invalid.
        let reordered = changed(source, &capacity, "");
        refuses(&changed(
            &reordered,
            &format!("let {predicate};"),
            &format!("let {predicate}; {capacity}"),
        ));
    }
}

#[test]
fn actual_split_predicate_constructor_transport_stays_closed() {
    for (source, constructor, alias) in [
        (RESIDUALS[0], "$THREAD_7_110556", "$KEY_386"),
        (RESIDUALS[1], "$THREAD_3_141163", "$KEY_387"),
    ] {
        for addition in [
            format!("{constructor} = Other;"),
            format!("{alias} = Other;"),
            format!("{alias}.prototype.clear = donor;"),
            format!("consume({alias});"),
            format!("let bag = {{ value: {constructor} }};"),
            format!("let readonly = {alias}.name;"),
            format!("{constructor}.extra = donor;"),
        ] {
            refuses(&format!("{source}\n{addition}"));
        }
        refuses(&format!("new {constructor}(20);\n{source}"));
        refuses(&format!("let early = {alias};\n{source}"));
        refuses(&format!("let Infinity = 42;\n{source}"));
        refuses(&format!("let Number = transform;\n{source}"));
    }
}

fn epoch_fixture(transport: &str) -> String {
    format!(
        "{}\n{transport}",
        REFERENCE.replace("CAPACITY", INLINE_CAPACITY)
    )
}

#[test]
fn ring_buffer_assignment_epoch_assignment_epochs_and_hoisted_consumers() {
    for transport in [
        "let Installed; Installed = Reference; new Installed(2);",
        "let Installed; Installed = Reference; let Copy; Copy = Installed; new Copy(2); Installed = Reference; new Installed(3);",
        "let Installed; Installed = Reference; function later() { new Installed(2); } later();",
        "let Installed; Installed = Reference; function later() { new Installed(2); } publish(later);",
        "let Installed; if (condition) { Installed = Reference; new Installed(2); } else { Installed = Reference; new Installed(3); }",
    ] {
        accepts(&epoch_fixture(transport));
        accepts(&epoch_fixture(transport).replace("Reference", "Renamed").replace("Installed", "Carrier"));
    }
    let second = REFERENCE
        .replace("CAPACITY", INLINE_CAPACITY)
        .replace("Reference", "Second");
    let source = epoch_fixture(&format!("{second}\nlet Installed; Installed = Reference; new Installed(2); Installed = Second; new Installed(3);"));
    assert_eq!(prove(&source).len(), 2);
    refuses(&source.replace("class Second {", "class Second extends Foreign {"));
    refuses(&source.replacen("class Second {", "class Second { extra() {}", 1));
    refuses(&source.replacen(
        "class Second {",
        "class Second { get extra() { return 1; }",
        1,
    ));
}

#[test]
fn ring_buffer_assignment_epoch_closed_epoch_nearest_misses() {
    for transport in [
        "let Installed; new Installed(2); Installed = Reference;",
        "let Installed; let Copy = Installed; Installed = Reference; new Copy(2);",
        "let Installed; Installed = Reference; Installed = Foreign; new Installed(2);",
        "let Installed; Installed = Foreign; Installed = Reference; new Installed(2);",
        "let Installed; if (condition) { Installed = Reference; } new Installed(2);",
        "let Installed; if (condition) { Installed = Reference; } else { new Installed(2); }",
        "let Installed; function later() { new Installed(2); } later(); Installed = Reference;",
        "let Installed; function later() { new Installed(2); } publish(later); Installed = Reference;",
        "let Installed; function later() { new Installed(2); } Installed = Reference; if (condition) { later(); } Installed = Foreign;",
        "let Installed; Installed = Reference; Installed.prototype.put = donor; new Installed(2);",
        "let Installed; Installed = Reference; Installed.put = donor; new Installed(2);",
        "let Installed; Installed = Reference; consume(Installed); new Installed(2);",
        "let Installed; Installed = Reference; Installed.name; new Installed(2);",
        "let Installed; Installed = Reference; Object.getPrototypeOf(Installed); new Installed(2);",
        "let Installed; Installed = Reference; eval('Installed = Other'); new Installed(2);",
        "let Installed; Installed = Reference; with (foreign) { new Installed(2); }",
        "let Installed; Installed = Reference; function later() { new Installed(2); } function reentry() { later(); } publish(reentry); reentry(); Installed = Foreign;",
        "let Installed; Installed = Reference; (() => { new Installed(2); })();",
        "let Installed; Installed = Reference; function later() { new Installed(2); } new Reference(1); Installed = Foreign; publish(later);",
        "let Installed; try { Installed = Reference; } catch (error) {} new Installed(2);",
        "let Installed; if (condition) { Installed = Reference; } else { Installed = Foreign; } new Installed(2);",
        "let Installed; Installed ||= Reference; new Installed(2);",
    ] {
        refuses(&epoch_fixture(transport));
    }
    let source = epoch_fixture("let Installed; Installed = Reference; new Installed(2);");
    for (from, to) in [
        ("Number(this.i)", "this.i"),
        ("Number(this.length)", "String(this.length)"),
        ("this.length < this.n", "this.length <= this.n"),
        ("this.i >= this.n", "this.i > this.n"),
        ("this.arr[this.i]", "this.arr[this.length]"),
        ("this.n = value", "this.n = 1 + value"),
        ("this.i = 0; }", "this.i = 1; }"),
        ("clear() {", "clear() { try {} catch (error) {}"),
        ("class Reference {", "class Reference extends Other {"),
        (
            "this.arr.slice(0, this.length)",
            "this.arr.slice(this.length, 0)",
        ),
        (
            "this.arr.slice(this.i, this.n).concat(this.arr.slice(0, this.i))",
            "this.arr.slice(0, this.i).concat(this.arr.slice(this.i, this.n))",
        ),
        (
            "this.arr = []; this.length = 0; this.i = 0;",
            "this.arr = []; this.i = 0;",
        ),
        ("typeof value !== 'number'", "typeof value === 'number'"),
    ] {
        refuses(&changed(&source, from, to));
    }
}

const ASSIGNMENT_EPOCH_SOURCE: &str = include_str!("../../../tests/fixtures/program/ring-buffer-constructor-epochs/constructor-epoch-parent.js");

#[test]
fn ring_buffer_assignment_epoch_whole_actual_model_legacy_guard_and_epoch_contract() {
    use oxc_span::GetSpan;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, ASSIGNMENT_EPOCH_SOURCE, SourceType::default()).parse();
    assert!(parsed.errors.is_empty());
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let (scoping, nodes) = semantic.into_scoping_and_nodes();
    let model = Model::new(
        ASSIGNMENT_EPOCH_SOURCE,
        &parsed.program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    let mut candidates = 0;
    for node in nodes.iter() {
        let AstKind::Class(class) = node.kind() else {
            continue;
        };
        if !methods(&scoping, class).is_some_and(|m| grammar().contains(&m)) {
            continue;
        }
        candidates += 1;
        let symbol = class.id.as_ref().unwrap().symbol_id.get().unwrap();
        let mut constructed = false;
        assert!(!constructor_uses(
            &model,
            symbol,
            class.span.end,
            &mut BTreeSet::new(),
            &mut constructed
        ));
        for reference in scoping.get_resolved_references(symbol) {
            eprintln!(
                "candidate {:?}: legacy fails at {:?}: {}",
                class.span,
                nodes.parent_kind(reference.node_id()).span(),
                &ASSIGNMENT_EPOCH_SOURCE[nodes.parent_kind(reference.node_id()).span().start as usize
                    ..nodes.parent_kind(reference.node_id()).span().end as usize]
            );
        }
        assert!(
            epoch_constructor_uses(&model, symbol),
            "epoch proof refused {:?}",
            class.span
        );
    }
    assert_eq!(candidates, 4);
    assert_eq!(claims(&model).len(), 4);
}

#[test]
fn ring_buffer_assignment_epoch_whole_actual_early_alias_and_publication_refused() {
    for source in [
        ASSIGNMENT_EPOCH_SOURCE.replacen(
            "$KEY_441 = $THREAD_0_89535;",
            "let early = $KEY_441; $KEY_441 = $THREAD_0_89535;",
            1,
        ),
        ASSIGNMENT_EPOCH_SOURCE.replacen(
            "$KEY_441 = $THREAD_0_89535;",
            "publish($THREAD_3_43811); $KEY_441 = $THREAD_0_89535;",
            1,
        ),
    ] {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &source, SourceType::default()).parse();
        assert!(parsed.errors.is_empty());
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let (scoping, nodes) = semantic.into_scoping_and_nodes();
        let model = Model::new(
            &source,
            &parsed.program,
            &scoping,
            &nodes,
            ProgramPropertiesLimits::default(),
        )
        .unwrap();
        assert!(claims(&model).is_empty());
    }
}

#[test]
fn ring_buffer_assignment_epoch_each_producer_and_transitive_publication_is_required() {
    let second = REFERENCE
        .replace("CAPACITY", INLINE_CAPACITY)
        .replace("Reference", "Second");
    let transport = "let Installed; Installed = Reference; new Installed(2); Installed = Second; new Installed(3);";
    assert_eq!(
        prove(&epoch_fixture(&format!("{second}\n{transport}"))).len(),
        2
    );
    for (from, to) in [
        ("Number(this.i)", "Number(this.length)"),
        ("this.n = value === 0", "this.n = value === 1"),
        (
            "this.arr = []; this.length = 0; this.i = 0;",
            "this.arr = []; this.length = 0;",
        ),
    ] {
        let bad_producer = changed(&second, from, to);
        refuses(&epoch_fixture(&format!("{bad_producer}\n{transport}")));
    }
    accepts(&epoch_fixture("let Installed; Installed = Reference; function later() { new Installed(2); } function outer() { later(); } outer();"));
    refuses(&epoch_fixture("let Installed; function outer() { later(); } outer(); Installed = Reference; function later() { new Installed(2); }"));
    refuses(&epoch_fixture("let Installed; function outer() { later(); } publish(outer); Installed = Reference; function later() { new Installed(2); }"));
    refuses(&epoch_fixture(
        "let Installed; Installed = Reference; (eval)('Installed = Other'); new Installed(2);",
    ));
}

#[test]
fn ring_buffer_assignment_epoch_destination_activation_and_immutable_bindings() {
    refuses(&epoch_fixture(
        "Installed = Reference; let Installed; new Installed(2);",
    ));
    refuses(&epoch_fixture(
        "const Installed = Reference; Installed = Reference; new Installed(2);",
    ));
    refuses(&epoch_fixture(
        "let Installed; Installed = Reference; Reference = Reference; new Installed(2);",
    ));
    accepts(&epoch_fixture(
        "Installed = Reference; var Installed; new Installed(2);",
    ));
    accepts(&epoch_fixture(
        "let Installed, unrelated; Installed = Reference; new Installed(2);",
    ));
}

// Generate controls from the unchanged saved source, retaining every constructor read and
// the complete own-method grammar.
fn ring_buffer_constructor_entry_helper_source(kind: &str) -> String {
    let replacement = match kind {
        "inert" => "function reviewNeverCalled(){ new $KEY_441(2); } $KEY_441 = $THREAD_0_89535;",
        "early" => "function reviewCalledEarly(){ new $KEY_441(2); } reviewCalledEarly(); $KEY_441 = $THREAD_0_89535;",
        "postinit" => "function reviewCalledAfter(){ new $KEY_441(2); } $KEY_441 = $THREAD_0_89535; reviewCalledAfter();",
        _ => panic!("unknown helper control"),
    };
    changed(ASSIGNMENT_EPOCH_SOURCE, "$KEY_441 = $THREAD_0_89535;", replacement)
}

fn ring_buffer_constructor_entry_actual_claims(source: &str) -> Vec<FinderClaim> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::default()).parse();
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let built = SemanticBuilder::new().build(&parsed.program);
    assert!(built.errors.is_empty(), "{:?}", built.errors);
    let (scoping, nodes) = built.semantic.into_scoping_and_nodes();
    let model = Model::new(
        source,
        &parsed.program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    let mut candidate_count = 0;
    for node in nodes.iter() {
        let AstKind::Class(class) = node.kind() else {
            continue;
        };
        if !methods(&scoping, class).is_some_and(|m| grammar().contains(&m)) {
            continue;
        }
        candidate_count += 1;
        let symbol = class.id.as_ref().unwrap().symbol_id.get().unwrap();
        assert!(!constructor_uses(
            &model,
            symbol,
            class.span.end,
            &mut BTreeSet::new(),
            &mut false
        ));
    }
    assert_eq!(
        candidate_count, 4,
        "negative must retain all four complete producer grammars"
    );
    claims(&model)
}

#[test]
fn ring_buffer_constructor_entry_model_controls() {
    assert_eq!(ring_buffer_constructor_entry_actual_claims(ASSIGNMENT_EPOCH_SOURCE).len(), 4);
    for kind in ["inert", "postinit"] {
        assert_eq!(
            ring_buffer_constructor_entry_actual_claims(&ring_buffer_constructor_entry_helper_source(kind)).len(),
            4,
            "{kind}"
        );
    }
    assert!(ring_buffer_constructor_entry_actual_claims(&ring_buffer_constructor_entry_helper_source("early")).is_empty());
    eprintln!("producer readiness Model: inert=4, postinit=4, early=0, original=4; all four producer grammars retained");
}

#[test]
fn ring_buffer_constructor_entry_full_production_controls() {
    use crate::naming::name_threads;
    for (kind, expected) in [("inert", 4), ("postinit", 4), ("early", 0)] {
        let source = ring_buffer_constructor_entry_helper_source(kind);
        let named = name_threads(&source, true).expect("actual source production naming");
        let report = named.report;
        let rings: Vec<_> = report
            .class_outcomes
            .iter()
            .filter(|o| o.applied && o.evaluator.as_deref() == Some("owned_ring_buffer_class"))
            .collect();
        assert_eq!(rings.len(), expected, "{kind}");
        assert_eq!(report.class_outcomes.len(), 20);
        assert_eq!(
            report.class_outcomes.iter().filter(|o| !o.applied).count(),
            13 - expected
        );
        for outcome in rings {
            assert_eq!(outcome.reference_edits, 1);
            assert!(outcome
                .proposed_name
                .as_ref()
                .unwrap()
                .starts_with("RingBuffer"));
            assert!(outcome.explanation[0].contains("every scoped constructor producer"));
        }
        let mut replay = source;
        for edit in report.edits.iter().rev() {
            replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
        }
        assert_eq!(replay, named.source, "{kind} exact edit replay");
        eprintln!(
            "producer readiness full production: {kind}={expected}, unresolved={}",
            13 - expected
        );
    }
}

#[test]
fn ring_buffer_constructor_entry_local_callable_aliases_and_entry_order() {
    for body in [
        "let Installed; function helper() { new Installed(2); } Installed = Reference; new Installed(3);",
        "let Installed; function helper() { new Installed(2); } Installed = Reference; helper();",
        "let Installed; function helper() { new Installed(2); } const cb = helper; Installed = Reference; cb();",
        "let Installed; function helper() { new Installed(2); } const first = helper; const cb = first; Installed = Reference; publish(cb);",
        "let Installed; function helper() { new Installed(2); } const cb = helper; Installed = Reference; new Installed(3);",
    ] { accepts(&epoch_fixture(body)); }
    for body in [
        "let Installed; function helper() { new Installed(2); } helper(); Installed = Reference;",
        "let Installed; function helper() { new Installed(2); } const cb = helper; cb(); Installed = Reference;",
        "let Installed; function helper() { new Installed(2); } const first = helper; const cb = first; publish(cb); Installed = Reference;",
        "let Installed; function helper() { new Installed(2); } const cb = first; const first = helper; Installed = Reference; cb();",
        "let Installed; function helper() { new Installed(2); } let cb = helper; cb = foreign; Installed = Reference; cb();",
        "let Installed; function helper() { new Installed(2); } function reenter() { helper(); } const cb = reenter; publish(cb); Installed = Reference;",
    ] { refuses(&epoch_fixture(body)); }
}

#[test]
fn ring_buffer_constructor_entry_capture_reentry_resource_and_producer_negatives() {
    accepts(&epoch_fixture("let Installed; for (let i = 0; i < 2; i++) { function helper() { new Installed(2); } Installed = Reference; publish(helper); }"));
    refuses(&epoch_fixture("let Installed; for (let i = 0; i < 2; i++) { function helper() { new Installed(2); } publish(helper); Installed = Reference; }"));
    accepts(&epoch_fixture(
        "let Installed; Installed = Reference; new Installed(2);",
    ));
    for body in [
        "let Installed; function install() { Installed = Reference; } install(); new Installed(2);",
        "let Installed; if (condition) { Installed = Reference; } else { Installed = Reference; } new Installed(2);",
        "let Installed; const early = Installed; Installed = Reference; new early(2);",
        "Installed = Reference; let Installed; new Installed(2);",
        "using Installed = Reference; let Copy; Copy = Installed; new Copy(2);",
        "let Installed; Installed = Reference; Installed.prototype.clear = foreign; new Installed(2);",
        "let Installed; Installed = Reference; Reflect.construct(Installed, []);",
    ] { refuses(&epoch_fixture(body)); }
    let source = epoch_fixture("let Installed; Installed = Reference; new Installed(2);");
    refuses(&format!("new Reference(2); {source}"));
    // Each mutation retains the other producer and constructor uses, so refusal
    // cannot be obtained by removing all candidate grammar or all consumers.
    let second = REFERENCE
        .replace("CAPACITY", INLINE_CAPACITY)
        .replace("Reference", "Second");
    for bad in [
        second.replace("Number(this.i)", "Number(this.length)"),
        second.replace("class Second", "@decorate class Second"),
        second.replace("put(value)", "@decorate put(value)"),
        second.replace("put(value)", "get put()"),
        second.replace("put(value)", "['put'](value)"),
        second.replace("class Second {", "class Second { field = eager();"),
        second.replace("class Second {", "class Second { static { eager(); }"),
    ] {
        refuses(&epoch_fixture(&format!("{bad}\nlet Installed; Installed = Reference; new Installed(2); Installed = Second; new Installed(3);")));
    }
}

const RING_BUFFER_PUBLIC_ENTRY_PUBLIC: &str = r#"var Installed;
function publicHelper() { new Installed(2); }
function bootstrap() {
  const copied = publicHelper;

class $THREAD_0_1 {
    constructor(value) { this.i = 0; this.length = 0; this.n = value === 0 || typeof value !== 'number' ? Infinity : value; this.arr = []; }
    put(value) {
        this.arr[this.i] = value;
        this.i = Number(this.i) + 1;
        if (this.length < this.n) { this.length = Number(this.length) + 1; }
        if (this.i >= this.n) { this.i = 0; }
    }
    toArray() {
        if (this.length < this.n) { return this.arr.slice(0, this.length); }
        else { return this.arr.slice(this.i, this.n).concat(this.arr.slice(0, this.i)); }
    }
    clear() { this.arr = []; this.length = 0; this.i = 0; }
}

Installed = $THREAD_0_1;
new Installed(3);
}
globalThis.publicHelper();
bootstrap();
"#;

fn ring_buffer_public_entry_controls() -> Vec<(&'static str, String, bool)> {
    let public = RING_BUFFER_PUBLIC_ENTRY_PUBLIC.to_string();
    let private = public
        .replace(
            "var Installed;\nfunction publicHelper() { new Installed(2); }\nfunction bootstrap() {",
            "function bootstrap() {\nlet Installed;\nfunction publicHelper() { new Installed(2); }",
        )
        .replace("globalThis.publicHelper();\n", "");
    vec![
        ("public-dead-alias", public.clone(), false),
        (
            "public-no-alias",
            public.replace("  const copied = publicHelper;\n", ""),
            false,
        ),
        (
            "public-alias-chain",
            public.replace(
                "const copied = publicHelper;",
                "const first = publicHelper; const copied = first;",
            ),
            false,
        ),
        (
            "public-export",
            public.replace("function publicHelper()", "export function publicHelper()"),
            false,
        ),
        (
            "public-export-alias",
            public.replace("bootstrap();", "bootstrap(); export { publicHelper };"),
            false,
        ),
        (
            "public-late-call",
            public.replace(
                "globalThis.publicHelper();\nbootstrap();",
                "bootstrap(); publicHelper();",
            ),
            false,
        ),
        ("private-inert", private.clone(), true),
        (
            "private-late-alias",
            private.replace("new Installed(3);", "new Installed(3); copied();"),
            true,
        ),
        (
            "private-late-chain",
            private
                .replace(
                    "const copied = publicHelper;",
                    "const first = publicHelper; const copied = first;",
                )
                .replace("new Installed(3);", "new Installed(3); copied();"),
            true,
        ),
        (
            "private-early-publication",
            private.replace(
                "const copied = publicHelper;",
                "const copied = publicHelper; globalThis.callback = copied;",
            ),
            false,
        ),
        (
            "private-early-call",
            private.replace(
                "const copied = publicHelper;",
                "const copied = publicHelper; copied();",
            ),
            false,
        ),
        (
            "private-reentry",
            private.replace(
                "const copied = publicHelper;",
                "function enter() { publicHelper(); } const copied = enter; publish(copied);",
            ),
            false,
        ),
        (
            "private-cycle",
            private.replace(
                "const copied = publicHelper;",
                "let first = publicHelper; const copied = first; first = copied;",
            ),
            false,
        ),
        (
            "private-copy-tdz",
            private.replace(
                "const copied = publicHelper;",
                "const copied = first; const first = publicHelper;",
            ),
            false,
        ),
        (
            "private-early-producer",
            private.replace(
                "Installed = $THREAD_0_1;",
                "Installed = $THREAD_0_1; Installed = Foreign;",
            ),
            false,
        ),
    ]
}

fn ring_buffer_public_entry_model(source: &str, expected: bool) {
    let a = Allocator::default();
    let parsed = Parser::new(&a, source, SourceType::unambiguous()).parse();
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(built.errors.is_empty(), "{:?}", built.errors);
    let (scoping, nodes) = built.semantic.into_scoping_and_nodes();
    let model = Model::new(
        source,
        &parsed.program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .expect("reachable real main and distinct program-scope helper");
    let mut complete = 0;
    for n in nodes.iter() {
        let AstKind::Class(c) = n.kind() else {
            continue;
        };
        if !methods(&scoping, c).is_some_and(|m| grammar().contains(&m)) {
            continue;
        }
        complete += 1;
        let symbol = c.id.as_ref().unwrap().symbol_id.get().unwrap();
        assert!(!constructor_uses(
            &model,
            symbol,
            c.span.end,
            &mut BTreeSet::new(),
            &mut false
        ));
        assert_eq!(epoch_constructor_uses(&model, symbol), expected);
    }
    assert_eq!(
        complete, 1,
        "control retains complete class grammar and constructor uses"
    );
    assert_eq!(claims(&model).len(), usize::from(expected));
    if source == RING_BUFFER_PUBLIC_ENTRY_PUBLIC {
        let helper = nodes.iter().find(|n| matches!(n.kind(), AstKind::Function(f) if f.id.as_ref().is_some_and(|id|id.name=="publicHelper"))).unwrap();
        assert!(!nodes.ancestor_ids(helper.id()).any(|p| matches!(
            nodes.kind(p),
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
        )));
        let alias = nodes.iter().find(|n| matches!(n.kind(), AstKind::VariableDeclarator(d) if matches!(&d.id, BindingPattern::BindingIdentifier(i) if i.name=="copied"))).unwrap();
        assert!(nodes
            .ancestor_ids(alias.id())
            .any(|p| matches!(nodes.kind(p), AstKind::Function(_))));
        let symbol = copy_destination(&model, alias.id()).unwrap();
        assert_eq!(scoping.get_resolved_references(symbol).count(), 0);
    }
}

#[test]
fn ring_buffer_public_entry_public_entry_pairs_have_relevant_models_and_production() {
    use crate::naming::name_threads;
    for (label, source, expected) in ring_buffer_public_entry_controls() {
        ring_buffer_public_entry_model(&source, expected);
        let named = name_threads(&source, true).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(named.report.class_outcomes.len(), 1, "{label}");
        let class = &named.report.class_outcomes[0];
        assert_eq!(class.applied, expected, "{label}: {class:?}");
        if expected {
            assert_eq!(class.evaluator.as_deref(), Some("owned_ring_buffer_class"));
            assert_eq!(class.proposed_name.as_deref(), Some("RingBuffer"));
            assert_eq!(class.reference_edits, 1);
        }
        let mut replay = source;
        for edit in named.report.edits.iter().rev() {
            replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
        }
        assert_eq!(replay, named.source, "{label} scoped UTF8 edit replay");
        eprintln!("producer ordering Model+production {label}: RingBuffer={expected}");
    }
}

#[test]
fn ring_buffer_compact_source_preserves_names_and_references() {
    use crate::naming::name_threads;
    // Exact accepted own factory body, with explicit opaque captured callable
    // ports. These ports' external behavior is not part of the factory claim.
    let mixed_factory_parent =
        include_str!("../../../tests/fixtures/program/observable-mixed-factories/mixed-factory-parent.js");
    let factory = &mixed_factory_parent[118549..123659];
    let compact_star = format!("function main() {{ function observerPort() {{}} function subscriptionPort() {{}} function teardownPort() {{}} function mapPort() {{}} function flatPort() {{}} const $KEY_107=observerPort; const $KEY_74=subscriptionPort; const $KEY_219=teardownPort; const $KEY_115=mapPort; const $KEY_198=flatPort; {factory} let Constructor=$THREAD_2_33189(); publish(Constructor); }} main();");
    let registry_parent = include_str!(
        "../../../tests/fixtures/program/wave-multi-consumer/reaching-registry-heldout.js"
    );
    let callback = &registry_parent[748809..749689];
    let compact_registry = format!("function main() {{ let $KEY_145=['opaque','alternate']; function log(e) {{ consume(e); }} let $KEY_11=log; function setup() {{ let $KEY_0=''; {callback} publish($THREAD_9_65801); }} setup(); }} main();");
    for (source, expected) in [
        (
            compact_star,
            vec![
                (
                    "$THREAD_2_33189",
                    "createObservableClass",
                    "shared_fast_observable_factory",
                ),
                (
                    "$THREAD_4_24972",
                    "convertToObservable",
                    "shared_fast_observable_installed_member",
                ),
                (
                    "$THREAD_8_86291",
                    "subscribeToObservable",
                    "shared_fast_observable_installed_member",
                ),
                (
                    "$THREAD_8_97257",
                    "pipeObservable",
                    "shared_fast_observable_installed_member",
                ),
                (
                    "$THREAD_7_11076",
                    "pipeObservableMapOperator",
                    "shared_fast_observable_installed_member",
                ),
                (
                    "$THREAD_2_77280",
                    "pipeObservableFlatMapOperator",
                    "shared_fast_observable_installed_member",
                ),
            ],
        ),
        (
            compact_registry,
            vec![(
                "$THREAD_9_65801",
                "captureRequestOpenMetadata",
                "network_request_open_metadata",
            )],
        ),
    ] {
        let named = name_threads(&source, true).unwrap();
        for (binding, name, evaluator) in expected {
            let outcomes: Vec<_> = named
                .report
                .outcomes
                .iter()
                .filter(|o| o.binding.name == binding)
                .collect();
            assert_eq!(outcomes.len(), 1, "{binding}");
            let o = outcomes[0];
            assert!(o.applied, "{binding}: {o:?}");
            assert_eq!(o.proposed_name.as_deref(), Some(name));
            assert_eq!(o.evaluator.as_deref(), Some(evaluator));
            assert_eq!(o.reference_edits, 1);
        }
        let mut replay = source;
        for e in named.report.edits.iter().rev() {
            replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
        }
        assert_eq!(replay, named.source);
    }
}
