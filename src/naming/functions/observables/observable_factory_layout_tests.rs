//! Complete saved-program witness for an ordinary-constructor layout.
//! An ordinary constructor never supplies a native class claim.
use super::*;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/program/observable-setup-order/ordinary-parent.js"
));

fn inspect(source: &str, check: impl FnOnce(&Model<'_, '_>, FunctionSite, &Function<'_>)) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = crate::analysis::parse(&allocator, source);
    assert!(parsed.errors.is_empty());
    let built = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(built.errors.is_empty());
    let model = Model::new(
        source,
        &parsed.program,
        built.semantic.scoping(),
        built.semantic.nodes(),
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    let site = model.all_sites().iter().copied().find(|site| {
        let AstKind::Function(f) = model.nodes.kind(site.node_id) else { return false; };
        f.id.as_ref().is_some_and(|id| id.name == "$THREAD_2_101607")
    }).expect("saved ordinary-constructor factory");
    let AstKind::Function(factory) = model.nodes.kind(site.node_id) else { unreachable!() };
    check(&model, site, factory);
}

#[test]
fn actual_saved_factory_first_guard_is_class_first_layout() {
    inspect(SOURCE, |model, site, factory| {
        assert_eq!(site.span, Span::new(160636, 165798));
        let body = body_of(factory).unwrap();
        let [Statement::FunctionDeclaration(conversion), Statement::FunctionDeclaration(constructor), alias, ..] = body.statements.as_slice() else {
            panic!("saved whole-body layout changed");
        };
        assert_eq!(plain_params(model, conversion).unwrap().len(), 2);
        assert_eq!(constructor.body.as_ref().unwrap().directives.len(), 1);
        assert_eq!(constructor.body.as_ref().unwrap().directives[0].directive, "use strict");
        assert!(constructor_method(model, constructor).is_some());
        let class = constructor.id.as_ref().unwrap().symbol_id.get().unwrap();
        let alias = one_variable(alias).unwrap();
        assert_eq!(identifier(model, alias.init.as_ref().unwrap()), Some(class));
        for extended in [false, true] {
            for lowerings in [false, true] {
                assert!(owned_factory(model, site, factory, extended, lowerings).is_none());
            }
            assert!(installed_factory(model, site, factory, extended).is_none());
        }
        assert!(proven_native_class(model, site).is_none());
        assert!(ordinary_factory(model, site, factory).is_some());
        eprintln!("ordinary constructor: owned_factory and installed_factory fail their first ClassDeclaration layout match; separate ordinary-constructor API proof succeeds without a native class claim");
    });
}

struct Captures<'m, 's, 'a> {
    model: &'m Model<'s, 'a>,
    site: FunctionSite,
    symbols: BTreeSet<Symbol>,
}
impl<'a> Visit<'a> for Captures<'_, '_, 'a> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if let Some(s) = crate::analysis::identifier_symbol(id, self.model.scoping) {
            if captured(self.model, self.site, s) && callable_declaration(self.model, s, 8) {
                self.symbols.insert(s);
            }
        }
    }
}

#[test]
fn actual_saved_captures_pass_readiness_at_the_real_factory_call() {
    inspect(SOURCE, |model, site, factory| {
        let body = body_of(factory).unwrap();
        let mut captures = Captures { model, site, symbols: BTreeSet::new() };
        captures.visit_function_body(body);
        assert_eq!(captures.symbols.len(), 5);
        let uses: Vec<_> = model.scoping.get_resolved_references(site.binding_symbol.unwrap())
            .map(|reference| reference.node_id()).collect();
        assert_eq!(uses.len(), 1);
        assert!(matches!(model.nodes.kind(model.nodes.parent_id(uses[0])), AstKind::CallExpression(_)));
        for symbol in captures.symbols {
            assert!(immutable(model, symbol));
            let declaration = model.scoping.symbol_declaration(symbol);
            let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
                panic!("expected captured initializer");
            };
            assert!(variable.init.is_some());
            let lexical_body = model.nodes.parent_id(model.nodes.parent_id(declaration));
            assert!(matches!(model.nodes.kind(lexical_body), AstKind::FunctionBody(_)));
            for use_node in &uses {
                assert!(ordered_captured_origin(model, symbol, *use_node, 8));
            }
            eprintln!("capture {:?} at {:?}: immutable initialized alias; ordered_captured_origin accepts every actual factory exposure", symbol, variable.span);
        }
        let mut origins = CapturedOrigins { model, site, uses, valid: true };
        origins.visit_function_body(body);
        assert!(origins.valid);
    });
}

#[test]
fn actual_tuple_call_preserves_receiver_and_detects_changed_call_receiver() {
    // Exercise the existing tuple proof independently of the blocked parent.
    // This does not turn a child proof into a native Observable claim.
    for (source, valid) in [
        (SOURCE.to_string(), true),
        (SOURCE.replace("$BR", "renamedStartTuple"), true),
        (SOURCE.replace("$BR[1].call($BR[0], $BP, $BQ)", "$BR[1].call($BR[1], $BP, $BQ)"), false),
    ] {
        inspect(&source, |model, _, factory| {
            let member = factory.body.as_ref().unwrap().statements.iter().find_map(|statement| {
                let Statement::FunctionDeclaration(function) = statement else { return None; };
                function.id.as_ref().is_some_and(|id| id.name == "$THREAD_3_101729").then_some(function)
            }).unwrap();
            let Statement::IfStatement(branch) = member.body.as_ref().unwrap().statements.last().unwrap() else { panic!() };
            let accepted = block(branch.alternate.as_ref().unwrap()).unwrap();
            let Statement::IfStatement(start) = &accepted[1] else { panic!() };
            let [bundle, call] = block(&start.consequent).unwrap() else { panic!() };
            let result = bundled_call(model, bundle, call);
            assert_eq!(result.is_some(), valid);
            if let Some((_, method, arguments)) = result {
                assert_eq!(method, "start");
                assert_eq!(arguments.len(), 1);
            }
        });
    }
}

fn own_change(from: &str, to: &str) -> String {
    let own = &SOURCE[160636..165798];
    assert!(own.contains(from), "missing mutation {from}");
    format!("{}{}{}", &SOURCE[..160636], own.replace(from, to), &SOURCE[165798..])
}

#[test]
fn ordinary_api_alpha_equivalents_and_complete_contract_refusals() {
    for source in [
        SOURCE.to_string(),
        SOURCE.replace("$KEY_379", "observerConstructorCapture")
            .replace("$KEY_436", "wrapperConstructorCapture")
            .replace("$KEY_62", "mapOperatorCapture")
            .replace("$KEY_38", "flatMapOperatorCapture")
            .replace("$BR", "startMethodTuple"),
    ] {
        inspect(&source, |model, site, factory| {
            let names = ordinary_factory(model, site, factory).expect("complete own API");
            assert_eq!(names.len(), 8);
            assert_eq!(ordinary_name(model, site, factory), Some("createSubscriberStreamConstructor"));
            assert!(proven_native_class(model, site).is_none());
        });
    }
    let mut mutations = Vec::new();
    for (from, to) in [
        ("$KEY_1.prototype.subscribe", "$KEY_1.otherPrototype.subscribe"),
        ("$KEY_1.from = $THREAD_0_148109;", "$KEY_1.from = $THREAD_0_148109; $KEY_1.from = $THREAD_0_148109;"),
        ("$KEY_1.prototype.pipe = $THREAD_5_71209;", ""),
        ("typeof $BM !== \"function\"", "typeof $BM === \"function\""),
        ("this._subscriber = $BM;", "this._subscriber = other;"),
        ("\"use strict\";", ""),
        ("$BR[1].call($BR[0], $BP, $BQ)", "$BR[1].call($BR[0], $BQ, $BP)"),
        ("$BS[1].call($BS[0], e)", "$BS[1].call($BS[1], e)"),
        ("$KEY_1.prototype.map =", "$KEY_1.prototype.unknownMethod ="),
        ("$KEY_1.prototype.map = $THREAD_7_22989;", "$KEY_1.prototype.unorderedFlatMap = $THREAD_7_22989;"),
        ("return this.pipe($KEY_38($BM));", "return this.pipe($KEY_62($BM));"),
        ("return $KEY_1;", "publish($KEY_1); return $KEY_1;"),
        (".then($BP =>", ".then(async $BP =>"),
    ] {
        mutations.push(own_change(from, to));
    }
    // Every real exposure must follow capture initialization, including a
    // publication that can invoke the factory synchronously and an alias read.
    for prefix in ["$THREAD_2_101607(); ", "register($THREAD_2_101607); ", "let earlyFactory = $THREAD_2_101607; earlyFactory(); "] {
        mutations.push(SOURCE.replacen("let $KEY_379 = $THREAD_7_143368;", &format!("{prefix}let $KEY_379 = $THREAD_7_143368;"), 1));
    }
    mutations.push(SOURCE.replacen("let $KEY_379 = $THREAD_7_143368;", "let $KEY_379 = $THREAD_7_143368; $KEY_379 = other;", 1));
    for (index, source) in mutations.iter().enumerate() {
        inspect(source, |model, site, factory| {
            assert!(ordinary_factory(model, site, factory).is_none(), "accepted mutation {index}");
            assert!(ordinary_name(model, site, factory).is_none(), "named incomplete parent {index}");
        });
    }
}
