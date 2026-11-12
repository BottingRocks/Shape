use super::*;
use crate::signals::schema::ProgramPropertiesLimits;

const OWNED_INTERVAL: &str = include_str!("../../../tests/fixtures/program/class-descriptors/captured-range-class.js");
const LOOP_INITIALIZER: &str = include_str!("../../../tests/fixtures/program/class-descriptors/observer-class.js");
const DEAD: &str = include_str!("../../../tests/fixtures/program/class-descriptors/dead-complete-class.js");

fn sites(source: &str) -> Vec<(&'static str, Span)> {
    let prefix = "function bootstrap() {\n";
    let wrapped = format!("{prefix}{source}\n}} bootstrap();");
    let source = wrapped.as_str();
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::default()).parse();
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let (scoping, nodes) = semantic.into_scoping_and_nodes();
    let model = Model::new(source, &parsed.program, &scoping, &nodes,
        ProgramPropertiesLimits::default()).expect("nonvacuous whole Model");
    claims(&model).into_iter().map(|claim| {
        let FinderClaim::ClassBinding { role, class_span, .. } = claim else {
            panic!("no function credit from class-local proof")
        };
        (role, Span::new(class_span.start - prefix.len() as u32, class_span.end - prefix.len() as u32))
    }).collect()
}

fn interval() -> &'static str {
    let start = 160322;
    &OWNED_INTERVAL[start..start + OWNED_INTERVAL[start..].find("\n\t\t\t}").unwrap()]
}

#[test]
fn actual_owned_interval_whole_model_and_exact_owned_interval() {
    assert_eq!(sites(OWNED_INTERVAL), [("subscription_class", Span::new(160322, 160814))]);
    assert_eq!(sites(interval()), [("subscription_class", Span::new(0, 492))]);
}

#[test]
fn actual_loop_initializer_for_init_let_whole_class_and_protocol() {
    assert_eq!(sites(LOOP_INITIALIZER), [("subscription_observer_class", Span::new(0, 1655))]);
    let aliases = format!("{LOOP_INITIALIZER}\nlet alias = $THREAD_7_110550; new alias({{}});");
    assert_eq!(sites(&aliases).len(), 1);
}

#[test]
fn descriptor_layout_alpha_and_equivalent_parentheses() {
    for (source, role) in [(interval(), "subscription_class"), (LOOP_INITIALIZER, "subscription_observer_class")] {
        let renamed = source.replace("$THREAD_5_2952", "LocalSubscription")
            .replace("$THREAD_7_110550", "LocalObserver")
            .replace("$THREAD_8_69198", "ownGetter").replace("$THREAD_0_110595", "ownGetter")
            .replace("$S", "descriptorList").replace("$T", "loopIndex").replace("$U", "propertyKey")
            .replace("$BP", "input").replace("$BQ", "observer");
        assert_eq!(sites(&renamed).iter().map(|s| s.0).collect::<Vec<_>>(), [role]);
        let equivalent = renamed.replace("configurable = true", "configurable = (true)");
        assert_eq!(sites(&equivalent).len(), 1);
    }
}

fn mutation(source: &str, old: &str, new: &str) {
    assert!(source.contains(old), "missing mutation {old}");
    let changed = source.replacen(old, new, 1);
    assert!(sites(&changed).is_empty(), "accepted mutation {old} -> {new}");
}

#[test]
fn terminal_increment_layout_refuses_effects_and_changed_semantics() {
    for (old, new) in [
        ("$THREAD_5_2952.prototype", "Foreign.prototype"),
        ("key: \"closed\"", "key: \"unsubscribe\""),
        ("$S.get = $THREAD_8_69198", "$S.get = foreign"),
        ("$S = [$S]", "$S = [foreign]"),
        ("configurable = true", "configurable = false"),
        ("enumerable || false", "enumerable || true"),
        ("writable = true", "writable = false"),
        ("let $T = 0", "let $T = 1"),
        ("for (;;)", "for (; $T < $S.length;)") ,
        ("++$T;", "--$T;"),
        ("++$T;", "$T++;"),
        ("++$T;", "++$T; sideEffect();"),
        ("$S[$T].configurable = true;", "++$T; $S[$T].configurable = true;"),
        ("$S.get = $THREAD_8_69198;", "$S.get = $THREAD_8_69198; sideEffect();"),
        ("$S = [$S];", "$S = [$S]; escape($S);"),
        ("let $T = 0;", "let $T = 0; escape($THREAD_8_69198);"),
        ("String($U)", "String($T)"),
        ("$S[$T]);", "foreign);"),
        ("return this._observer === void 0;", "return this._observer !== void 0;"),
        ("unsubscribe()", "get unsubscribe()"),
    ] { mutation(interval(), old, new); }
}

#[test]
fn for_init_layout_refuses_bounds_index_flags_getter_and_exposure_changes() {
    for (old, new) in [
        ("$THREAD_7_110550.prototype", "Foreign.prototype"),
        ("key: \"closed\"", "key: dynamicKey"),
        ("$S.get = $THREAD_0_110595", "$S.get = foreign"),
        ("this._subscription._observer === void 0", "this._subscription._observer !== void 0"),
        ("configurable = true", "configurable = false"),
        ("enumerable || false", "enumerable || true"),
        ("writable: false", "writable: true"),
        ("let j = 0", "let j = 1"),
        ("let j = 0", "var j = 0"),
        ("j < $S.length", "j <= $S.length"),
        ("j < $S.length", "j < foreign.length"),
        ("j++)", "++j)"),
        ("j++)", "j--)"),
        ("$S[j].key, $S[j]", "$S[j].key, $S[0]"),
        ("$S.get = $THREAD_0_110595;", "$S.get = $THREAD_0_110595; sideEffect();"),
        ("$S = [$S];", "$S = [$S]; escape($S);"),
        ("$S.get = $THREAD_0_110595;", "$S.get = $THREAD_0_110595; escape($THREAD_0_110595);"),
        ("next($BP)", "get next()"),
    ] { mutation(LOOP_INITIALIZER, old, new); }
}

#[test]
fn new_layouts_refuse_early_eager_uses_unproved_publications_and_aliases() {
    for (source, class) in [(interval(), "$THREAD_5_2952"), (LOOP_INITIALIZER, "$THREAD_7_110550")] {
        for changed in [
            format!("new {class}({{}}); {source}"),
            format!("let early = {class}; {source}"),
            format!("{source}\nlet alias = {class}; escape(alias);"),
            format!("let alias; {source}\nif (condition) alias = {class};"),
            format!("{source}\nlet alias = {class}; alias = foreign; new alias({{}});"),
            format!("{source}\nnew alias({{}}); let alias = {class};"),
            format!("{source}\nlet alias = {class}; let copy = alias; copy.prototype.x = foreign;"),
            format!("early(); {source}\nfunction early() {{ new {class}({{}}); }}"),
            format!("{source}\nif (condition) {{ var alias = {class}; }} new alias({{}});"),
        ] { assert!(sites(&changed).is_empty(), "accepted exposure: {changed}"); }
    }
}

#[test]
fn genuine_dead_complete_dead_complete_is_not_an_observer_lifecycle() {
    assert!(sites(DEAD).is_empty());
    let start = LOOP_INITIALIZER.find("\n\t\t\tcomplete() {").unwrap();
    let end = LOOP_INITIALIZER.find("\n\t\t\tisComplete()").unwrap();
    let changed = format!("{}\ncomplete() {{ this._subscription._observer; this._subscription._observer === void 0; }}{}",
        &LOOP_INITIALIZER[..start], &LOOP_INITIALIZER[end..]);
    assert!(sites(&changed).is_empty());
}
