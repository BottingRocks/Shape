//! Structural recognition for [`super::ProgramProperties`].
//!
//! The scanner does not look for generated key/thread spellings, signal ids,
//! wave counts, or capture-specific literals. It asks scope-resolved, structural
//! questions:
//!
//! * Which reachable function owns the broadest direct binding set (the main)?
//! * Which array's object elements carry a constructor/priority/name shape and
//!   receive later numeric-index population writes?
//! * Which parallel function/index-row/boolean-selector arrays are consumed by
//!   one function body through the same registry?
//! * Which functions shape a number writer, a string-writer factory, a seeded
//!   mask closure, a 32-bit token hash, an observer factory, a snapshot getter
//!   and an envelope builder?
//!
//! Parameter positions are asked through [`Model::parameter_slot`], so a
//! helper written with `arguments[N]` (target traces) and one written with
//! named formals (research captures) are recognized identically, and a slot
//! always names the function that owns it. Candidates are never told apart by
//! size: a tie that consumers and dataflow cannot break stays ambiguous.
//!
//! All identities returned by this module are resolved symbols and exact spans.
//! Names are only labels in the evidence.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};

use super::bindings::{
    array_elements, binding_pattern_symbol, call_arguments, computed_member_name, is_boolean_array,
    is_integer_array, literal_value, new_arguments, numeric_index, numeric_value, object_members,
    object_members_of, property_key_name, proven_function_at_read, proven_function_node,
    static_member_name, FunctionSite,
    Model,
};
use super::reaching::expression_value_at_use;
use crate::signals::schema::{
    BindingRecord, BindingRef, BindingRole, CaptureKind, Confidence, ConstantTable, ContextField,
    CoverageSummary, DescriptorPopulation, DescriptorRecord, DescriptorRegistry, Diagnostic,
    DirectSlot, EmissionKind, EmissionRecord, EnvelopeProperties, ExprRecord, ExternalInput,
    ExternalInputKind, HashChain, HelperRecord, LiteralValue, ObserverRoute, ObserverSlot,
    PopulationAssignment, ProgramProperties, ProgramPropertiesError, ProgramPropertiesLimits,
    ProgramScope, SerializerProperties, SessionArgumentMapping, SessionProperties, SignalRecord,
    SourceIdentity, SourceSpan, StaticCapture, StoreWiring, UnresolvedFact, WaveRecord,
    WaveRegistry, WireLayoutStatus, PROGRAM_PROPERTIES_SCHEMA_VERSION,
};

// Registry identity is independent of the per-emitter routing proof below.
#[path = "grouped_registry.rs"]
pub(crate) mod grouped_registry;

/// Extract generator-input properties from exact program source.
pub fn extract_program_properties(
    source: &str,
) -> Result<ProgramProperties, ProgramPropertiesError> {
    extract_program_properties_with_limits(source, ProgramPropertiesLimits::default())
}

/// Extract generator-input properties with explicit deterministic budgets.
pub fn extract_program_properties_with_limits(
    source: &str,
    limits: ProgramPropertiesLimits,
) -> Result<ProgramProperties, ProgramPropertiesError> {
    if source.trim().is_empty() {
        return Err(ProgramPropertiesError::InvalidInput(
            "source is empty".to_string(),
        ));
    }
    if source.len() > limits.max_source_bytes {
        return Err(ProgramPropertiesError::Limit {
            what: "source bytes",
            limit: limits.max_source_bytes,
        });
    }

    let allocator = Allocator::default();
    let parsed = crate::analysis::parse(&allocator, source);
    if let Some(error) = parsed.errors.first() {
        return Err(ProgramPropertiesError::Parse(error.clone()));
    }
    if parsed.panicked {
        return Err(ProgramPropertiesError::Parse(
            "Oxc parser panicked on the source".to_string(),
        ));
    }
    let (scoping, nodes) = crate::analysis::analyze(&parsed.program);
    let model = Model::new(source, &parsed.program, &scoping, &nodes, limits)?;
    Extractor::new(&model, limits).extract()
}

/// [`extract_program_properties_with_limits`] on an existing model of the
/// same source and limits.
pub(crate) fn properties_from_model(
    model: &Model<'_, '_>,
    limits: ProgramPropertiesLimits,
) -> Result<ProgramProperties, ProgramPropertiesError> {
    Extractor::new(model, limits).extract()
}

/// The same registry resolver used by analysis, without running codec analysis.
pub(crate) fn descriptor_registry_from_model(
    model: &Model<'_, '_>,
) -> Result<Option<DescriptorRegistry>, ProgramPropertiesError> {
    let mut extractor = Extractor::new(model, ProgramPropertiesLimits::default());
    extractor.strict_registry = true;
    extractor.detect_descriptors()
}

struct Extractor<'m, 's, 'a> {
    strict_registry: bool,
    model: &'m Model<'s, 'a>,
    limits: ProgramPropertiesLimits,
    retained_bytes: usize,
    /// [`Self::detect_prng_factories`], computed once (it reads only the model).
    prng_factories: std::cell::OnceCell<Vec<FunctionSite>>,
    roles: HashMap<u32, BindingRole>,
    unresolved: Vec<UnresolvedFact>,
    diagnostics: Vec<Diagnostic>,
}

/// Registry and row recognition only; deliberately excludes codecs, helpers,
/// session reconstruction and per-emission grammars.
pub(crate) fn collector_properties(
    model: &Model<'_, '_>,
    limits: ProgramPropertiesLimits,
) -> Result<ProgramProperties, ProgramPropertiesError> {
    let mut extractor = Extractor::new(model, limits);
    let descriptors = extractor.detect_descriptors()?;
    let waves = extractor.detect_waves(descriptors.as_ref())?;
    let host_probes = super::host_probes::detect(model, waves.as_ref());
    Ok(ProgramProperties {
        schema_version: PROGRAM_PROPERTIES_SCHEMA_VERSION,
        source: SourceIdentity {
            sha256: sha256_hex(model.source),
            bytes: model.source.len(),
        },
        program_scope: ProgramScope {
            main: model.function_ref(model.main_site()),
            main_argument: None,
            owned_binding_count: model.main_symbols.len(),
            returns_init_object: false,
            top_level_call: None,
            host_event_bridge: None,
        },
        descriptors,
        waves,
        session: None,
        bindings: Vec::new(),
        helpers: Vec::new(),
        signals: Vec::new(),
        host_probes,
        constants: Vec::new(),
        static_captures: Vec::new(),
        external_inputs: Vec::new(),
        unresolved: extractor.unresolved,
        diagnostics: extractor.diagnostics,
        coverage: CoverageSummary::default(),
    })
}

impl<'m, 's, 'a> Extractor<'m, 's, 'a> {
    fn new(model: &'m Model<'s, 'a>, limits: ProgramPropertiesLimits) -> Self {
        Self {
            strict_registry: false,
            model,
            limits,
            retained_bytes: 0,
            prng_factories: std::cell::OnceCell::new(),
            roles: HashMap::new(),
            unresolved: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn extract(mut self) -> Result<ProgramProperties, ProgramPropertiesError> {
        let main_site = self.model.main_site();
        let main_call = self.model.calls_to(main_site).into_iter().find(|call| {
            self.model
                .enclosing_function(call.node_id.get())
                .map(|enclosing| enclosing.span.start)
                != Some(main_site.span.start)
        });
        let main_argument = main_call
            .and_then(|call| call_arguments(call).first().copied())
            .and_then(|arg| match arg {
                Expression::Identifier(identifier) => Some(identifier.name.to_string()),
                _ => None,
            });
        let t = crate::signals::timing::span("    s.main+bridge");
        let host_event_bridge = self.host_event_bridge();
        let returns_init_object = self.returned_property_function(main_site, "init").is_some();

        self.assign_role_for_site(main_site, BindingRole::MainEntry);
        drop(t);
        let t = crate::signals::timing::span("    s.descriptors");

        let descriptors = self.detect_descriptors()?;
        drop(t);
        let t = crate::signals::timing::span("    s.waves");
        let waves = self.detect_waves(descriptors.as_ref())?;
        drop(t);
        let t = crate::signals::timing::span("    s.session");
        let session = self.detect_session(descriptors.as_ref(), waves.as_ref())?;

        drop(t);
        let t = crate::signals::timing::span("    s.helpers");
        let helper_specs =
            self.detect_helpers(descriptors.as_ref(), waves.as_ref(), session.as_ref());
        let mut helpers = Vec::new();
        for (role, site, confidence, notes) in helper_specs {
            self.assign_role_for_site(site, role);
            let mut function = self.model.function_ref(site);
            let body_source = self.retain_function_body(site, role);
            function.source = body_source.clone();
            helpers.push(HelperRecord {
                role,
                function,
                body_source,
                confidence,
                notes,
            });
        }

        drop(t);
        let t = crate::signals::timing::span("    s.envelope+serializer");
        // Shared serializer/envelope facts need the session and helper roles.
        let envelope_site = self.detect_envelope_builder();
        if let Some(site) = envelope_site {
            self.assign_role_for_site(site, BindingRole::EnvelopeBuilder);
        }
        let default_encoder = envelope_site.and_then(|site| self.default_encoder_symbol(site));
        if let Some(symbol) = default_encoder {
            self.roles
                .insert(symbol.index() as u32, BindingRole::DefaultEncoder);
        }
        let serializer = self.build_serializer(
            envelope_site,
            default_encoder,
            session.as_ref(),
            descriptors.as_ref(),
        )?;

        let envelope = envelope_site.map(|site| self.build_envelope(site));
        let session = session.map(|mut session| {
            session.envelope = envelope;
            session.serializer = serializer;
            session
        });

        if let Some(site) = envelope_site {
            if !helpers
                .iter()
                .any(|helper| helper.role == BindingRole::EnvelopeBuilder)
            {
                self.push_helper(
                    &mut helpers,
                    BindingRole::EnvelopeBuilder,
                    site,
                    Confidence::High,
                    vec![
                        "field-by-field envelope with f/b/c/d/z/a and the default encode path"
                            .to_string(),
                    ],
                );
            }
        }
        if let Some(factory) = session
            .as_ref()
            .and_then(|session| session.serializer.as_ref())
            .and_then(|serializer| serializer.factory.as_ref())
        {
            if let Some(site) = self
                .model
                .site_by_span(Span::new(factory.span.start, factory.span.end))
            {
                if !helpers
                    .iter()
                    .any(|helper| helper.role == BindingRole::SerializerFactory)
                {
                    self.push_helper(
                        &mut helpers,
                        BindingRole::SerializerFactory,
                        site,
                        Confidence::Medium,
                        vec!["encoder factory wired from the session arguments".to_string()],
                    );
                }
            }
        }
        if let Some(error_buffer) = session
            .as_ref()
            .and_then(|session| session.stores.error_buffer.as_ref())
        {
            if let Some(site) = self
                .model
                .symbol_by_index(error_buffer.symbol)
                .and_then(|symbol| self.error_ring_constructor(symbol))
            {
                self.push_helper(
                    &mut helpers,
                    BindingRole::ErrorRingConstructor,
                    site,
                    Confidence::Medium,
                    vec![
                        "capacity-normalising ring buffer used by the snapshot errors field"
                            .to_string(),
                    ],
                );
            }
        }

        drop(t);
        let t = crate::signals::timing::span("    s.probes");
        let host_probes = super::host_probes::detect(self.model, waves.as_ref());
        drop(t);
        let t = crate::signals::timing::span("    s.build_signals");
        let mut signals = self.build_signals(descriptors.as_ref(), waves.as_ref())?;
        drop(t);
        let t = crate::signals::timing::span("    s.layouts+constants");
        let static_captures =
            self.apply_signal_layouts(&helpers, &mut signals, waves.as_ref(), session.as_ref());
        let constants = self.constant_tables();

        if let Some(session) = session.as_ref() {
            self.assign_role_for_symbol_opt(
                session.constructor.binding.as_ref(),
                BindingRole::SessionConstructor,
            );
            if let Some(context) = session.context_binding.as_ref() {
                self.roles
                    .insert(context.symbol, BindingRole::ContextConfig);
            }
            if let Some(init) = session.init.as_ref() {
                if let Some(binding) = init.binding.as_ref() {
                    self.roles.insert(binding.symbol, BindingRole::SessionInit);
                }
            }
            if let Some(teardown) = session.teardown.as_ref() {
                if let Some(binding) = teardown.binding.as_ref() {
                    self.roles
                        .insert(binding.symbol, BindingRole::SessionTeardown);
                }
            }
        }

        drop(t);
        let _t = crate::signals::timing::span("    s.rest");
        let external_inputs = self.external_inputs(session.as_ref());
        let mut bindings = Vec::new();
        for symbol in &self.model.main_symbols {
            let role = self
                .roles
                .get(&(symbol.index() as u32))
                .copied()
                .unwrap_or(BindingRole::Unknown);
            let initializer = self.model.symbol_value(*symbol).map(|expr| {
                let mut record = self.model.expr_record(expr);
                if let Some(function) = record.function.clone() {
                    record.function = Some(function);
                }
                record
            });
            bindings.push(BindingRecord {
                id: symbol.index() as u32,
                name: self.model.scoping().symbol_name(*symbol).to_string(),
                declaration: SourceSpan::from(
                    self.model
                        .nodes()
                        .get_node(self.model.scoping().symbol_declaration(*symbol))
                        .span(),
                ),
                identifier: SourceSpan::from(self.model.scoping().symbol_span(*symbol)),
                kind: self.model.binding_kind(*symbol),
                role,
                references: self.model.binding_references(*symbol),
                writes: self.model.binding_writes(*symbol),
                initializer,
            });
        }

        let recognized_binding_roles = bindings
            .iter()
            .filter(|binding| !binding.role.is_unknown())
            .count();
        let descriptor_count = descriptors.as_ref().map_or(0, |registry| registry.length);
        let wave_count = waves.as_ref().map_or(0, |registry| registry.waves.len());
        let direct_slot_count = waves
            .as_ref()
            .map_or(0, |registry| registry.direct_slots.len());
        let emission_count = signals.iter().map(|signal| signal.emissions.len()).sum();

        self.finalize_unresolved(
            descriptors.as_ref(),
            waves.as_ref(),
            session.as_ref(),
            &helpers,
        );
        let mut coverage = CoverageSummary {
            binding_count: bindings.len(),
            recognized_binding_roles,
            descriptor_count,
            wave_count,
            direct_slot_count,
            signal_count: signals.len(),
            emission_count,
            unresolved_count: self.unresolved.len(),
        };
        coverage.unresolved_count = self.unresolved.len();

        Ok(ProgramProperties {
            schema_version: PROGRAM_PROPERTIES_SCHEMA_VERSION,
            source: SourceIdentity {
                sha256: sha256_hex(self.model.source),
                bytes: self.model.source.len(),
            },
            program_scope: ProgramScope {
                main: self.model.function_ref(main_site),
                main_argument,
                owned_binding_count: self.model.main_symbols.len(),
                returns_init_object,
                top_level_call: main_call.map(|call| SourceSpan::from(call.span)),
                host_event_bridge,
            },
            bindings,
            helpers,
            descriptors,
            waves,
            session,
            signals,
            host_probes,
            constants,
            static_captures,
            external_inputs,
            unresolved: self.unresolved,
            diagnostics: self.diagnostics,
            coverage,
        })
    }

    // -----------------------------------------------------------------------
    // Scope/role bookkeeping
    // -----------------------------------------------------------------------

    fn push_helper(
        &mut self,
        helpers: &mut Vec<HelperRecord>,
        role: BindingRole,
        site: FunctionSite,
        confidence: Confidence,
        notes: Vec<String>,
    ) {
        self.assign_role_for_site(site, role);
        let mut function = self.model.function_ref(site);
        let body_source = self.retain_function_body(site, role);
        function.source = body_source.clone();
        helpers.push(HelperRecord {
            role,
            function,
            body_source,
            confidence,
            notes,
        });
    }

    fn assign_role_for_site(&mut self, site: FunctionSite, role: BindingRole) {
        if let Some(symbol) = site.binding_symbol {
            self.roles.insert(symbol.index() as u32, role);
            if let Some(alias) = self.public_alias_of(symbol) {
                self.roles.insert(alias.index() as u32, role);
            }
        }
    }

    fn assign_role_for_symbol_opt(&mut self, binding: Option<&BindingRef>, role: BindingRole) {
        if let Some(binding) = binding {
            self.roles.insert(binding.symbol, role);
        }
    }

    fn retain_function_body(&mut self, site: FunctionSite, _role: BindingRole) -> Option<String> {
        let body = site.body_span?;
        let length = (body.end - body.start) as usize;
        if self.retained_bytes + length > self.limits.max_retained_source_bytes {
            self.unresolved.push(UnresolvedFact {
                fact: "helper_body_retained".to_string(),
                reason: format!(
                    "retained helper source budget {} bytes would be exceeded",
                    self.limits.max_retained_source_bytes
                ),
                span: Some(SourceSpan::from(body)),
                required: false,
            });
            return None;
        }
        self.retained_bytes += length;
        Some(self.model.exact(body).to_string())
    }

    fn host_event_bridge(&self) -> Option<SourceSpan> {
        self.model
            .nodes()
            .iter()
            .find_map(|node| match node.kind() {
                AstKind::CallExpression(call)
                    if static_member_name(&call.callee) == Some("initCustomEvent") =>
                {
                    Some(SourceSpan::from(call.span))
                }
                _ => None,
            })
    }

    /// The values `site` itself returns: the arguments of its own `return`
    /// statements (not those of a nested function or arrow) and, for an
    /// arrow with a concise body, the body expression.
    fn returned_values(&self, site: FunctionSite) -> Vec<&'a Expression<'a>> {
        let mut values = Vec::new();
        if let AstKind::ArrowFunctionExpression(arrow) = self.model.nodes().kind(site.node_id) {
            values.extend(arrow.get_expression());
        }
        for node in self.model.owned_returns(site) {
            let AstKind::ReturnStatement(ret) = node.kind() else {
                continue;
            };
            if !self.model.node_within(node.id(), site) || !self.owns(site, node.id()) {
                continue;
            }
            values.extend(ret.argument.as_ref());
        }
        values
    }

    /// True when `site` is the innermost function or arrow holding `node`.
    fn owns(&self, site: FunctionSite, node: oxc_semantic::NodeId) -> bool {
        self.model
            .owning_site(node)
            .is_some_and(|owner| owner.node_id == site.node_id)
    }

    fn returned_property_function(
        &self,
        site: FunctionSite,
        property: &str,
    ) -> Option<FunctionSite> {
        for argument in self.returned_values(site) {
            let Some(members) = object_members(argument) else {
                continue;
            };
            let Some(member) = members.iter().find(|member| member.name == property) else {
                continue;
            };
            if let Some(target) = self.model.function_from_expression(member.value) {
                return Some(target);
            }
        }
        None
    }

    fn returns_object_with_keys(
        &self,
        site: FunctionSite,
        required: &[&str],
    ) -> Vec<BTreeSet<String>> {
        let mut result = Vec::new();
        for argument in self.returned_values(site) {
            let Some(expression) = self.resolve_object_expression(argument, 0) else {
                continue;
            };
            let Some(members) = object_members(expression) else {
                continue;
            };
            let keys: BTreeSet<String> = members.iter().map(|member| member.name.clone()).collect();
            if required.iter().all(|key| keys.contains(*key)) {
                result.push(keys);
            }
        }
        result
    }

    /// Keys assigned on an object binding inside `site`, including the keys of
    /// its object-literal initializer. Used for the observer factory, whose
    /// `next`/`error`/`complete` methods are installed incrementally.
    fn object_binding_keys(&self, symbol: SymbolId, site: FunctionSite) -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        if let Some(value) = self.model.symbol_value(symbol) {
            if let Some(expression) = self.resolve_object_expression(value, 0) {
                if let Some(members) = object_members(expression) {
                    keys.extend(members.into_iter().map(|member| member.name));
                }
            }
        }
        for node in self.model.nodes_within(site).into_iter() {
            if !self.model.node_within(node.id(), site) {
                continue;
            }
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let key = match &assignment.left {
                AssignmentTarget::ComputedMemberExpression(member) => computed_member_name(member),
                AssignmentTarget::StaticMemberExpression(member) => {
                    Some(member.property.name.to_string())
                }
                _ => None,
            };
            let Some(key) = key else {
                continue;
            };
            let object = match &assignment.left {
                AssignmentTarget::ComputedMemberExpression(member) => {
                    crate::analysis::expression_symbol(&member.object, self.model.scoping())
                }
                AssignmentTarget::StaticMemberExpression(member) => {
                    crate::analysis::expression_symbol(&member.object, self.model.scoping())
                }
                _ => None,
            };
            if object == Some(symbol) {
                keys.insert(key);
            }
        }
        keys
    }

    /// A main binding that aliases a local registry array (`public = local`).
    fn public_alias_of(&self, symbol: SymbolId) -> Option<SymbolId> {
        let mut candidates = Vec::new();
        for node in self.model.kind_nodes(oxc_ast::AstType::AssignmentExpression) {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                continue;
            };
            let Some(target) = crate::analysis::identifier_symbol(identifier, self.model.scoping())
            else {
                continue;
            };
            if target == symbol || !self.model.main_symbols.contains(&target) {
                continue;
            }
            if crate::analysis::expression_symbol(&assignment.right, self.model.scoping())
                == Some(symbol)
            {
                candidates.push(target);
            }
        }
        candidates.sort_by_key(|candidate| candidate.index());
        candidates.first().copied()
    }

    fn body_contains_identifier(&self, site: FunctionSite, name: &str) -> bool {
        self.model.own_nodes(site).any(|node| {
            self.model.node_within(node.id(), site)
                && self
                    .model
                    .enclosing_function(node.id())
                    .map(|f| f.span.start)
                    == Some(site.span.start)
                && matches!(node.kind(), AstKind::IdentifierReference(id) if id.name == name)
        })
    }

    /// True when the site's own body names the property `key`: as a string
    /// literal (`o["key"]`), a static member (`o.key`) or an identifier
    /// object key (`{ key: v }`). Both capture representations spell keys.
    fn body_names_key(&self, site: FunctionSite, key: &str) -> bool {
        self.model.own_nodes(site).any(|node| {
            self.model.node_within(node.id(), site)
                && self
                    .model
                    .enclosing_function(node.id())
                    .map(|f| f.span.start)
                    == Some(site.span.start)
                && match node.kind() {
                    AstKind::StringLiteral(literal) => literal.value == key,
                    AstKind::StaticMemberExpression(member) => member.property.name == key,
                    AstKind::ObjectProperty(property) => matches!(
                        &property.key,
                        PropertyKey::StaticIdentifier(identifier) if identifier.name == key
                    ),
                    _ => false,
                }
        })
    }

    fn body_contains_shift(
        &self,
        site: FunctionSite,
        operator: oxc_syntax::operator::BinaryOperator,
        amount: f64,
    ) -> bool {
        self.model.own_nodes(site).any(|node| {
            let AstKind::BinaryExpression(binary) = node.kind() else {
                return false;
            };
            self.model.node_within(node.id(), site)
                && self
                    .model
                    .enclosing_function(node.id())
                    .map(|f| f.span.start)
                    == Some(site.span.start)
                && binary.operator == operator
                && numeric_value(&binary.right).is_some_and(|value| (value - amount).abs() < 1e-9)
        })
    }

    // -----------------------------------------------------------------------
    // Descriptor registry
    // -----------------------------------------------------------------------

    fn detect_descriptors(&mut self) -> Result<Option<DescriptorRegistry>, ProgramPropertiesError> {
        let mut candidates: Vec<(SymbolId, SymbolId, Span, Vec<&Expression<'a>>)> = Vec::new();
        for symbol in &self.model.main_symbols {
            let Some(mut initializer) = self.model.symbol_value(*symbol) else {
                continue;
            };
            let mut origin_symbol = *symbol;
            // A stable registry may capture a scratch local at one read:
            // scratch = [{ctor: task}]; registry = scratch; scratch = other.
            // Resolve at that read, not from scratch's last program-wide value.
            for _ in 0..8 {
                let Ok(definition) = expression_value_at_use(
                    self.model.scoping(), self.model.nodes(), initializer,
                ) else { break };
                if definition.value.span() == initializer.span() { break; }
                if let Some(owner) = crate::analysis::expression_symbol(initializer, self.model.scoping()) {
                    origin_symbol = owner;
                }
                initializer = definition.value;
            }
            let Some(elements) = array_elements(initializer) else {
                continue;
            };
            if elements.len() > self.limits.max_array_elements {
                return Err(ProgramPropertiesError::Limit {
                    what: "descriptor array elements",
                    limit: self.limits.max_array_elements,
                });
            }
            let shaped = elements.iter().any(|element| {
                object_members(element)
                    .is_some_and(|members| members.iter().any(|member| member.name == "ctor"))
            });
            if !shaped {
                continue;
            }
            candidates.push((*symbol, origin_symbol, initializer.span(), elements));
        }
        // Several proven holders of one allocation are one registry, not
        // competing registries. Preserve ambiguity between different arrays.
        let mut allocations = BTreeSet::new();
        candidates.retain(|candidate| allocations.insert((candidate.2.start, candidate.2.end)));
        if candidates.is_empty() {
            return Ok(None);
        }
        // The registry is told apart from other `{ctor}` arrays by its
        // consumer, never by its length: it is iterated by a callback that
        // reads `ctor` from its own element parameter. A lone shaped array
        // needs no discriminator; anything else that dataflow cannot decide
        // stays ambiguous.
        let consumed: Vec<_> = candidates
            .iter()
            .filter(|candidate| self.has_ctor_consumer(candidate.0))
            .collect();
        let top: Vec<_> = if consumed.is_empty() {
            candidates.iter().collect()
        } else {
            consumed
        };
        if top.len() > 1 {
            return Err(ProgramPropertiesError::Ambiguous {
                what: "descriptor registry",
                candidates: top
                    .iter()
                    .map(|candidate| {
                        format!(
                            "{}..{}",
                            self.model.scoping().symbol_span(candidate.0).start,
                            self.model.scoping().symbol_span(candidate.0).end
                        )
                    })
                    .collect(),
            });
        }
        let (symbol, origin_symbol, array_span, base_elements) = top[0];
        let public_symbol = self.public_alias_of(*symbol).unwrap_or(*symbol);
        let binding = self.model.binding_ref(public_symbol);
        self.roles
            .insert(symbol.index() as u32, BindingRole::DescriptorRegistry);
        self.roles.insert(
            public_symbol.index() as u32,
            BindingRole::DescriptorRegistry,
        );

        let overrides = self.registry_overrides(*origin_symbol, *array_span)?;
        let base_len = base_elements.len();
        let max_index = overrides.keys().copied().max().unwrap_or(0);
        let length = base_len.max(max_index.saturating_add(1));
        let mut elements = Vec::with_capacity(length);
        let mut population_assignments = Vec::new();

        for index in 0..length {
            if let Some(assignment) = overrides.get(&index) {
                population_assignments.push(PopulationAssignment {
                    index,
                    assignment_span: SourceSpan::from(assignment.assignment_span),
                    value_span: SourceSpan::from(assignment.value_span),
                    value_source: self.model.exact(assignment.value_span).to_string(),
                });
                if let Some(record) = self.descriptor_record(
                    index,
                    DescriptorPopulation::IndexOverride,
                    assignment.value,
                ) {
                    elements.push(record);
                }
            } else if let Some(value) = base_elements.get(index).copied() {
                if let Some(record) =
                    self.descriptor_record(index, DescriptorPopulation::ArrayLiteral, value)
                {
                    elements.push(record);
                }
            }
        }

        Ok(Some(DescriptorRegistry {
            binding,
            array_literal_span: Some(SourceSpan::from(*array_span)),
            length,
            elements,
            population_assignments,
        }))
    }

    /// True when the array, or a direct copy of it, is iterated with
    /// `forEach`/`map` by a callback that reads `ctor` from its own element
    /// parameter (slot 0), as the bootstrap does to start each collector.
    fn has_ctor_consumer(&self, array: SymbolId) -> bool {
        let mut holders = vec![array];
        for node in self.model.kind_nodes(oxc_ast::AstType::AssignmentExpression) {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            if crate::analysis::expression_symbol(&assignment.right, self.model.scoping())
                == Some(array)
            {
                holders.extend(simple_assignment_symbol(
                    &assignment.left,
                    self.model.scoping(),
                ));
            }
        }
        self.model.kind_nodes(oxc_ast::AstType::CallExpression).any(|node| {
            let AstKind::CallExpression(call) = node.kind() else {
                return false;
            };
            if !matches!(static_member_name(&call.callee), Some("forEach" | "map")) {
                return false;
            }
            let receiver = member_object_expression(&call.callee).and_then(|object| {
                crate::analysis::expression_symbol(object, self.model.scoping())
            });
            if !receiver.is_some_and(|symbol| holders.contains(&symbol)) {
                return false;
            }
            let Some(callback) = call_arguments(call)
                .first()
                .and_then(|argument| self.model.function_from_expression(argument))
            else {
                return false;
            };
            self.model.nodes_within(callback).into_iter().any(|inner| {
                let object = match inner.kind() {
                    AstKind::StaticMemberExpression(member) if member.property.name == "ctor" => {
                        &member.object
                    }
                    AstKind::ComputedMemberExpression(member)
                        if computed_member_name(member).as_deref() == Some("ctor") =>
                    {
                        &member.object
                    }
                    _ => return false,
                };
                self.model.node_within(inner.id(), callback)
                    && self.model.site_parameter_slot(object, callback) == Some(0)
            })
        })
    }

    fn descriptor_record(
        &self,
        index: usize,
        population: DescriptorPopulation,
        expression: &'a Expression<'a>,
    ) -> Option<DescriptorRecord> {
        // Scratch holders are reused between constant-index population writes.
        // Read the object at this assignment, never its final function-wide value.
        let read = expression;
        let mut expression = expression;
        for _ in 0..8 {
            if matches!(expression, Expression::ObjectExpression(_)) { break; }
            expression = expression_value_at_use(
                self.model.scoping(), self.model.nodes(), expression,
            ).ok()?.value;
        }
        let Expression::ObjectExpression(_) = expression else { return None; };
        let mut members = object_members(expression)?;
        // `d = {name: "x"}; d.ctor = make(); d.priority = false; table[i] = d`:
        // the descriptor literal completed by member writes before the read.
        if !std::ptr::eq(read, expression) {
            for (name, value) in self.late_descriptor_members(read, expression.span()) {
                // A placeholder (`{ctor: void 0}`) is what the late write fills.
                let placeholder = |member: &super::bindings::ObjectMember<'_>| {
                    matches!(member.value.without_parentheses(),
                        Expression::UnaryExpression(unary) if unary.operator == oxc_syntax::operator::UnaryOperator::Void)
                        || matches!(member.value.without_parentheses(), Expression::Identifier(id) if id.name == "undefined")
                };
                match members.iter_mut().find(|member| member.name == name) {
                    Some(member) if placeholder(member) => {
                        member.value = value;
                        member.span = value.span();
                    }
                    Some(_) if self.strict_registry => return None,
                    Some(_) => {}
                    None => members.push(super::bindings::ObjectMember { name, value, span: value.span() }),
                }
            }
        }
        let ctor = members
            .iter()
            .find(|member| member.name == "ctor")
            .map(|member| self.model.expr_record(member.value));
        let literal_at_read = |mut value: &Expression<'a>| {
            for _ in 0..8 {
                if let Some(literal) = literal_value(value) { return Some(literal); }
                if !self.strict_registry { return None; }
                value = expression_value_at_use(self.model.scoping(), self.model.nodes(), value).ok()?.value;
            }
            None
        };
        let priority = members
            .iter()
            .find(|member| member.name == "priority")
            .and_then(|member| literal_at_read(member.value));
        let name = members
            .iter()
            .find(|member| member.name == "name")
            .and_then(|member| literal_at_read(member.value));
        if self.strict_registry && ((name.is_none() && members.iter().any(|m| m.name == "name"))
            || (priority.is_none() && members.iter().any(|m| m.name == "priority"))) {
            return None;
        }
        Some(DescriptorRecord {
            index,
            population,
            span: SourceSpan::from(expression.span()),
            ctor,
            priority,
            name,
            source: self.model.exact(expression.span()).to_string(),
        })
    }

    /// Member writes `holder.name = value` that complete a descriptor literal
    /// bound to `holder` before `read` uses it: each write is an expression
    /// statement in the same block as the statement holding `read`, after the
    /// literal and before the read, and each name is written exactly once in
    /// that window. Anything else adds nothing.
    fn late_descriptor_members(
        &self,
        read: &Expression<'a>,
        literal: Span,
    ) -> Vec<(String, &'a Expression<'a>)> {
        let Expression::Identifier(holder) = read.without_parentheses() else {
            return Vec::new();
        };
        let Some(symbol) = crate::analysis::identifier_symbol(holder, self.model.scoping()) else {
            return Vec::new();
        };
        let nodes = self.model.nodes();
        let statement_block = |id: oxc_semantic::NodeId| {
            let mut id = id;
            loop {
                let parent = nodes.parent_id(id);
                if parent == id {
                    return None;
                }
                if matches!(nodes.kind(parent), AstKind::BlockStatement(_) | AstKind::FunctionBody(_) | AstKind::Program(_)) {
                    return Some((parent, id));
                }
                if matches!(
                    nodes.kind(parent),
                    AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
                ) {
                    return None;
                }
                id = parent;
            }
        };
        let holder_node = self.model.scoping().get_reference(holder.reference_id()).node_id();
        let Some((read_block, _)) = statement_block(holder_node) else {
            return Vec::new();
        };
        let mut found: BTreeMap<String, Vec<&'a Expression<'a>>> = BTreeMap::new();
        for reference in self.model.scoping().get_resolved_references(symbol) {
            let node = reference.node_id();
            let AstKind::IdentifierReference(identifier) = nodes.kind(node) else { continue };
            if identifier.span.start <= literal.end || identifier.span.start >= holder.span.start {
                continue;
            }
            let member_id = nodes.parent_id(node);
            let AstKind::StaticMemberExpression(_) = nodes.kind(member_id) else { continue };
            let assignment_id = nodes.parent_id(member_id);
            let AstKind::AssignmentExpression(assignment) = nodes.kind(assignment_id) else { continue };
            if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign {
                continue;
            }
            let AssignmentTarget::StaticMemberExpression(target) = &assignment.left else { continue };
            if target.object.span() != identifier.span {
                continue;
            }
            let statement = nodes.parent_id(assignment_id);
            if !matches!(nodes.kind(statement), AstKind::ExpressionStatement(_)) {
                continue;
            }
            let Some((block, child)) = statement_block(assignment_id) else { continue };
            if block != read_block || child != statement {
                continue;
            }
            found.entry(target.property.name.to_string()).or_default().push(&assignment.right);
        }
        found
            .into_iter()
            .filter_map(|(name, values)| (values.len() == 1).then(|| (name, values[0])))
            .collect()
    }

    fn resolve_object_expression(
        &self,
        expression: &'a Expression<'a>,
        depth: usize,
    ) -> Option<&'a Expression<'a>> {
        if depth > 8 {
            return None;
        }
        match expression {
            Expression::ObjectExpression(_) => Some(expression),
            Expression::Identifier(identifier) => {
                let symbol = crate::analysis::identifier_symbol(identifier, self.model.scoping())?;
                // A recovered table is commonly declared as an array literal
                // and later populated with `table[i] = identifier`, while the
                // identifier itself was first assigned a factory call and only
                // then replaced by its descriptor object. Try the last direct
                // assignment first, then the declarator initializer, and never
                // accept the identifier as its own value.
                for value in [
                    self.model.symbol_assigned_value(symbol),
                    self.model.symbol_value(symbol),
                ]
                .into_iter()
                .flatten()
                {
                    if value.span() == expression.span() {
                        continue;
                    }
                    if let Some(found) = self.resolve_object_expression(value, depth + 1) {
                        return Some(found);
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn resolve_call_expression(
        &self,
        expression: &'a Expression<'a>,
        depth: usize,
    ) -> Option<&'a CallExpression<'a>> {
        if depth > 8 {
            return None;
        }
        match expression {
            Expression::CallExpression(call) => Some(call),
            Expression::Identifier(identifier) => {
                let symbol = crate::analysis::identifier_symbol(identifier, self.model.scoping())?;
                for value in [
                    self.model.symbol_assigned_value(symbol),
                    self.model.symbol_value(symbol),
                ]
                .into_iter()
                .flatten()
                {
                    if value.span() == expression.span() {
                        continue;
                    }
                    if let Some(found) = self.resolve_call_expression(value, depth + 1) {
                        return Some(found);
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn registry_overrides(
        &self,
        registry: SymbolId,
        array_span: Span,
    ) -> Result<BTreeMap<usize, OverrideValue<'a>>, ProgramPropertiesError> {
        if self.strict_registry {
            return self.consumed_registry_overrides(registry, array_span);
        }
        let mut overrides = BTreeMap::new();
        for node in self.model.kind_nodes(oxc_ast::AstType::AssignmentExpression) {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
                continue;
            };
            let Some(object_symbol) =
                crate::analysis::expression_symbol(&member.object, self.model.scoping())
            else {
                continue;
            };
            if object_symbol != registry {
                continue;
            }
            // Population belongs to this array lifetime, not to a later
            // object stored in the same coalesced scratch binding.
            if let Ok(value) = expression_value_at_use(self.model.scoping(), self.model.nodes(), &member.object) {
                if value.value.span() != array_span { continue; }
            }
            let Some(index) = numeric_index(&member.expression) else {
                continue;
            };
            let index = index as usize;
            if index >= self.limits.max_array_elements {
                return Err(ProgramPropertiesError::Limit {
                    what: "descriptor index",
                    limit: self.limits.max_array_elements,
                });
            }
            overrides.insert(
                index,
                OverrideValue {
                    value: &assignment.right,
                    value_span: assignment.right.span(),
                    assignment_span: assignment.span,
                },
            );
        }
        Ok(overrides)
    }

    /// Resolve population at the first activation/handoff of this allocation.
    /// Use the shared reaching-definition proof for every receiver and alias.
    /// Only unconditional statements in the allocation's own list may donate.
    fn consumed_registry_overrides(
        &self, registry: SymbolId, array_span: Span,
    ) -> Result<BTreeMap<usize, OverrideValue<'a>>, ProgramPropertiesError> {
        let nodes = self.model.nodes();
        let scoping = self.model.scoping();
        let fail = |reason: &str| ProgramPropertiesError::InvalidInput(format!("consumed descriptor registry: {reason}"));
        let origin = nodes.iter().find(|n| n.span() == array_span && matches!(n.kind(), AstKind::ArrayExpression(_)))
            .ok_or_else(|| fail("allocation is absent"))?;
        let owner = self.model.enclosing_function(origin.id()).map(|f| f.node_id);
        let (list, _) = super::reaching::statement_of(nodes, origin.id()).map_err(|e| fail(&e))?;
        let resolves = |expression: &Expression<'a>| {
            let mut value = expression;
            for _ in 0..8 {
                if value.span() == array_span { return true; }
                match expression_value_at_use(scoping, nodes, value) {
                    Ok(next) if next.value.span() != value.span() => value = next.value,
                    _ => return false,
                }
            }
            false
        };
        let same_owner = |id| self.model.enclosing_function(id).map(|f| f.node_id) == owner;
        let mut cutoff = u32::MAX;
        // A handoff to an outer binding freezes the local population. Local
        // aliases remain eligible receivers until an actual consumer runs.
        for node in nodes.iter().filter(|n| same_owner(n.id()) && n.span().start > array_span.end) {
            if node.span().start >= cutoff { continue; }
            match node.kind() {
                AstKind::AssignmentExpression(a) if resolves(&a.right) => {
                    if let Some(symbol) = simple_assignment_symbol(&a.left, scoping) {
                        if self.model.enclosing_function(scoping.symbol_declaration(symbol)).map(|f| f.node_id) != owner {
                            cutoff = cutoff.min(a.span.start);
                        }
                    } else { return Err(fail("unresolved registry handoff")); }
                }
                AstKind::CallExpression(call) => {
                    if member_object_expression(&call.callee).is_some_and(&resolves) {
                        match static_member_name(&call.callee) {
                            Some("map" | "forEach" | "slice") => cutoff = cutoff.min(call.span.start),
                            _ => return Err(fail("unknown registry method/mutation")),
                        }
                    }
                    if call_arguments(call).iter().any(|arg| resolves(arg)) {
                        return Err(fail("registry escapes through an unproven call"));
                    }
                }
                AstKind::ReturnStatement(r) if r.argument.as_ref().is_some_and(&resolves) => cutoff = cutoff.min(r.span.start),
                _ => {}
            }
        }
        let mut overrides = BTreeMap::new();
        for node in nodes.iter() {
            if node.span().start <= array_span.end || node.span().start >= cutoff { continue; }
            // Updates/deletes cannot masquerade as reads of the allocation.
            let receiver = match node.kind() {
                AstKind::ComputedMemberExpression(m) => Some(&m.object),
                AstKind::StaticMemberExpression(m) => Some(&m.object),
                _ => None,
            };
            if let Some(receiver) = receiver {
                let mut root = receiver;
                while let Some(object) = member_object_expression(root) { root = object; }
                if resolves(root) {
                    let parent = nodes.kind(nodes.parent_id(node.id()));
                    if matches!(parent, AstKind::UpdateExpression(_) | AstKind::UnaryExpression(_)) {
                        return Err(fail("registry update/delete is unsupported"));
                    }
                    if receiver.span() != root.span() && matches!(parent, AstKind::AssignmentExpression(_)) {
                        return Err(fail("descriptor member mutation is unsupported"));
                    }
                }
            }
            let (receiver, index, assignment) = match node.kind() {
                AstKind::AssignmentExpression(a) => match &a.left {
                    AssignmentTarget::ComputedMemberExpression(m) => (&m.object, Some(&m.expression), a),
                    AssignmentTarget::StaticMemberExpression(m) => (&m.object, None, a),
                    _ => continue,
                },
                _ => continue,
            };
            if !same_owner(node.id()) {
                let mut value = receiver;
                let mut captured = crate::analysis::expression_symbol(value, scoping) == Some(registry);
                for _ in 0..8 {
                    let Some(symbol) = crate::analysis::expression_symbol(value, scoping) else { break; };
                    let Some(next) = self.model.symbol_value(symbol) else { break; };
                    if next.span() == array_span { captured = true; break; }
                    if next.span() == value.span() { break; }
                    value = next;
                }
                let nested = self.model.enclosing_function(node.id());
                if captured && nested.is_some_and(|f| self.model.kind_nodes(oxc_ast::AstType::CallExpression).any(|n| {
                    matches!(n.kind(), AstKind::CallExpression(c) if same_owner(n.id()) && c.span.start < cutoff
                        && self.model.function_from_expression(&c.callee).is_some_and(|called| called.node_id == f.node_id))
                })) { return Err(fail("called nested writer is unsupported")); }
                continue;
            }
            if !resolves(receiver) {
                // A write to the original binding with an unprovable lifetime
                // cannot safely be treated as belonging to a different array.
                if same_owner(node.id()) && crate::analysis::expression_symbol(receiver, scoping) == Some(registry)
                    && expression_value_at_use(scoping, nodes, receiver).is_err() {
                    return Err(fail("receiver lifetime is unproven"));
                }
                continue;
            }
            let (write_list, position) = super::reaching::statement_of(nodes, node.id()).map_err(|e| fail(&e))?;
            if !std::ptr::eq(list, write_list)
                || !matches!(&write_list[position], Statement::ExpressionStatement(s) if s.expression.span() == assignment.span)
                || assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign {
                return Err(fail("population is guarded or not a plain assignment"));
            }
            let index = index.and_then(numeric_index).ok_or_else(|| fail("nonconstant registry index or member mutation"))? as usize;
            if index >= self.limits.max_array_elements { return Err(fail("registry index exceeds limit")); }
            overrides.insert(index, OverrideValue { value: &assignment.right, value_span: assignment.right.span(), assignment_span: assignment.span });
        }
        Ok(overrides)
    }

    // -----------------------------------------------------------------------
    // Waves
    // -----------------------------------------------------------------------

    /// A setup branch can return its session before later scratch uses of a
    /// captured selector. Those later writes are on the other execution path.
    /// Accept only one literal definition in a branch ending in return, with
    /// no intervening or captured write on that branch.
    fn returning_branch_array(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        let mut found = Vec::new();
        for node in self.model.kind_nodes(oxc_ast::AstType::AssignmentExpression) {
            let AstKind::AssignmentExpression(assignment) = node.kind() else { continue; };
            if simple_assignment_symbol(&assignment.left, self.model.scoping()) != Some(symbol)
                || !matches!(assignment.right, Expression::ArrayExpression(_)) { continue; }
            let mut parent = self.model.nodes().parent_id(node.id());
            let mut branch = None;
            loop {
                match self.model.nodes().kind(parent) {
                    AstKind::IfStatement(statement) => {
                        for arm in [Some(&statement.consequent), statement.alternate.as_ref()].into_iter().flatten() {
                            if let Statement::BlockStatement(block) = arm {
                                if block.span.start < assignment.span.start && assignment.span.end < block.span.end
                                    && matches!(block.body.last(), Some(Statement::ReturnStatement(_))) {
                                    branch = Some(block.span); break;
                                }
                            }
                        }
                        if branch.is_some() { break; }
                    }
                    AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => break,
                    _ => {}
                }
                let next = self.model.nodes().parent_id(parent); if next == parent { break; } parent = next;
            }
            let Some(branch) = branch else { continue; };
            let owner = self.model.enclosing_function(node.id()).map(|s| s.span);
            let safe = self.model.scoping().get_resolved_references(symbol).all(|reference| {
                if !reference.is_write() { return true; }
                let write = self.model.nodes().get_node(reference.node_id()).span();
                self.model.enclosing_function(reference.node_id()).map(|s| s.span) == owner
                    && (write.end <= assignment.span.end || write.start >= branch.end)
            });
            if safe { found.push(&assignment.right); }
        }
        (found.len() == 1).then(|| found[0])
    }

    fn detect_waves(
        &mut self,
        descriptors: Option<&DescriptorRegistry>,
    ) -> Result<Option<WaveRegistry>, ProgramPropertiesError> {
        let mut rows: Vec<(SymbolId, Vec<Vec<usize>>, Span)> = Vec::new();
        let mut selectors: Vec<(SymbolId, Vec<bool>, Span)> = Vec::new();
        let mut function_arrays: Vec<(SymbolId, Vec<FunctionSite>, Span)> = Vec::new();

        for symbol in &self.model.main_symbols {
            let Some(initializer) = self.model.symbol_value(*symbol)
                .filter(|value| array_elements(value).is_some())
                .or_else(|| self.returning_branch_array(*symbol)) else {
                continue;
            };
            let Some(elements) = array_elements(initializer) else {
                continue;
            };
            if elements.len() > self.limits.max_array_elements {
                return Err(ProgramPropertiesError::Limit {
                    what: "registry array elements",
                    limit: self.limits.max_array_elements,
                });
            }
            if let Some(parsed_rows) = array_of_integer_arrays(initializer) {
                rows.push((*symbol, parsed_rows, initializer.span()));
                continue;
            }
            if let Some(parsed_selector) = is_boolean_array(initializer) {
                selectors.push((*symbol, parsed_selector, initializer.span()));
                continue;
            }
            let mut sites = Vec::with_capacity(elements.len());
            let mut all_functions = true;
            for element in &elements {
                match self.model.function_from_expression(element) {
                    Some(site) => sites.push(site),
                    None => {
                        all_functions = false;
                        break;
                    }
                }
            }
            if all_functions && !sites.is_empty() {
                function_arrays.push((*symbol, sites, initializer.span()));
            }
        }

        let mut triples = Vec::new();
        for (row_symbol, row_values, row_span) in &rows {
            for (selector_symbol, selector_values, selector_span) in &selectors {
                if row_values.len() != selector_values.len() {
                    continue;
                }
                for (function_symbol, functions, function_span) in &function_arrays {
                    if functions.len() != row_values.len() {
                        continue;
                    }
                    if !functions.iter().all(|site| self.has_observer_sink(*site)) {
                        continue;
                    }
                    if let Some(descriptors) = descriptors {
                        if row_values
                            .iter()
                            .flatten()
                            .any(|index| *index >= descriptors.length)
                        {
                            continue;
                        }
                    }
                    let symbols = [*row_symbol, *selector_symbol, *function_symbol];
                    if !self.has_shared_consumer(&symbols) {
                        continue;
                    }
                    triples.push(Triple {
                        row_symbol: *row_symbol,
                        selector_symbol: *selector_symbol,
                        function_symbol: *function_symbol,
                        row_values: row_values.clone(),
                        selector_values: selector_values.clone(),
                        functions: functions.clone(),
                        row_span: *row_span,
                        selector_span: *selector_span,
                        function_span: *function_span,
                    });
                }
            }
        }

        if triples.is_empty() {
            return Ok(None);
        }
        let best: Vec<_> = triples.iter().collect();
        if best.len() > 1 {
            return Err(ProgramPropertiesError::Ambiguous {
                what: "wave registry (function array + index rows + routing selector)",
                candidates: best
                    .iter()
                    .map(|triple| {
                        format!(
                            "functions {}..{}, rows {}..{}, selector {}..{}",
                            triple.function_span.start,
                            triple.function_span.end,
                            triple.row_span.start,
                            triple.row_span.end,
                            triple.selector_span.start,
                            triple.selector_span.end
                        )
                    })
                    .collect(),
            });
        }
        let triple = best[0];
        self.roles.insert(
            triple.function_symbol.index() as u32,
            BindingRole::WaveRegistry,
        );
        self.roles
            .insert(triple.row_symbol.index() as u32, BindingRole::WaveIndexRows);
        self.roles.insert(
            triple.selector_symbol.index() as u32,
            BindingRole::WaveRoutingSelector,
        );

        let mut waves = Vec::with_capacity(triple.functions.len());
        let mut covered = BTreeSet::new();
        for (index, (site, row)) in triple.functions.iter().zip(&triple.row_values).enumerate() {
            let observer_slots = row
                .iter()
                .enumerate()
                .map(|(slot, signal)| ObserverSlot {
                    slot: slot as u32,
                    signal: *signal as u32,
                })
                .collect();
            covered.extend(row.iter().copied());
            waves.push(WaveRecord {
                index,
                function: self.model.function_ref(*site),
                signal_indices: row.clone(),
                observer_slots,
                routing_selector: triple.selector_values[index],
                function_span: SourceSpan::from(site.span),
                row_span: SourceSpan::from(triple.row_span),
                selector_span: SourceSpan::from(triple.selector_span),
            });
            self.assign_role_for_site(*site, BindingRole::WaveCollector);
        }

        let direct_slots = self.direct_slots(descriptors, &covered);
        for slot in &direct_slots {
            if let Some(ctor_binding) = slot.ctor.binding.as_ref() {
                self.roles
                    .insert(ctor_binding.symbol, BindingRole::DirectCollector);
            }
        }

        let function_public = self
            .public_alias_of(triple.function_symbol)
            .unwrap_or(triple.function_symbol);
        let row_public = self
            .public_alias_of(triple.row_symbol)
            .unwrap_or(triple.row_symbol);
        let selector_public = self
            .public_alias_of(triple.selector_symbol)
            .unwrap_or(triple.selector_symbol);
        self.roles
            .insert(function_public.index() as u32, BindingRole::WaveRegistry);
        self.roles
            .insert(row_public.index() as u32, BindingRole::WaveIndexRows);
        self.roles.insert(
            selector_public.index() as u32,
            BindingRole::WaveRoutingSelector,
        );
        Ok(Some(WaveRegistry {
            function_array: self.model.binding_ref(function_public),
            index_rows: self.model.binding_ref(row_public),
            routing_selector: self.model.binding_ref(selector_public),
            waves,
            direct_slots,
        }))
    }

    fn has_observer_sink(&self, site: FunctionSite) -> bool {
        self.model.nodes_within(site).into_iter().any(|node| {
            if !self.model.node_within(node.id(), site) {
                return false;
            }
            let AstKind::CallExpression(call) = node.kind() else {
                return false;
            };
            matches!(
                static_member_name(&call.callee),
                Some("next") | Some("error") | Some("complete")
            ) || (static_member_name(&call.callee) == Some("call")
                && member_object_expression(&call.callee)
                    .and_then(|method| {
                        static_member_name(method).or_else(|| {
                            expression_value_at_use(self.model.scoping(), self.model.nodes(), method)
                                .ok().and_then(|definition| static_member_name(definition.value))
                        })
                    })
                    .is_some_and(|name| matches!(name, "next" | "error" | "complete")))
        })
    }

    fn has_shared_consumer(&self, symbols: &[SymbolId; 3]) -> bool {
        self.model.all_sites().iter().any(|site| {
            let Some(body) = site.body_span else {
                return false;
            };
            symbols.iter().all(|symbol| {
                self.model
                    .scoping()
                    .get_resolved_reference_ids(*symbol)
                    .iter()
                    .any(|reference| {
                        let node_id = self.model.scoping().get_reference(*reference).node_id();
                        let span = self.model.nodes().get_node(node_id).span();
                        span.start >= body.start && span.end <= body.end
                    })
            })
        })
    }

    fn direct_slots(
        &self,
        descriptors: Option<&DescriptorRegistry>,
        covered: &BTreeSet<usize>,
    ) -> Vec<DirectSlot> {
        let Some(descriptors) = descriptors else {
            return Vec::new();
        };
        let mut slots = Vec::new();
        for element in &descriptors.elements {
            if covered.contains(&element.index) {
                continue;
            }
            let Some(ctor) = element.ctor.as_ref() else {
                continue;
            };
            let nullish = match ctor.literal {
                Some(LiteralValue::Null) | Some(LiteralValue::Undefined) => true,
                Some(_) => false,
                None => ctor.kind == crate::signals::schema::ValueKind::Literal,
            };
            if nullish {
                continue;
            }
            let resolution = resolve_descriptor_ctor(self.model, ctor);
            let ctor_call = resolution
                .wrapper
                .map(|site| self.model.function_ref(site))
                .or_else(|| ctor.function.clone());
            let callback = resolution
                .callback
                .map(|site| self.model.function_ref(site));
            let mut unresolved = vec![
                "direct-constructor collector/wire layout is left to the per-signal pass"
                    .to_string(),
            ];
            unresolved.extend(resolution.refusals);
            let declared_not_emitted = resolution
                .callback
                .is_some_and(|callback| task_completes_without_value(self.model, callback));
            slots.push(DirectSlot {
                signal: element.index,
                descriptor_index: element.index,
                ctor: ctor.clone(),
                ctor_call,
                callback,
                declared_not_emitted,
                unresolved,
            });
        }
        slots
    }

    // -----------------------------------------------------------------------
    // Session/context
    // -----------------------------------------------------------------------

    fn detect_session(
        &mut self,
        descriptors: Option<&DescriptorRegistry>,
        _waves: Option<&WaveRegistry>,
    ) -> Result<Option<SessionProperties>, ProgramPropertiesError> {
        let mut candidates = Vec::new();
        for site in self.model.all_sites().iter().copied() {
            let Some(body) = site.body_span else {
                continue;
            };
            let mut context_span = None;
            for node in self.model.kind_nodes(oxc_ast::AstType::ObjectExpression) {
                let AstKind::ObjectExpression(object) = node.kind() else {
                    continue;
                };
                if object.span.start < body.start || object.span.end > body.end {
                    continue;
                }
                let members = object_members_of(object);
                let keys: BTreeSet<&str> =
                    members.iter().map(|member| member.name.as_str()).collect();
                if keys.contains("global") && keys.contains("transactionToken") {
                    context_span = Some(object.span);
                    break;
                }
            }
            if let Some(context_span) = context_span {
                let returns = self.returns_object_with_keys(site, &["init", "teardown"]);
                if !returns.is_empty() {
                    candidates.push((site, context_span));
                }
            }
        }
        if candidates.is_empty() {
            return Ok(None);
        }
        if candidates.len() > 1 {
            // Several functions carry the session shape. Refusing the whole
            // analysis loses every signal in the capture for a fact that only
            // labels the session contract, so that contract stays unproven and
            // the rest of the report is produced.
            self.diagnostics.push(Diagnostic {
                code: "session_constructor_ambiguous".to_string(),
                message: format!(
                    "{} session-constructor candidates; the session contract stays unresolved",
                    candidates.len()
                ),
                span: candidates.first().map(|(site, _)| SourceSpan::from(site.span)),
            });
            return Ok(None);
        }
        let (site, context_span) = candidates[0];
        self.assign_role_for_site(site, BindingRole::SessionConstructor);

        let context_local = self.context_local_symbol(site, context_span);
        let context_binding = self.main_alias_of(context_local);

        let context_node = self
            .model
            .nodes()
            .iter()
            .find(|node| node.span() == context_span)
            .map(|node| node.id());
        let mut context_fields = Vec::new();
        if let Some(context_local) = context_local {
            if let Some(node_id) = context_node {
                if let AstKind::ObjectExpression(object) = self.model.nodes().kind(node_id) {
                    let members = object_members_of(object);
                    for member in members {
                        context_fields.push(ContextField {
                            name: member.name.clone(),
                            declaration_span: SourceSpan::from(member.span),
                            declared: literal_value(member.value),
                            assigned: None,
                            session_argument: None,
                            public_init_argument: None,
                        });
                    }
                }
            }
            self.apply_context_assignments(site, context_local, &mut context_fields);
        }

        let init = self.returned_property_function(site, "init");
        let teardown = self.returned_property_function(site, "teardown");
        let public_init = self.returned_property_function(self.model.main_site(), "init");
        if let Some(public_init) = public_init {
            if let Some(symbol) = public_init.binding_symbol {
                self.roles
                    .insert(symbol.index() as u32, BindingRole::SessionInit);
            }
        }

        let mut argument_mapping = Vec::new();
        if let Some(public_init) = public_init {
            for call in self.model.new_expressions_to(site) {
                if !self.model.node_within(call.node_id.get(), public_init) {
                    continue;
                }
                for (index, argument) in new_arguments(call).into_iter().enumerate() {
                    let expression = self.model.expr_record(argument);
                    let public_init_argument =
                        self.model.site_parameter_slot(argument, public_init);
                    argument_mapping.push(SessionArgumentMapping {
                        session_argument: index,
                        public_init_argument: public_init_argument.map(|value| value as usize),
                        expression,
                    });
                }
                break;
            }
        }

        let stores = self.detect_stores(site, descriptors);
        let default_filter = self.detect_default_filter(site);
        if let Some(symbol) = default_filter {
            self.roles
                .insert(symbol.index() as u32, BindingRole::DefaultSignalFilter);
        }
        let hash_chain = self.detect_hash_chain(site, context_local, context_binding.as_ref());

        let mut session = SessionProperties {
            constructor: self.model.function_ref(site),
            context_binding,
            context_span: Some(SourceSpan::from(context_span)),
            context_fields,
            init: init.map(|site| self.model.function_ref(site)),
            teardown: teardown.map(|site| self.model.function_ref(site)),
            argument_mapping,
            stores,
            default_filter: default_filter.map(|symbol| self.model.binding_ref(symbol)),
            envelope: None,
            serializer: None,
            hash_chain,
        };
        self.link_context_public_arguments(&mut session);
        if let Some(chain) = session.hash_chain.as_mut() {
            if let Some(field) = session
                .context_fields
                .iter()
                .find(|field| field.name == "transactionToken")
            {
                chain.session_argument = field.session_argument;
                chain.public_init_argument = field.public_init_argument;
            }
        }
        Ok(Some(session))
    }

    fn context_local_symbol(&self, site: FunctionSite, context_span: Span) -> Option<SymbolId> {
        let node = self
            .model
            .nodes()
            .iter()
            .find(|node| node.span() == context_span)?;
        let parent = self
            .model
            .nodes()
            .get_node(self.model.nodes().parent_id(node.id()));
        match parent.kind() {
            AstKind::VariableDeclarator(declarator)
                if declarator
                    .init
                    .as_ref()
                    .is_some_and(|init| init.span() == context_span) =>
            {
                binding_pattern_symbol(&declarator.id)
            }
            AstKind::AssignmentExpression(assignment) => {
                let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left
                else {
                    return None;
                };
                crate::analysis::identifier_symbol(identifier, self.model.scoping())
            }
            _ => None,
        }
        .filter(|symbol| {
            let span = self.model.scoping().symbol_span(*symbol);
            span.start >= site.span.start && span.end <= site.span.end
        })
    }

    /// The main binding that receives `local`, directly (`main = local`) or
    /// through one intermediate copy (`mid = local; main = mid`). The
    /// intermediate must have that copy as its only write and no initializer
    /// other than `undefined`, so it can only ever hold `local`'s value.
    fn main_alias_of(&self, local: Option<SymbolId>) -> Option<BindingRef> {
        let local = local?;
        if let Some(binding) = self.direct_main_alias_of(local) {
            return Some(binding);
        }
        let mut found = None;
        for node in self.model.kind_nodes(oxc_ast::AstType::AssignmentExpression) {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let Some(mid) = simple_assignment_symbol(&assignment.left, self.model.scoping()) else {
                continue;
            };
            if mid == local
                || self.model.main_symbols.contains(&mid)
                || crate::analysis::expression_symbol(&assignment.right, self.model.scoping())
                    != Some(local)
                || self.model.binding_writes(mid) != 1
                || self.model.initializer(mid).is_some_and(|value| {
                    !matches!(literal_value(value), Some(LiteralValue::Undefined))
                })
            {
                continue;
            }
            let Some(binding) = self.direct_main_alias_of(mid) else {
                continue;
            };
            if found.is_some() {
                return None;
            }
            found = Some(binding);
        }
        found
    }

    fn direct_main_alias_of(&self, local: SymbolId) -> Option<BindingRef> {
        for node in self.model.kind_nodes(oxc_ast::AstType::AssignmentExpression) {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                continue;
            };
            let Some(target) = crate::analysis::identifier_symbol(identifier, self.model.scoping())
            else {
                continue;
            };
            if !self.model.main_symbols.contains(&target) {
                continue;
            }
            if crate::analysis::expression_symbol(&assignment.right, self.model.scoping())
                == Some(local)
            {
                return Some(self.model.binding_ref(target));
            }
        }
        None
    }

    fn apply_context_assignments(
        &self,
        session_site: FunctionSite,
        context_local: SymbolId,
        fields: &mut [ContextField],
    ) {
        let mut assignments: HashMap<String, (Span, &Expression<'a>)> = HashMap::new();
        for node in self.model.kind_nodes(oxc_ast::AstType::AssignmentExpression) {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let Some((object, name)) = member_target_key(&assignment.left) else {
                continue;
            };
            if crate::analysis::expression_symbol(object, self.model.scoping())
                != Some(context_local)
            {
                continue;
            }
            assignments.insert(name, (assignment.span, &assignment.right));
        }
        for field in fields {
            if let Some((span, value)) = assignments.get(&field.name) {
                field.assigned = Some(self.model.expr_record(value));
                field.session_argument = self
                    .model
                    .site_parameter_slot(value, session_site)
                    .or_else(|| self.assigned_from_arguments(value, session_site))
                    .map(|slot| slot as usize);
                field.declaration_span = SourceSpan::from(*span);
            }
        }
    }

    /// The session parameter slot a local copies: its declarator
    /// initializer or a direct assignment is the session's own slot.
    fn assigned_from_arguments(
        &self,
        expression: &Expression<'a>,
        session_site: FunctionSite,
    ) -> Option<u32> {
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let symbol = crate::analysis::identifier_symbol(identifier, self.model.scoping())?;
        if let Some(slot) = self
            .model
            .initializer(symbol)
            .and_then(|value| self.model.site_parameter_slot(value, session_site))
        {
            return Some(slot);
        }
        for node in self.model.kind_nodes(oxc_ast::AstType::AssignmentExpression) {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                continue;
            };
            if crate::analysis::identifier_symbol(target, self.model.scoping()) == Some(symbol) {
                if let Some(slot) = self
                    .model
                    .site_parameter_slot(&assignment.right, session_site)
                {
                    return Some(slot);
                }
            }
        }
        None
    }

    fn link_context_public_arguments(&self, session: &mut SessionProperties) {
        for field in &mut session.context_fields {
            if let Some(session_argument) = field.session_argument {
                field.public_init_argument = session
                    .argument_mapping
                    .iter()
                    .find(|mapping| mapping.session_argument == session_argument)
                    .and_then(|mapping| mapping.public_init_argument);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Stores/filter/hash
    // -----------------------------------------------------------------------

    fn detect_stores(
        &mut self,
        session_site: FunctionSite,
        descriptors: Option<&DescriptorRegistry>,
    ) -> StoreWiring {
        let mut wiring = StoreWiring {
            latest_values: None,
            statuses: None,
            debug_names: None,
            sentinel: None,
            sentinel_binding: None,
            error_buffer: None,
        };
        let Some(descriptors) = descriptors else {
            return wiring;
        };
        let registry_symbol = descriptors.binding.symbol;
        for node in self.model.nodes_within(session_site).into_iter() {
            if !self.model.node_within(node.id(), session_site) {
                continue;
            }
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            if static_member_name(&call.callee) != Some("map") {
                continue;
            }
            let Some(object) = member_object_expression(&call.callee) else {
                continue;
            };
            if crate::analysis::expression_symbol(object, self.model.scoping())
                .map(|symbol| symbol.index() as u32)
                != Some(registry_symbol)
            {
                continue;
            }
            let Some(callback) = call_arguments(call).first().copied() else {
                continue;
            };
            let Some(callback_site) = self.model.function_from_expression(callback) else {
                continue;
            };
            let Some(target) = self.assignment_target_of_call(call) else {
                continue;
            };
            let target_binding = self.model.binding_ref(target);
            if self.callback_returns_status_object(callback_site) {
                wiring.statuses = Some(target_binding);
                self.roles
                    .insert(target.index() as u32, BindingRole::StatusStore);
            } else if self.callback_returns_member(callback_site, "debugName") {
                wiring.debug_names = Some(target_binding);
                self.roles
                    .insert(target.index() as u32, BindingRole::DebugNameStore);
            } else if let Some(sentinel) = self.callback_returns_identifier(callback_site) {
                if self.is_create_null(sentinel) {
                    wiring.latest_values = Some(target_binding);
                    wiring.sentinel = self
                        .model
                        .symbol_value(sentinel)
                        .map(|expr| self.model.expr_record(expr));
                    wiring.sentinel_binding = Some(self.model.binding_ref(sentinel));
                    self.roles
                        .insert(target.index() as u32, BindingRole::LatestValueStore);
                    self.roles
                        .insert(sentinel.index() as u32, BindingRole::SentinelObject);
                }
            } else if let Some(sentinel) = self.callback_returns_create_null(callback_site) {
                wiring.latest_values = Some(target_binding);
                wiring.sentinel = Some(self.model.expr_record(sentinel));
                self.roles
                    .insert(target.index() as u32, BindingRole::LatestValueStore);
            }
        }
        wiring.error_buffer = self.error_buffer(session_site);
        wiring
    }

    fn assignment_target_of_call(&self, call: &CallExpression<'a>) -> Option<SymbolId> {
        let node_id = call.node_id.get();
        let parent = self
            .model
            .nodes()
            .get_node(self.model.nodes().parent_id(node_id));
        match parent.kind() {
            AstKind::VariableDeclarator(declarator) => binding_pattern_symbol(&declarator.id),
            AstKind::AssignmentExpression(assignment) => {
                let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left
                else {
                    return None;
                };
                crate::analysis::identifier_symbol(identifier, self.model.scoping())
            }
            _ => None,
        }
    }

    fn callback_returns_status_object(&self, site: FunctionSite) -> bool {
        self.returns_object_with_keys(site, &["hasCollected", "hasErrored"])
            .iter()
            .any(|keys| keys.contains("hasCollected") && keys.contains("hasErrored"))
    }

    /// The callback returns `member_name` read from its own element
    /// parameter (slot 0), e.g. `element.debugName`.
    fn callback_returns_member(&self, site: FunctionSite, member_name: &str) -> bool {
        self.returned_values(site).into_iter().any(|argument| {
            static_member_name(argument) == Some(member_name)
                && member_object_expression(argument)
                    .is_some_and(|object| self.model.site_parameter_slot(object, site) == Some(0))
        })
    }

    fn callback_returns_identifier(&self, site: FunctionSite) -> Option<SymbolId> {
        self.returned_values(site)
            .into_iter()
            .find_map(|argument| match argument {
                Expression::Identifier(identifier) => {
                    crate::analysis::identifier_symbol(identifier, self.model.scoping())
                }
                _ => None,
            })
    }

    fn callback_returns_create_null(&self, site: FunctionSite) -> Option<&'a Expression<'a>> {
        self.returned_values(site)
            .into_iter()
            .find(|argument| self.is_create_null_expression(argument))
    }

    fn is_create_null_expression(&self, expression: &'a Expression<'a>) -> bool {
        let Expression::CallExpression(call) = expression else {
            return false;
        };
        if static_member_name(&call.callee) != Some("create") {
            return false;
        }
        let object = match &call.callee {
            Expression::StaticMemberExpression(member) => Some(&member.object),
            Expression::ComputedMemberExpression(member) => Some(&member.object),
            _ => None,
        };
        object.is_some_and(|object| {
            matches!(object, Expression::Identifier(identifier) if identifier.name == "Object")
        }) && call_arguments(call)
            .first()
            .is_some_and(|arg| matches!(literal_value(arg), Some(LiteralValue::Null)))
    }

    fn is_create_null(&self, symbol: SymbolId) -> bool {
        let Some(initializer) = self.model.symbol_value(symbol) else {
            return false;
        };
        let Expression::CallExpression(call) = initializer else {
            return false;
        };
        if static_member_name(&call.callee) != Some("create") {
            return false;
        }
        let object = match &call.callee {
            Expression::StaticMemberExpression(member) => Some(&member.object),
            Expression::ComputedMemberExpression(member) => Some(&member.object),
            _ => None,
        };
        object.is_some_and(|object| {
            matches!(object, Expression::Identifier(identifier) if identifier.name == "Object")
        }) && call_arguments(call)
            .first()
            .is_some_and(|arg| matches!(literal_value(arg), Some(LiteralValue::Null)))
    }

    fn error_buffer(&mut self, session_site: FunctionSite) -> Option<BindingRef> {
        // The snapshot getter's `errors: ring.toArray()` is the structural
        // anchor. It is found before this call in normal operation, but the
        // ring binding is also reachable by its `toArray` method shape.
        for node in self.model.nodes_within(session_site).into_iter() {
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            if !self.model.node_within(node.id(), session_site) {
                continue;
            }
            if static_member_name(&call.callee) != Some("toArray") {
                continue;
            }
            let Some(object) = member_object_expression(&call.callee) else {
                continue;
            };
            let Some(symbol) = crate::analysis::expression_symbol(object, self.model.scoping())
            else {
                continue;
            };
            if let Some(constructor) = self.error_ring_constructor(symbol) {
                self.roles
                    .insert(symbol.index() as u32, BindingRole::ErrorRingBuffer);
                self.assign_role_for_site(constructor, BindingRole::ErrorRingConstructor);
                return Some(self.model.binding_ref(symbol));
            }
        }
        None
    }

    /// The constructor of an error ring buffer bound to `symbol`, with the
    /// same structural evidence in the function form (`function R() {...}` plus
    /// `R.prototype.m = f` writes) and the class form (`class R {...}`, also
    /// through a proven alias such as `let K = R`):
    ///
    /// * the binding's value is `new R(<numeric literal>)`, a fixed capacity;
    /// * R's constructor stores a value derived from its first argument on
    ///   `this` (the normalised capacity);
    /// * R has the `toArray` method the snapshot consumer calls, and it returns
    ///   an ordered `this.<buffer>.slice(...)` snapshot;
    /// * some R method writes `this.<buffer>[this.<cursor>] = <its argument>`
    ///   and wraps that cursor back to `0`, overwriting on capacity.
    ///
    /// The returned site is the constructor function; for a class it carries
    /// the class binding so the role lands on the class and its aliases.
    fn error_ring_constructor(&self, symbol: SymbolId) -> Option<FunctionSite> {
        let Some(Expression::NewExpression(new)) = self.model.symbol_value(symbol) else {
            return None;
        };
        let capacity = new_arguments(new);
        if capacity.len() != 1 || numeric_value(capacity[0]).is_none() {
            return None;
        }
        let (constructor, methods) = self.constructor_shape(&new.callee)?;
        let constructor_captures_capacity = self.model.kind_nodes(oxc_ast::AstType::AssignmentExpression).any(|node| {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                return false;
            };
            self.model.owning_site(node.id()).map(|site| site.span) == Some(constructor.span)
                && this_member_name(&assignment.left).is_some()
                && self.mentions_slot(&assignment.right, constructor, 0, 0)
        });
        if !constructor_captures_capacity {
            return None;
        }
        let snapshot = methods
            .iter()
            .find(|(name, _)| name == "toArray")
            .is_some_and(|(_, site)| self.returns_this_slice(*site));
        let overwrites = methods
            .iter()
            .any(|(_, site)| self.overwrites_at_cursor(*site));
        (snapshot && overwrites).then_some(constructor)
    }

    /// The constructor function and the named methods of the value a `new`
    /// callee denotes through proven bindings.
    fn constructor_shape(
        &self,
        callee: &Expression<'a>,
    ) -> Option<(FunctionSite, Vec<(String, FunctionSite)>)> {
        let scoping = self.model.scoping();
        let mut symbol = crate::analysis::expression_symbol(callee, scoping)?;
        for _ in 0..16 {
            if !scoping.symbol_redeclarations(symbol).is_empty() {
                return None;
            }
            let writes: Vec<_> = scoping
                .get_resolved_references(symbol)
                .filter(|reference| reference.is_write())
                .collect();
            let declaration = scoping.symbol_declaration(symbol);
            match self.model.nodes().kind(declaration) {
                AstKind::Function(function) if writes.is_empty() => {
                    let site = self.model.site_by_span(function.span)?;
                    return Some((site, self.prototype_methods(symbol)));
                }
                AstKind::Class(class) if writes.is_empty() => {
                    return self.class_shape(class, symbol);
                }
                AstKind::VariableDeclarator(declarator) => {
                    // A single definition: the initializer, or one plain
                    // assignment to an uninitialized binding.
                    let value = match (declarator.init.as_ref(), writes.as_slice()) {
                        (Some(value), []) => value,
                        (None, [write]) => {
                            let parent = self.model.nodes().parent_id(write.node_id());
                            let AstKind::AssignmentExpression(assignment) =
                                self.model.nodes().kind(parent)
                            else {
                                return None;
                            };
                            if assignment.operator
                                != oxc_syntax::operator::AssignmentOperator::Assign
                            {
                                return None;
                            }
                            &assignment.right
                        }
                        _ => return None,
                    };
                    match value.without_parentheses() {
                        Expression::ClassExpression(class) if writes.is_empty() => {
                            return self.class_shape(class, symbol);
                        }
                        other => symbol = crate::analysis::expression_symbol(other, scoping)?,
                    }
                }
                _ => return None,
            }
        }
        None
    }

    /// A class's explicit constructor and its named instance methods. The
    /// constructor site carries the class binding.
    fn class_shape(
        &self,
        class: &Class<'a>,
        binding: SymbolId,
    ) -> Option<(FunctionSite, Vec<(String, FunctionSite)>)> {
        let mut constructor = None;
        let mut methods = Vec::new();
        for element in &class.body.body {
            let ClassElement::MethodDefinition(method) = element else {
                continue;
            };
            let Some(site) = self.model.site_by_span(method.value.span) else {
                continue;
            };
            match method.kind {
                MethodDefinitionKind::Constructor => constructor = Some(site),
                MethodDefinitionKind::Method if !method.r#static => {
                    if let Some(name) = property_key_name(&method.key) {
                        methods.push((name, site));
                    }
                }
                _ => {}
            }
        }
        let constructor = constructor?;
        Some((
            FunctionSite {
                binding_symbol: Some(binding),
                ..constructor
            },
            methods,
        ))
    }

    /// `R.prototype.name = f` (or the computed-key form) writes anywhere in
    /// the program, where `f` is a proven function value.
    fn prototype_methods(&self, constructor: SymbolId) -> Vec<(String, FunctionSite)> {
        let scoping = self.model.scoping();
        let mut methods = Vec::new();
        for node in self.model.kind_nodes(oxc_ast::AstType::AssignmentExpression) {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let (name, prototype) = match &assignment.left {
                AssignmentTarget::StaticMemberExpression(member) => {
                    (member.property.name.to_string(), &member.object)
                }
                AssignmentTarget::ComputedMemberExpression(member) => {
                    let Some(name) = computed_member_name(member) else {
                        continue;
                    };
                    (name, &member.object)
                }
                _ => continue,
            };
            if static_member_name(prototype) != Some("prototype")
                || member_object_expression(prototype)
                    .and_then(|object| crate::analysis::expression_symbol(object, scoping))
                    != Some(constructor)
            {
                continue;
            }
            if let Ok(site) = proven_function_value(self.model, Some(&assignment.right), 0) {
                methods.push((name, site));
            }
        }
        methods
    }

    /// True when `expression` reads argument `slot` of `site` anywhere inside
    /// it (through operators, conditionals and member objects).
    fn mentions_slot(
        &self,
        expression: &Expression<'a>,
        site: FunctionSite,
        slot: u32,
        depth: usize,
    ) -> bool {
        if depth > 32 {
            return false;
        }
        if self.model.site_parameter_slot(expression, site) == Some(slot) {
            return true;
        }
        let recurse = |inner: &Expression<'a>| self.mentions_slot(inner, site, slot, depth + 1);
        match expression.without_parentheses() {
            Expression::ConditionalExpression(conditional) => {
                recurse(&conditional.test)
                    || recurse(&conditional.consequent)
                    || recurse(&conditional.alternate)
            }
            Expression::LogicalExpression(logical) => {
                recurse(&logical.left) || recurse(&logical.right)
            }
            Expression::BinaryExpression(binary) => recurse(&binary.left) || recurse(&binary.right),
            Expression::UnaryExpression(unary) => recurse(&unary.argument),
            Expression::StaticMemberExpression(member) => recurse(&member.object),
            Expression::ComputedMemberExpression(member) => {
                recurse(&member.object) || recurse(&member.expression)
            }
            _ => false,
        }
    }

    /// True when `site` returns `this.<buffer>.slice(...)`, possibly chained
    /// (`.slice(...).concat(this.<buffer>.slice(...))`).
    fn returns_this_slice(&self, site: FunctionSite) -> bool {
        self.model.kind_nodes(oxc_ast::AstType::ReturnStatement).any(|node| {
            let AstKind::ReturnStatement(ret) = node.kind() else {
                return false;
            };
            if self.model.owning_site(node.id()).map(|owner| owner.span) != Some(site.span) {
                return false;
            }
            let mut current = ret.argument.as_ref().map(Expression::without_parentheses);
            while let Some(Expression::CallExpression(call)) = current {
                let callee = call.callee.without_parentheses();
                let object = member_object_expression(callee).map(Expression::without_parentheses);
                if static_member_name(callee) == Some("slice")
                    && object.is_some_and(|object| this_member_expression_name(object).is_some())
                {
                    return true;
                }
                current = object;
            }
            false
        })
    }

    /// True when `site` stores its first argument at `this.<buffer>[this.<cursor>]`
    /// and resets `this.<cursor> = 0`.
    fn overwrites_at_cursor(&self, site: FunctionSite) -> bool {
        let own = |node: &oxc_semantic::AstNode<'a>| {
            self.model.owning_site(node.id()).map(|owner| owner.span) == Some(site.span)
        };
        let mut cursors: BTreeSet<String> = BTreeSet::new();
        let mut resets: BTreeSet<String> = BTreeSet::new();
        for node in self.model.kind_nodes(oxc_ast::AstType::AssignmentExpression) {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            if !own(node) {
                continue;
            }
            if let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left {
                if this_member_expression_name(member.object.without_parentheses()).is_some()
                    && self.model.site_parameter_slot(&assignment.right, site) == Some(0)
                {
                    if let Some(cursor) =
                        this_member_expression_name(member.expression.without_parentheses())
                    {
                        cursors.insert(cursor.to_string());
                    }
                }
            }
            if let Some(name) = this_member_name(&assignment.left) {
                if numeric_value(&assignment.right) == Some(0.0) {
                    resets.insert(name);
                }
            }
        }
        cursors.intersection(&resets).next().is_some()
    }

    fn detect_default_filter(&mut self, _session_site: FunctionSite) -> Option<SymbolId> {
        let snapshot = self.detect_snapshot_getter()?;
        let mut candidates = Vec::new();
        for symbol in &self.model.main_symbols {
            let Some(initializer) = self.model.symbol_value(*symbol) else {
                continue;
            };
            let Some(values) = integer_sequence(initializer) else {
                continue;
            };
            if values.len() < 2 {
                continue;
            }
            let referenced = self
                .model
                .scoping()
                .get_resolved_reference_ids(*symbol)
                .iter()
                .any(|reference| {
                    let node_id = self.model.scoping().get_reference(*reference).node_id();
                    let span = self.model.nodes().get_node(node_id).span();
                    span.start >= snapshot.body_span.unwrap_or(snapshot.span).start
                        && span.end <= snapshot.body_span.unwrap_or(snapshot.span).end
                });
            if referenced {
                candidates.push(*symbol);
            }
        }
        if candidates.len() == 1 {
            Some(candidates[0])
        } else {
            None
        }
    }

    fn detect_snapshot_getter(&self) -> Option<FunctionSite> {
        let mut candidates = Vec::new();
        for site in self.model.all_sites().iter().copied() {
            let mut matched = false;
            for argument in self.returned_values(site) {
                let Some(expression) = self.resolve_object_expression(argument, 0) else {
                    continue;
                };
                let Some(members) = object_members(expression) else {
                    continue;
                };
                let keys: BTreeSet<&str> =
                    members.iter().map(|member| member.name.as_str()).collect();
                if keys.contains("signals")
                    && keys.contains("positionalSignals")
                    && keys.contains("keyedSignals")
                    && keys.contains("errors")
                {
                    matched = true;
                    break;
                }
            }
            if matched {
                candidates.push(site);
            }
        }
        match candidates.len() {
            0 => None,
            1 => {
                let site = candidates[0];
                Some(site)
            }
            _ => None,
        }
    }

    fn detect_hash_chain(
        &self,
        session_site: FunctionSite,
        context_local: Option<SymbolId>,
        context_binding: Option<&BindingRef>,
    ) -> Option<HashChain> {
        let hash_functions = self.detect_hash_functions();
        let mut selected = None;
        for node in self.model.nodes_within(session_site).into_iter() {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            if !self.model.node_within(node.id(), session_site) {
                continue;
            }
            let Some(target) = simple_assignment_symbol(&assignment.left, self.model.scoping())
            else {
                continue;
            };
            let Some(call) = self.resolve_call_expression(&assignment.right, 0) else {
                continue;
            };
            let Some(callee) = self.model.callee_function(&call.callee) else {
                continue;
            };
            if !hash_functions
                .iter()
                .any(|candidate| candidate.span.start == callee.span.start)
            {
                continue;
            }
            let Some(argument) = call_arguments(call).first().copied() else {
                continue;
            };
            let is_transaction = static_member_name(argument) == Some("transactionToken")
                || context_local.is_some_and(|local| {
                    crate::analysis::expression_symbol(argument, self.model.scoping())
                        == Some(local)
                });
            if is_transaction {
                selected = Some((target, callee, self.model.expr_record(argument)));
                break;
            }
        }
        let (state_symbol, hash, hash_input_record) = selected?;
        // A multi-thread capture carries one factory per thread; the chain only
        // needs one of them to consume this state binding.
        let prng_factories = self.detect_prng_factories();
        if !prng_factories.is_empty()
            && !prng_factories
                .iter()
                .any(|prng| self.prng_references_state(*prng, state_symbol))
        {
            return None;
        }
        let chain = HashChain {
            prng_state: self.model.binding_ref(state_symbol),
            hash_function: self.model.function_ref(hash),
            hash_input: hash_input_record,
            transaction_token_field: Some("transactionToken".to_string()),
            session_argument: None,
            public_init_argument: None,
        };
        if let Some(binding) = context_binding {
            let _ = binding;
        }
        Some(chain)
    }

    fn detect_hash_functions(&self) -> Vec<FunctionSite> {
        let mut candidates = Vec::new();
        for site in self.model.all_sites().iter().copied() {
            let has_shift_five = self.body_contains_shift(
                site,
                oxc_syntax::operator::BinaryOperator::ShiftLeft,
                5.0,
            );
            let has_view = self.body_contains_identifier(site, "ArrayBuffer")
                && self.body_contains_identifier(site, "Int32Array");
            if has_shift_five && has_view {
                candidates.push(site);
            }
        }
        candidates
    }

    fn prng_references_state(&self, prng: FunctionSite, state: SymbolId) -> bool {
        self.model
            .scoping()
            .get_resolved_reference_ids(state)
            .iter()
            .any(|reference| {
                let node_id = self.model.scoping().get_reference(*reference).node_id();
                let span = self.model.nodes().get_node(node_id).span();
                let body = prng.body_span.unwrap_or(prng.span);
                span.start >= body.start && span.end <= body.end
            })
    }

    // -----------------------------------------------------------------------
    // Helper detection
    // -----------------------------------------------------------------------

    fn detect_helpers(
        &mut self,
        _descriptors: Option<&DescriptorRegistry>,
        _waves: Option<&WaveRegistry>,
        session: Option<&SessionProperties>,
    ) -> Vec<(BindingRole, FunctionSite, Confidence, Vec<String>)> {
        let mut helpers = Vec::new();
        // One record per proven copy. A capture that traces several threads
        // repeats each encoder helper once per thread; the records share a role
        // and are told apart by their resolved binding, never by position.
        for site in self.detect_prng_factories() {
            helpers.push((
                BindingRole::CreateSignalMask,
                site,
                Confidence::High,
                vec!["seeded closure state, discarded warm-up calls and byte cycles".to_string()],
            ));
        }
        for site in self.detect_number_writers() {
            helpers.push((
                BindingRole::WriteNumber,
                site,
                Confidence::High,
                vec!["buffer writes masked by a PRNG call".to_string()],
            ));
        }
        for site in self.detect_string_writers() {
            helpers.push((
                BindingRole::WriteString,
                site,
                Confidence::High,
                vec!["interning factory returning a masked UTF-8 writer".to_string()],
            ));
        }
        if let Some(site) = self.detect_observer_factory() {
            helpers.push((
                BindingRole::ObserverFactory,
                site,
                Confidence::High,
                vec!["returns next/error/complete sinks".to_string()],
            ));
        }
        if let Some(site) = self.detect_snapshot_getter() {
            helpers.push((
                BindingRole::SnapshotGetter,
                site,
                Confidence::High,
                vec!["options-taking getter returning the snapshot partitions".to_string()],
            ));
        }
        if let Some(chain) = session.and_then(|session| session.hash_chain.as_ref()) {
            if let Some(site) = self.model.site_by_span(Span::new(
                chain.hash_function.span.start,
                chain.hash_function.span.end,
            )) {
                helpers.push((
                    BindingRole::HashTransactionToken,
                    site,
                    Confidence::High,
                    vec!["32-bit string/view hash used for the mask session state".to_string()],
                ));
            }
        }
        helpers
    }

    fn detect_prng_factories(&self) -> Vec<FunctionSite> {
        self.prng_factories
            .get_or_init(|| self.find_prng_factories())
            .clone()
    }

    fn find_prng_factories(&self) -> Vec<FunctionSite> {
        // The `% 4` and `<< n (n >= 9)` nodes of the program, once: a site
        // has the PRNG cycle when its body holds one of each.
        let mut mod_four = Vec::new();
        let mut shifts = Vec::new();
        for node in self.model.kind_nodes(oxc_ast::AstType::BinaryExpression) {
            let AstKind::BinaryExpression(binary) = node.kind() else {
                continue;
            };
            if binary.operator == oxc_syntax::operator::BinaryOperator::Remainder
                && numeric_value(&binary.right).is_some_and(|value| (value - 4.0).abs() < 1e-9)
            {
                mod_four.push(node.span());
            }
            if binary.operator == oxc_syntax::operator::BinaryOperator::ShiftLeft
                && numeric_value(&binary.right).is_some_and(|value| value >= 9.0)
            {
                shifts.push(node.span());
            }
        }
        let has_cycle = |site: FunctionSite| -> bool {
            if site.body_span.is_none() {
                return false;
            }
            let body = site.body_span.unwrap_or(site.span);
            let inside = |span: &Span| span.start >= body.start && span.end <= body.end;
            mod_four.iter().any(inside) && shifts.iter().any(inside)
        };
        let sites = self.model.all_sites();
        let cycles: Vec<bool> = sites.iter().map(|site| has_cycle(*site)).collect();
        let mut candidates = Vec::new();
        for site in sites.iter().copied() {
            let Some(body) = site.body_span else {
                continue;
            };
            let nested: Vec<_> = sites
                .iter()
                .zip(&cycles)
                .filter(|(candidate, cycle)| {
                    candidate.span.start >= body.start
                        && candidate.span.end <= body.end
                        && candidate.span != site.span
                        && **cycle
                })
                .map(|(candidate, _)| *candidate)
                .collect();
            if nested.len() != 1 {
                continue;
            }
            let inner = nested[0];
            if !self.returns_function(site, inner) {
                continue;
            }
            let calls = self
                .model
                .nodes()
                .iter()
                .filter(|node| {
                    let AstKind::CallExpression(call) = node.kind() else {
                        return false;
                    };
                    self.model.node_within(node.id(), site)
                        && self
                            .model
                            .callee_function(&call.callee)
                            .map(|target| target.span.start)
                            == Some(inner.span.start)
                })
                .count();
            if calls < 3 {
                continue;
            }
            candidates.push(site);
        }
        // Every copy is a proven PRNG factory. A two-thread capture repeats the
        // helper once per thread; the use site's resolved binding picks which
        // copy a mask call belongs to, so listing all of them is exact.
        candidates
    }

    fn returns_function(&self, outer: FunctionSite, inner: FunctionSite) -> bool {
        let mut found = false;
        for argument in self.returned_values(outer) {
            if self
                .model
                .function_from_expression(argument)
                .map(|target| target.span.start)
                == Some(inner.span.start)
            {
                found = true;
            }
        }
        found
    }

    /// The number writer, recognised by its own parameter-slot dataflow in
    /// either capture representation: masked writes `slot2[i] = v ^ slot0()`
    /// into the buffer slot, the integer test `(slot1 | 0) === slot1` on the
    /// value slot, and no returned closure (that would be a writer factory).
    /// Slots must belong to the site itself, so neither an enclosing nor a
    /// nested function inherits the shape.
    ///
    /// Equal-shaped candidates are told apart by their consumers: the writer
    /// threads its returned position back into its own position slot
    /// (`p = write(mask, value, buffer, p)`). Write counts never decide; a tie
    /// that dataflow cannot break stays an unresolved fact.
    fn detect_number_writers(&mut self) -> Vec<FunctionSite> {
        let mut candidates = Vec::new();
        for site in self.model.all_sites().iter().copied() {
            let mut masked_writes = 0usize;
            let mut integer_test = false;
            for node in self.model.nodes_within(site).into_iter() {
                if !self.model.node_within(node.id(), site) {
                    continue;
                }
                match node.kind() {
                    AstKind::AssignmentExpression(assignment) => {
                        if self.is_buffer_write(&assignment.left, 2, site)
                            && self.is_masked_value(&assignment.right, site)
                        {
                            masked_writes += 1;
                        }
                    }
                    AstKind::BinaryExpression(binary) => {
                        if self.is_integer_test(binary, 1, site) {
                            integer_test = true;
                        }
                    }
                    _ => {}
                }
            }
            let returns_closure = self
                .returned_values(site)
                .into_iter()
                .any(|argument| self.model.function_from_expression(argument).is_some());
            // One masked write is enough when the value slot is tested for
            // integrality and no closure is returned: a writer that advances its
            // position with `buf[++i] = … ^ mask()` records one plain indexed
            // write, so a two-write floor rejects that build outright.
            if masked_writes >= 1 && integer_test && !returns_closure {
                candidates.push(site);
            }
        }
        match candidates.as_slice() {
            [] => return Vec::new(),
            [site] => return vec![*site],
            _ => {}
        }
        // More than one function has the shape. A capture that traces several
        // threads carries one copy of the helper per thread, so every copy that
        // threads its own position is a proven NUMBER writer. Identity stays
        // per use site: a write is NUMBER only when its callee resolves to one
        // of these bindings, so a copy never types another copy's writes.
        // Every shape match is a copy of the writer. A capture may carry a copy
        // per thread, and a copy need not thread its position through the
        // caller's chain (`w = write(mask, value, buf, pos)`), so filtering to
        // threading candidates silently dropped a whole copy: its call sites
        // then looked like unmodelled calls and their buffers lost their wires.
        if candidates.iter().any(|site| self.threads_position(*site)) {
            return candidates;
        }
        self.unresolved.push(UnresolvedFact {
            fact: "number_writer".to_string(),
            reason: format!(
                "{} functions share the masked number-writer shape and none threads its position",
                candidates.len()
            ),
            span: None,
            required: true,
        });
        Vec::new()
    }

    /// `target` is `slot[...]` for parameter `slot` of `site`.
    fn is_buffer_write(
        &self,
        target: &AssignmentTarget<'a>,
        slot: u32,
        site: FunctionSite,
    ) -> bool {
        let AssignmentTarget::ComputedMemberExpression(member) = target else {
            return false;
        };
        self.model.site_slot_value(&member.object, site) == Some(slot)
    }

    /// `value ^ mask()` (either operand order), where `mask` is parameter
    /// slot 0 of `site`, called directly or through `.call(...)`.
    fn is_masked_value(&self, expression: &Expression<'a>, site: FunctionSite) -> bool {
        let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
            return false;
        };
        binary.operator == oxc_syntax::operator::BinaryOperator::BitwiseXOR
            && (self.is_slot_call(&binary.left, 0, site)
                || self.is_slot_call(&binary.right, 0, site))
    }

    fn is_slot_call(&self, expression: &Expression<'a>, slot: u32, site: FunctionSite) -> bool {
        let Expression::CallExpression(call) = expression.without_parentheses() else {
            return false;
        };
        if self.model.site_slot_value(&call.callee, site) == Some(slot) {
            return true;
        }
        let callee = call.callee.without_parentheses();
        static_member_name(callee) == Some("call")
            && member_object_expression(callee)
                .is_some_and(|object| self.model.site_slot_value(object, site) == Some(slot))
    }

    /// `(slot | 0) === slot` for parameter `slot` of `site`.
    fn is_integer_test(
        &self,
        binary: &BinaryExpression<'a>,
        slot: u32,
        site: FunctionSite,
    ) -> bool {
        // The writer pair tests `(value | 0) === value` on its integer path and
        // `(value | 0) !== value` on its float path; both are the same predicate
        // seen from either side, and a capture may keep only one copy per thread.
        if !matches!(
            binary.operator,
            oxc_syntax::operator::BinaryOperator::StrictEquality
                | oxc_syntax::operator::BinaryOperator::StrictInequality
                | oxc_syntax::operator::BinaryOperator::Equality
                | oxc_syntax::operator::BinaryOperator::Inequality
        ) {
            return false;
        }
        let Expression::BinaryExpression(left) = binary.left.without_parentheses() else {
            return false;
        };
        if left.operator != oxc_syntax::operator::BinaryOperator::BitwiseOR {
            return false;
        }
        let is_slot = |expression: &Expression<'a>| {
            self.model.site_slot_value(expression, site) == Some(slot)
        };
        // The compared side may also be a local declared from the slot
        // (`var v = arguments[1]; (arguments[1] | 0) === v`); the truncated
        // side must be the slot itself.
        let is_compared = |expression: &Expression<'a>| {
            is_slot(expression)
                || crate::analysis::expression_symbol(expression, self.model.scoping())
                    .and_then(|symbol| self.model.initializer(symbol))
                    .is_some_and(|value| self.model.site_parameter_slot(value, site) == Some(slot))
        };
        let is_zero = |expression: &Expression<'a>| numeric_value(expression) == Some(0.0);
        ((is_slot(&left.left) && is_zero(&left.right))
            || (is_zero(&left.left) && is_slot(&left.right)))
            && is_compared(&binary.right)
    }

    /// Some call of `site` receives, as its position argument (slot 3), the
    /// position returned by another call of `site`: either directly nested or
    /// through a binding assigned from such a call.
    fn threads_position(&self, site: FunctionSite) -> bool {
        let calls = self.model.calls_to(site);
        let call_spans: BTreeSet<(u32, u32)> = calls
            .iter()
            .map(|call| (call.span.start, call.span.end))
            .collect();
        let mut results = BTreeSet::new();
        for call in &calls {
            let node_id = call.node_id.get();
            let parent = self
                .model
                .nodes()
                .get_node(self.model.nodes().parent_id(node_id));
            let target = match parent.kind() {
                AstKind::VariableDeclarator(declarator) => binding_pattern_symbol(&declarator.id),
                AstKind::AssignmentExpression(assignment) => {
                    simple_assignment_symbol(&assignment.left, self.model.scoping())
                }
                _ => None,
            };
            results.extend(target);
        }
        calls.iter().any(|call| {
            let Some(position) = call_arguments(call).get(3).copied() else {
                return false;
            };
            match position.without_parentheses() {
                Expression::CallExpression(inner) => {
                    call_spans.contains(&(inner.span.start, inner.span.end))
                }
                Expression::Identifier(identifier) => {
                    crate::analysis::identifier_symbol(identifier, self.model.scoping())
                        .is_some_and(|symbol| results.contains(&symbol))
                }
                _ => false,
            }
        })
    }

    fn detect_string_writers(&self) -> Vec<FunctionSite> {
        let mut candidates = Vec::new();
        for site in self.model.all_sites().iter().copied() {
            let mut creates_null = false;
            for node in self.model.nodes_within(site).into_iter() {
                if !self.model.node_within(node.id(), site) {
                    continue;
                }
                if self
                    .model
                    .enclosing_function(node.id())
                    .map(|f| f.span.start)
                    != Some(site.span.start)
                {
                    continue;
                }
                let AstKind::CallExpression(call) = node.kind() else {
                    continue;
                };
                if static_member_name(&call.callee) != Some("create") {
                    continue;
                }
                let object = match &call.callee {
                    Expression::StaticMemberExpression(member) => Some(&member.object),
                    Expression::ComputedMemberExpression(member) => Some(&member.object),
                    _ => None,
                };
                if object.is_some_and(|object| {
                    matches!(object, Expression::Identifier(identifier) if identifier.name == "Object")
                }) && call_arguments(call)
                    .first()
                    .is_some_and(|arg| matches!(literal_value(arg), Some(LiteralValue::Null)))
                {
                    creates_null = true;
                }
            }
            if !creates_null {
                continue;
            }
            let returns_function = self.returned_values(site).into_iter().any(|argument| {
                self.model
                    .function_from_expression(argument)
                    .is_some_and(|target| self.model.node_within(target.node_id, site))
            });
            if returns_function {
                candidates.push(site);
            }
        }
        // As with the NUMBER writer, a second copy is another thread's copy of
        // the same factory, not a competing candidate. Each use site resolves
        // to the binding it actually captured.
        candidates
    }

    fn detect_observer_factory(&self) -> Option<FunctionSite> {
        let mut candidates = Vec::new();
        for site in self.model.all_sites().iter().copied() {
            if self
                .returns_object_with_keys(site, &["next", "error", "complete"])
                .iter()
                .any(|keys| keys.len() == 3)
            {
                candidates.push(site);
                continue;
            }
            // Incremental form: an object literal is returned by identifier and
            // the three sink methods are assigned before the return.
            for argument in self.returned_values(site) {
                let Expression::Identifier(identifier) = argument else {
                    continue;
                };
                let Some(symbol) =
                    crate::analysis::identifier_symbol(identifier, self.model.scoping())
                else {
                    continue;
                };
                let keys = self.object_binding_keys(symbol, site);
                if keys.len() == 3
                    && keys.contains("next")
                    && keys.contains("error")
                    && keys.contains("complete")
                {
                    candidates.push(site);
                    break;
                }
            }
        }
        if candidates.len() == 1 {
            Some(candidates[0])
        } else {
            None
        }
    }

    // -----------------------------------------------------------------------
    // Serializer/envelope
    // -----------------------------------------------------------------------

    fn detect_envelope_builder(&self) -> Option<FunctionSite> {
        const FIELDS: [&str; 6] = ["f", "b", "c", "d", "z", "a"];
        let mut candidates = Vec::new();
        for site in self.model.all_sites().iter().copied() {
            if !self.body_names_key(site, "signalFilter") {
                continue;
            }
            let mut by_object: HashMap<SymbolId, BTreeSet<String>> = HashMap::new();
            for node in self.model.nodes_within(site).into_iter() {
                if !self.model.node_within(node.id(), site) {
                    continue;
                }
                if self
                    .model
                    .enclosing_function(node.id())
                    .map(|f| f.span.start)
                    != Some(site.span.start)
                {
                    continue;
                }
                let AstKind::AssignmentExpression(assignment) = node.kind() else {
                    continue;
                };
                let Some((object, key)) = member_target_key(&assignment.left) else {
                    continue;
                };
                let Some(object) = crate::analysis::expression_symbol(object, self.model.scoping())
                else {
                    continue;
                };
                if FIELDS.contains(&key.as_str()) {
                    by_object.entry(object).or_default().insert(key);
                }
            }
            if by_object.values().any(|keys| keys.len() >= 4) {
                candidates.push(site);
            }
        }
        if candidates.len() == 1 {
            Some(candidates[0])
        } else {
            None
        }
    }

    fn default_encoder_symbol(&self, envelope: FunctionSite) -> Option<SymbolId> {
        let mut candidates = Vec::new();
        for node in self.model.nodes_within(envelope).into_iter() {
            let AstKind::ConditionalExpression(conditional) = node.kind() else {
                continue;
            };
            if !self.model.node_within(node.id(), envelope) {
                continue;
            }
            if !self.body_names_key(envelope, "encode") {
                continue;
            }
            if let Expression::Identifier(identifier) = &conditional.alternate {
                if let Some(symbol) =
                    crate::analysis::identifier_symbol(identifier, self.model.scoping())
                {
                    candidates.push(symbol);
                }
            }
        }
        candidates.sort_by_key(|symbol| symbol.index());
        candidates.dedup();
        if candidates.len() == 1 {
            Some(candidates[0])
        } else {
            None
        }
    }

    fn build_envelope(&mut self, site: FunctionSite) -> EnvelopeProperties {
        const FIELDS: [&str; 6] = ["f", "b", "c", "d", "z", "a"];
        let mut fields = BTreeMap::new();
        let (header_prefix, header_chunk_size) = self.dispatch_header();
        for node in self.model.nodes_within(site).into_iter() {
            if !self.model.node_within(node.id(), site) {
                continue;
            }
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let Some((_, key)) = member_target_key(&assignment.left) else {
                continue;
            };
            if !FIELDS.contains(&key.as_str()) {
                continue;
            }
            let record = self.model.expr_record(&assignment.right);
            fields.entry(key.to_string()).or_insert(record);
        }
        let mut function = self.model.function_ref(site);
        function.source = self.retain_function_body(site, BindingRole::EnvelopeBuilder);
        EnvelopeProperties {
            builder: function,
            fields,
            header_prefix,
            header_chunk_size,
        }
    }

    /// Read the header name prefix and chunk size from the dispatch-detail
    /// literal object. The object is located structurally: it is a two-field
    /// `{<string>, <number>}` object nested under a `config` or `detail`
    /// property. Values are never defaulted; a missing literal yields `None`.
    fn dispatch_header(&self) -> (Option<LiteralValue>, Option<LiteralValue>) {
        for node in self.model.kind_nodes(oxc_ast::AstType::ObjectExpression) {
            let AstKind::ObjectExpression(object) = node.kind() else {
                continue;
            };
            for member in object_members_of(object) {
                if member.name != "config" && member.name != "detail" {
                    continue;
                }
                let Expression::ObjectExpression(inner) = member.value else {
                    continue;
                };
                let members = object_members_of(inner);
                if members.len() != 2 {
                    continue;
                }
                let mut prefix = None;
                let mut chunk = None;
                for field in &members {
                    match literal_value(field.value) {
                        Some(value @ LiteralValue::String(_)) => prefix = Some(value),
                        Some(value @ LiteralValue::Number(_)) => chunk = Some(value),
                        _ => {}
                    }
                }
                if let (Some(prefix), Some(chunk)) = (prefix, chunk) {
                    return (Some(prefix), Some(chunk));
                }
            }
        }
        (None, None)
    }

    fn build_serializer(
        &mut self,
        envelope: Option<FunctionSite>,
        default_encoder: Option<SymbolId>,
        session: Option<&SessionProperties>,
        _descriptors: Option<&DescriptorRegistry>,
    ) -> Result<Option<SerializerProperties>, ProgramPropertiesError> {
        let Some(default_encoder) = default_encoder else {
            return Ok(None);
        };
        let mut factory = None;
        let mut factory_arguments = Vec::new();
        for node in self.model.kind_nodes(oxc_ast::AstType::AssignmentExpression) {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let Some(target) = simple_assignment_symbol(&assignment.left, self.model.scoping())
            else {
                continue;
            };
            if target != default_encoder {
                continue;
            }
            let Some(call) = self.resolve_call_expression(&assignment.right, 0) else {
                continue;
            };
            if let Some(callee) = self.model.callee_function(&call.callee) {
                factory = Some(callee);
                factory_arguments = call_arguments(call)
                    .into_iter()
                    .map(|argument| self.model.expr_record(argument))
                    .collect();
                break;
            }
        }
        let mut serializer = SerializerProperties {
            default_encoder: Some(self.model.binding_ref(default_encoder)),
            factory: factory.map(|site| self.model.function_ref(site)),
            factory_arguments,
            alphabet_fallback: None,
            keysets: None,
        };
        if let Some(site) = factory {
            self.assign_role_for_site(site, BindingRole::SerializerFactory);
            serializer.alphabet_fallback = self.find_alphabet_fallback(site);
            if let Some(session) = session {
                let _ = session;
            }
            // The keyset list is the argument whose expression is a `.map`
            // call over an empty array or a context list; retain it exactly.
            serializer.keysets = serializer
                .factory_arguments
                .iter()
                .find(|argument| argument.source.contains(".map("))
                .cloned();
        }
        if let Some(envelope) = envelope {
            let _ = envelope;
        }
        Ok(Some(serializer))
    }

    fn find_alphabet_fallback(&self, factory: FunctionSite) -> Option<ExprRecord> {
        for node in self.model.nodes_within(factory).into_iter() {
            let AstKind::LogicalExpression(logical) = node.kind() else {
                continue;
            };
            if logical.operator != oxc_syntax::operator::LogicalOperator::Coalesce {
                continue;
            }
            if !self.model.node_within(node.id(), factory) {
                continue;
            }
            if matches!(logical.right, Expression::StringLiteral(_)) {
                return Some(self.model.expr_record(&logical.right));
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // Signals/emissions
    // -----------------------------------------------------------------------

    fn build_signals(
        &mut self,
        descriptors: Option<&DescriptorRegistry>,
        waves: Option<&WaveRegistry>,
    ) -> Result<Vec<SignalRecord>, ProgramPropertiesError> {
        let Some(waves) = waves else {
            return Ok(Vec::new());
        };
        let mut by_signal: BTreeMap<u32, SignalRecord> = BTreeMap::new();
        let descriptor_length = descriptors.map_or(0, |registry| registry.length);

        for wave in &waves.waves {
            let Some(site) = self
                .model
                .site_by_span(Span::new(wave.function_span.start, wave.function_span.end))
            else {
                continue;
            };
            let emissions = self.collector_emissions(site);
            for (slot, signal) in wave.signal_indices.iter().enumerate() {
                let record = by_signal.entry(*signal as u32).or_insert_with(|| SignalRecord {
                    signal: *signal as u32,
                    descriptor_index: *signal as u32,
                    direct: false,
                    wave: Some(wave.index as u32),
                    wave_position: Some(slot as u32),
                    collector: Some(wave.function.clone()),
                    observer: ObserverRoute::WaveSlot(slot as u32),
                    emissions: Vec::new(),
                    emission_layouts: Vec::new(),
                    wire_layout: WireLayoutStatus::Unresolved(
                        "ordered writes, encoder, PRNG/mask and seed chain are owned by the per-signal pass"
                            .to_string(),
                    ),
                    unresolved: Vec::new(),
                });
                for (route, emission) in &emissions {
                    if *route == ObserverRoute::WaveSlot(slot as u32) {
                        record.emissions.push(emission.clone());
                    }
                }
            }
        }

        for slot in &waves.direct_slots {
            by_signal
                .entry(slot.signal as u32)
                .or_insert_with(|| SignalRecord {
                    signal: slot.signal as u32,
                    descriptor_index: slot.descriptor_index as u32,
                    direct: true,
                    wave: None,
                    wave_position: None,
                    collector: slot.ctor_call.clone(),
                    observer: ObserverRoute::Direct,
                    emissions: Vec::new(),
                    emission_layouts: Vec::new(),
                    wire_layout: if slot.declared_not_emitted {
                        WireLayoutStatus::DeclaredNotEmitted(
                            "the task callback completes without ever mentioning an observer next; the value does not exist in this capture"
                                .to_string(),
                        )
                    } else {
                        WireLayoutStatus::Unresolved(
                            "direct-constructor collector and wire layout are not traced by this pass"
                                .to_string(),
                        )
                    },
                    unresolved: slot.unresolved.clone(),
                });
        }

        if descriptor_length > 0 {
            for signal in 0..descriptor_length {
                if !by_signal.contains_key(&(signal as u32)) {
                    self.unresolved.push(UnresolvedFact {
                        fact: "signal_route".to_string(),
                        reason: format!(
                            "descriptor index {signal} is not reached by a wave row or direct constructor"
                        ),
                        span: None,
                        required: true,
                    });
                }
            }
        }

        if by_signal.len() > self.limits.max_emissions {
            return Err(ProgramPropertiesError::Limit {
                what: "signal records",
                limit: self.limits.max_emissions,
            });
        }
        Ok(by_signal.into_values().collect())
    }

    fn collector_emissions(&self, site: FunctionSite) -> Vec<(ObserverRoute, EmissionRecord)> {
        let aliases = self.observer_aliases(site);
        let mut emissions = Vec::new();
        let mut seen = BTreeSet::new();
        for node in self.model.nodes_within(site).into_iter() {
            if !self.model.node_within(node.id(), site) {
                continue;
            }
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            let Some((kind, observer_expression)) = observer_sink_call(call) else {
                continue;
            };
            let Some(route) = self.observer_route(observer_expression, &aliases, site) else {
                continue;
            };
            if !seen.insert((call.span.start, call.span.end)) {
                continue;
            }
            if emissions.len() >= self.limits.max_emissions {
                break;
            }
            let arguments = call_arguments(call);
            let payload = arguments
                .first()
                .map(|argument| self.model.expr_record(argument));
            let buffer = arguments
                .first()
                .and_then(|argument| emitted_buffer(argument))
                .map(|expression| self.model.expr_record(expression));
            emissions.push((
                route,
                EmissionRecord {
                    kind,
                    span: SourceSpan::from(call.span),
                    observer: route,
                    payload,
                    buffer,
                },
            ));
        }
        emissions
    }

    fn observer_aliases(&self, site: FunctionSite) -> HashMap<SymbolId, ObserverRoute> {
        let mut aliases = HashMap::new();
        // Two passes let an alias of an alias resolve without following writes
        // outside the collector body.
        for _ in 0..3 {
            for node in self.model.nodes_within(site).into_iter() {
                if !self.model.node_within(node.id(), site) {
                    continue;
                }
                match node.kind() {
                    AstKind::VariableDeclarator(declarator) => {
                        let Some(init) = declarator.init.as_ref() else {
                            continue;
                        };
                        let Some(symbol) = binding_pattern_symbol(&declarator.id) else {
                            continue;
                        };
                        if let Some(route) = self.observer_route(init, &aliases, site) {
                            aliases.entry(symbol).or_insert(route);
                        }
                    }
                    AstKind::AssignmentExpression(assignment) => {
                        let AssignmentTarget::AssignmentTargetIdentifier(identifier) =
                            &assignment.left
                        else {
                            continue;
                        };
                        let Some(symbol) =
                            crate::analysis::identifier_symbol(identifier, self.model.scoping())
                        else {
                            continue;
                        };
                        if let Some(route) = self.observer_route(&assignment.right, &aliases, site)
                        {
                            aliases.entry(symbol).or_insert(route);
                        }
                    }
                    _ => {}
                }
            }
        }
        aliases
    }

    /// The observer a collector-body expression names: the collector's own
    /// parameter slot 0 (a direct observer) or a numeric element of it (a
    /// wave slot), directly or through a local alias.
    fn observer_route(
        &self,
        expression: &Expression<'a>,
        aliases: &HashMap<SymbolId, ObserverRoute>,
        site: FunctionSite,
    ) -> Option<ObserverRoute> {
        if self.model.site_parameter_slot(expression, site) == Some(0) {
            return Some(ObserverRoute::Direct);
        }
        if let Expression::ComputedMemberExpression(member) = expression {
            if self.model.site_parameter_slot(&member.object, site) == Some(0) {
                return numeric_index(&member.expression).map(ObserverRoute::WaveSlot);
            }
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let symbol = crate::analysis::identifier_symbol(identifier, self.model.scoping())?;
        aliases.get(&symbol).copied()
    }

    /// Connect the per-signal wire extractor to the core registry/wave facts.
    ///
    /// Wave collectors use their own function span plus the row-derived
    /// `slot -> signal` mapping. A direct registry slot uses the *task callback*
    /// passed to the constructor wrapper, because that is the function whose
    /// body owns the emissions. The extractor reparses the exact source; every
    /// fact it returns is an exact source slice or a resolved binding.
    fn apply_signal_layouts(
        &mut self,
        helpers: &[HelperRecord],
        signals: &mut [SignalRecord],
        waves: Option<&WaveRegistry>,
        session: Option<&SessionProperties>,
    ) -> Vec<StaticCapture> {
        let Some(waves) = waves else {
            return Vec::new();
        };
        // Every binding proven for a role, not just the first. The extractor
        // resolves each write to the callee's own lexical binding, so the copies
        // that a multi-thread capture carries each type their own writes.
        let helper_spans = |role: BindingRole| {
            helpers
                .iter()
                .filter(|helper| helper.role == role)
                .filter_map(|helper| helper.function.binding.as_ref())
                .map(|binding| Span::new(binding.identifier.start, binding.identifier.end))
                .collect::<Vec<_>>()
        };
        let helpers_request = crate::signals::extract::layout::HelperBindings {
            number_writer: helper_spans(BindingRole::WriteNumber),
            string_writer: helper_spans(BindingRole::WriteString),
            prng_factory: helper_spans(BindingRole::CreateSignalMask),
            hash: helpers
                .iter()
                .find(|helper| helper.role == BindingRole::HashTransactionToken)
                .and_then(|helper| helper.function.binding.as_ref())
                .map(|binding| Span::new(binding.identifier.start, binding.identifier.end)),
        };
        // A launcher half that forwards a VM worker reply has no local wire: the
        // bytes are built by the worker function the launch message names.
        // Detect that pair here and scan the worker instead of the forwarder.
        //
        // The join uses the tracer's link naming (`$THREAD_<thread>_<pc>`), which
        // is the only place the message's two numbers meet a function. If the
        // name is absent nothing is claimed and the signal stays unresolved.
        // A launcher half that forwards a VM worker reply has no local wire: the
        // bytes are built by the worker function the launch message names.
        // Detect that pair here and scan the worker instead of the forwarder.
        //
        // The join uses the tracer's link naming (`$THREAD_<thread>_<pc>`), which
        // is the only place the message's two numbers meet a function. If the
        // name is absent nothing is claimed and the signal stays unresolved.
        let signal_collector: BTreeMap<u32, Span> = waves
            .waves
            .iter()
            .flat_map(|wave| {
                let span = Span::new(wave.function_span.start, wave.function_span.end);
                wave.observer_slots
                    .iter()
                    .map(move |slot| (slot.signal, span))
            })
            .collect();
        // A direct slot's emissions live in its task callback, the function
        // its collector request scans below; the forwarding check reads the
        // same function (the constructor wrapper never forwards).
        let direct_collector: BTreeMap<u32, Span> = waves
            .direct_slots
            .iter()
            .filter_map(|slot| {
                let function = direct_task_function(self.model, slot)?;
                Some((slot.signal as u32, Span::new(function.start, function.end)))
            })
            .collect();
        let mut forwarding: BTreeSet<u32> = BTreeSet::new();
        let mut forwarded: BTreeMap<u32, String> = BTreeMap::new();
        let mut worker_requests: Vec<(u32, Span)> = Vec::new();
        for record in signals.iter() {
            let span = signal_collector
                .get(&record.signal)
                .or_else(|| direct_collector.get(&record.signal))
                .copied()
                .or_else(|| {
                    record
                        .collector
                        .as_ref()
                        .map(|collector| Span::new(collector.span.start, collector.span.end))
                });
            let Some(span) = span else {
                continue;
            };
            let text = &self.model.source[span.start as usize..span.end as usize];
            if !text.contains("vmWorkerCallbacks.push") {
                continue;
            }
            // Only the signal that forwards the worker's reply parameter is a
            // worker signal; its siblings in the same collector build their
            // payloads locally.
            let Some(parameter) = forwarded_parameter(text, span.start, record) else {
                continue;
            };
            let Some((thread, pc)) = posted_worker_target(text) else {
                continue;
            };
            let name = format!("$THREAD_{thread}_{pc}");
            let Some(site) = self.worker_function(&name) else {
                continue;
            };
            forwarding.insert(span.start);
            forwarded.insert(span.start, parameter);
            worker_requests.push((record.signal, Span::new(site.start, site.end)));
        }

        let mut collectors = Vec::new();
        for wave in &waves.waves {
            collectors.push(crate::signals::extract::layout::CollectorRequest {
                function: Span::new(wave.function_span.start, wave.function_span.end),
                site: if forwarding.contains(&wave.function_span.start) {
                    crate::signals::extract::layout::EmissionSite::WorkerForward
                } else {
                    crate::signals::extract::layout::EmissionSite::ObserverNext
                },
                forwarded_parameter: forwarded.get(&wave.function_span.start).cloned(),
                observers: crate::signals::extract::layout::ObserverWiring::WaveSlots(
                    wave.observer_slots
                        .iter()
                        .map(|slot| crate::signals::extract::layout::ObserverSlot {
                            slot: slot.slot,
                            signal: slot.signal,
                        })
                        .collect(),
                ),
            });
        }
        for slot in &waves.direct_slots {
            // A constructor that names a function directly (no wrapper call) is
            // its own collector. Once a wrapper call is present only its task
            // callback owns the emissions; the wrapper factory never stands in.
            let resolution = resolve_descriptor_ctor(self.model, &slot.ctor);
            let function = direct_task_function(self.model, slot);
            let Some(function) = function else {
                let fact = format!("signal_{}_direct_callback", slot.signal);
                let reasons = if resolution.refusals.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", resolution.refusals.join("; "))
                };
                self.diagnostics.push(Diagnostic {
                    code: "signal_layout_direct_callback".to_string(),
                    message: format!(
                        "direct signal {} has no resolvable task callback; its wire layout stays unresolved{reasons}",
                        slot.signal
                    ),
                    span: Some(slot.ctor.span),
                });
                self.unresolved.push(UnresolvedFact {
                    fact,
                    reason: format!(
                        "direct constructor has no source-resolved task callback{reasons}"
                    ),
                    span: Some(slot.ctor.span),
                    required: true,
                });
                continue;
            };
            collectors.push(crate::signals::extract::layout::CollectorRequest {
                function: Span::new(function.start, function.end),
                site: if forwarding.contains(&function.start) {
                    crate::signals::extract::layout::EmissionSite::WorkerForward
                } else {
                    crate::signals::extract::layout::EmissionSite::ObserverNext
                },
                forwarded_parameter: forwarded.get(&function.start).cloned(),
                observers: crate::signals::extract::layout::ObserverWiring::Direct {
                    signal: slot.signal as u32,
                },
            });
        }

        for (signal, span) in worker_requests {
            collectors.push(crate::signals::extract::layout::CollectorRequest {
                function: span,
                observers: crate::signals::extract::layout::ObserverWiring::Direct { signal },
                site: crate::signals::extract::layout::EmissionSite::WorkerReply,
                forwarded_parameter: None,
            });
        }

        let request = crate::signals::extract::layout::SignalLayoutRequest {
            source: self.model.source,
            helpers: helpers_request,
            collectors,
            limits: crate::signals::extract::layout::SignalLayoutLimits {
                max_source_bytes: self.limits.max_source_bytes,
                max_collectors: self.limits.max_functions,
                max_emissions: self.limits.max_emissions,
                ..crate::signals::extract::layout::SignalLayoutLimits::default()
            },
        };
        let report = match crate::signals::extract::layout::extract_signal_layouts(&request) {
            Ok(report) => report,
            Err(error) => {
                self.diagnostics.push(Diagnostic {
                    code: "signal_layout_failed".to_string(),
                    message: format!("per-signal wire extraction failed: {error}"),
                    span: None,
                });
                self.unresolved.push(UnresolvedFact {
                    fact: "signal_wire_layouts".to_string(),
                    reason: format!("per-signal wire extraction failed: {error}"),
                    span: None,
                    required: true,
                });
                return Vec::new();
            }
        };

        let session_state = session
            .and_then(|session| session.hash_chain.as_ref())
            .map(|chain| chain.prng_state.name.clone());
        let mut by_signal: BTreeMap<u32, Vec<crate::signals::extract::layout::Emission>> = BTreeMap::new();
        for layout in report.signals {
            for mut emission in layout.emissions {
                emission.session_state = session_state.clone();
                by_signal.entry(emission.signal).or_default().push(emission);
            }
        }
        // Every extractor-level unresolved fact is a generator-required gap,
        // not a harmless note: the status must not read as complete while a
        // write, payload, prefix, mask or encoding rule is unproven.
        for unresolved in &report.unresolved {
            self.diagnostics.push(Diagnostic {
                code: format!("signal_layout_{}", unresolved.reason),
                message: unresolved.detail.clone(),
                span: Some(SourceSpan::from(unresolved.span)),
            });
            self.unresolved.push(UnresolvedFact {
                fact: format!("signal_wire_{}", unresolved.reason),
                reason: unresolved.detail.clone(),
                span: Some(SourceSpan::from(unresolved.span)),
                required: true,
            });
        }

        let mut captures = Vec::new();
        let mut seen_captures = BTreeSet::new();
        for record in signals.iter_mut() {
            let layouts = by_signal.remove(&record.signal).unwrap_or_default();
            let core_next_count = record
                .emissions
                .iter()
                .filter(|emission| emission.kind == EmissionKind::Next)
                .count();
            if layouts.is_empty() {
                if let WireLayoutStatus::DeclaredNotEmitted(reason) = &record.wire_layout {
                    let reason = reason.clone();
                    record.unresolved.retain(|message| {
                        !message.contains("per-signal pass")
                            && !message.contains("not traced by this pass")
                    });
                    record
                        .unresolved
                        .push(format!("declared, not emitted: {reason}"));
                    continue;
                }
                let fact = format!("signal_{}_wire_layout", record.signal);
                self.unresolved.push(UnresolvedFact {
                    fact: fact.clone(),
                    reason: "the collector produced no reconstructable emission for this route"
                        .to_string(),
                    span: Some(SourceSpan::from(
                        record
                            .collector
                            .as_ref()
                            .map_or(SourceSpan::default(), |collector| collector.span),
                    )),
                    required: true,
                });
                record.unresolved.push(
                    "no reconstructable value emission was traced to this signal route".to_string(),
                );
                record.wire_layout = WireLayoutStatus::Unresolved(
                    "no reconstructable value emission for this signal route".to_string(),
                );
                continue;
            }
            if core_next_count != layouts.len() {
                self.diagnostics.push(Diagnostic {
                    code: "signal_emission_surface_difference".to_string(),
                    message: format!(
                        "signal {}: core next/error/complete pass recorded {} next emission(s), the wire pass reconstructed {} value emission(s); the reconciled list keeps both proven next records",
                        record.signal, core_next_count, layouts.len()
                    ),
                    span: record.collector.as_ref().map(|collector| collector.span),
                });
            }

            // The core direct-slot placeholder says the per-signal pass has not
            // run; once layouts exist that note is stale and must not be read as
            // a remaining generator gap.
            record.unresolved.retain(|message| {
                !message.contains("per-signal pass") && !message.contains("not traced by this pass")
            });
            let mut complete = true;
            for emission in &layouts {
                let stats = wire_stats(&emission.wire);
                let prefix_count = emission
                    .prefix
                    .as_ref()
                    .map(|prefix| prefix.elements.len().max(prefix.byte_width))
                    .unwrap_or(0);
                let payload_leaves = stats.leaves.saturating_sub(prefix_count);
                let mut issues = Vec::new();
                if stats.unknown > 0 {
                    issues.push("unknown wire operation".to_string());
                }
                match emission.variant {
                    crate::signals::extract::layout::EmissionVariant::RawBuffer => {
                        if emission.prefix.is_none() {
                            issues.push("missing prefix seed".to_string());
                        }
                        if payload_leaves == 0 {
                            issues.push("no ordered payload write".to_string());
                        }
                        if stats.masked > 0 && emission.mask.is_none() {
                            issues.push("missing per-emission mask".to_string());
                        }
                    }
                    crate::signals::extract::layout::EmissionVariant::GenericObject => {
                        if stats.unknown > 0 {
                            issues.push("untyped generic object field".to_string());
                        }
                    }
                }
                if !issues.is_empty() {
                    complete = false;
                    let fact = format!(
                        "signal_{}_emission_{}",
                        record.signal, emission.origin.start
                    );
                    let reason = issues.join("; ");
                    self.unresolved.push(UnresolvedFact {
                        fact: fact.clone(),
                        reason: reason.clone(),
                        span: Some(SourceSpan::from(emission.origin)),
                        required: true,
                    });
                    record.unresolved.push(reason);
                }
                if let Some(mask) = &emission.mask {
                    if let (Some(value), Some(source)) = (mask.xor_literal, mask.xor_source.clone())
                    {
                        if seen_captures.insert((
                            emission.origin.start,
                            emission.origin.end,
                            source.clone(),
                        )) {
                            captures.push(StaticCapture {
                                kind: CaptureKind::EmissionXorLiteral,
                                span: if mask.xor_span != Span::default() {
                                    SourceSpan::from(mask.xor_span)
                                } else {
                                    SourceSpan::from(emission.origin)
                                },
                                value,
                                source,
                                collector: record.collector.clone(),
                                role: BindingRole::WaveCollector,
                            });
                        }
                    }
                }
            }
            // Reconcile the two emission surfaces instead of leaving one
            // authoritative by accident. Keep every proven core next record
            // (including `.next.call` buffers), backfill a missing buffer from
            // the wire leaves, and add layout-only next emissions.
            let mut layout_by_origin: BTreeMap<(u32, u32), &crate::signals::extract::layout::Emission> =
                BTreeMap::new();
            for emission in &layouts {
                layout_by_origin.insert((emission.origin.start, emission.origin.end), emission);
            }
            let mut reconciled = Vec::new();
            for core in &record.emissions {
                if core.kind != EmissionKind::Next {
                    reconciled.push(core.clone());
                    continue;
                }
                let mut core = core.clone();
                if core.buffer.is_none() {
                    if let Some(layout) = layout_by_origin.get(&(core.span.start, core.span.end)) {
                        core.buffer = buffer_record_of(layout);
                    }
                }
                reconciled.push(core);
            }
            for emission in &layouts {
                let present = record.emissions.iter().any(|core| {
                    core.span.start == emission.origin.start && core.span.end == emission.origin.end
                });
                if !present {
                    reconciled.push(EmissionRecord {
                        kind: EmissionKind::Next,
                        span: SourceSpan::from(emission.origin),
                        observer: match emission.route {
                            crate::signals::extract::layout::ObserverRoute::WaveSlot(slot) => {
                                ObserverRoute::WaveSlot(slot)
                            }
                            crate::signals::extract::layout::ObserverRoute::Direct => ObserverRoute::Direct,
                        },
                        payload: None,
                        buffer: buffer_record_of(emission),
                    });
                }
            }
            reconciled.sort_by_key(|emission| (emission.span.start, emission.span.end));
            record.emissions = reconciled;
            record.emission_layouts = layouts;
            record.wire_layout = if complete {
                WireLayoutStatus::Partial(format!(
                    "{} emission(s) reconstructed from the observer buffer: ordered STRING/NUMBER/BYTE writes with prefix seed and per-emission mask",
                    record.emission_layouts.len()
                ))
            } else {
                WireLayoutStatus::Unresolved(format!(
                    "{} emission(s) traced, but required wire facts remain unresolved; see required missing facts",
                    record.emission_layouts.len()
                ))
            };
        }
        captures
    }

    // -----------------------------------------------------------------------
    // Constants/external inputs
    // -----------------------------------------------------------------------

    fn constant_tables(&mut self) -> Vec<ConstantTable> {
        let mut tables = Vec::new();
        for symbol in &self.model.main_symbols {
            let Some(initializer) = self.model.symbol_value(*symbol) else {
                continue;
            };
            let Expression::ObjectExpression(_) = initializer else {
                continue;
            };
            let Some(members) = object_members(initializer) else {
                continue;
            };
            if members.len() < 4 {
                continue;
            }
            let mut entries = BTreeMap::new();
            let mut numeric = true;
            for member in members {
                match literal_value(member.value) {
                    Some(LiteralValue::Number(value)) => {
                        entries.insert(member.name.clone(), LiteralValue::Number(value));
                    }
                    _ => {
                        numeric = false;
                        break;
                    }
                }
            }
            if numeric && entries.len() >= 4 {
                tables.push(ConstantTable {
                    binding: self.model.binding_ref(*symbol),
                    span: SourceSpan::from(initializer.span()),
                    entries,
                });
            }
        }
        if tables.len() == 1 {
            let symbol = tables[0].binding.symbol;
            self.roles.insert(symbol, BindingRole::TagTable);
        }
        tables
    }

    fn external_inputs(&self, session: Option<&SessionProperties>) -> Vec<ExternalInput> {
        let mut inputs = Vec::new();
        if let Some(session) = session {
            for field in &session.context_fields {
                if let Some(assigned) = field.assigned.as_ref() {
                    if assigned.kind == crate::signals::schema::ValueKind::Literal
                        && !matches!(assigned.literal, Some(LiteralValue::Undefined))
                    {
                        continue;
                    }
                    inputs.push(ExternalInput {
                        kind: ExternalInputKind::ContextField,
                        name: field.name.clone(),
                        source: assigned.clone(),
                        public_init_argument: field.public_init_argument,
                        session_argument: field.session_argument,
                        required: field.public_init_argument.is_some(),
                    });
                }
            }
        }
        // Host globals are unresolved root references. They are inputs, not
        // guessed values; the name is a descriptive label.
        let mut seen = BTreeSet::new();
        let mut globals = Vec::new();
        for (name, references) in self.model.scoping().root_unresolved_references().iter() {
            if name == "arguments" || name == "undefined" || name == "host" {
                continue;
            }
            if !seen.insert(name.to_string()) {
                continue;
            }
            let span = references
                .first()
                .map(|reference| {
                    let node_id = self.model.scoping().get_reference(*reference).node_id();
                    SourceSpan::from(self.model.nodes().get_node(node_id).span())
                })
                .unwrap_or_default();
            globals.push(ExternalInput {
                kind: ExternalInputKind::HostGlobal,
                name: name.to_string(),
                source: ExprRecord {
                    span,
                    source: name.to_string(),
                    kind: crate::signals::schema::ValueKind::Identifier,
                    literal: None,
                    binding: None,
                    function: None,
                },
                public_init_argument: None,
                session_argument: None,
                required: false,
            });
        }
        globals.sort_by(|left, right| left.name.cmp(&right.name));
        inputs.extend(globals);
        inputs
    }

    /// The function declared as `name`, by the tracer's link naming. Worker
    /// functions are top-level siblings of the entry and need not be in the
    /// main scope, so the declaration text is the join the link offers.
    fn worker_function(&self, name: &str) -> Option<SourceSpan> {
        let needle = format!("function {name}(");
        let start = self.model.source.find(&needle)? as u32;
        // The collector request is matched by the root's span start; the end is
        // only descriptive, so the declaration start is enough.
        Some(SourceSpan::new(start, start))
    }

    fn finalize_unresolved(
        &mut self,
        descriptors: Option<&DescriptorRegistry>,
        waves: Option<&WaveRegistry>,
        session: Option<&SessionProperties>,
        helpers: &[HelperRecord],
    ) {
        if descriptors.is_none() {
            self.unresolved.push(UnresolvedFact {
                fact: "descriptor_registry".to_string(),
                reason: "no main-owned array with constructor/priority/name descriptor elements and indexed population writes was proven".to_string(),
                span: None,
                required: true,
            });
        }
        if waves.is_none() {
            self.unresolved.push(UnresolvedFact {
                fact: "wave_registry".to_string(),
                reason: "no shared consumer proved a function array, integer index rows and boolean routing selector together".to_string(),
                span: None,
                required: true,
            });
        }
        if session.is_none() {
            self.unresolved.push(UnresolvedFact {
                fact: "session_contract".to_string(),
                reason: "no constructor with a context/config literal and init/teardown return was proven".to_string(),
                span: None,
                required: true,
            });
        }
        let required_helpers = [
            BindingRole::CreateSignalMask,
            BindingRole::WriteNumber,
            BindingRole::WriteString,
            BindingRole::HashTransactionToken,
            BindingRole::ObserverFactory,
            BindingRole::SnapshotGetter,
            BindingRole::EnvelopeBuilder,
        ];
        for role in required_helpers {
            if !helpers.iter().any(|helper| helper.role == role) {
                self.unresolved.push(UnresolvedFact {
                    fact: format!("helper_{role:?}"),
                    reason: format!("no source function matched the structural role {role:?}"),
                    span: None,
                    required: true,
                });
            }
        }
        if let Some(session) = session {
            if session.stores.latest_values.is_none() {
                self.unresolved.push(UnresolvedFact {
                    fact: "latest_value_store".to_string(),
                    reason: "no descriptor-registry map produced the indexed latest-value store"
                        .to_string(),
                    span: None,
                    required: true,
                });
            }
            if session.stores.statuses.is_none() {
                self.unresolved.push(UnresolvedFact {
                    fact: "status_store".to_string(),
                    reason: "no descriptor-registry map produced the hasCollected/hasErrored store"
                        .to_string(),
                    span: None,
                    required: true,
                });
            }
            if session.stores.error_buffer.is_none() {
                self.unresolved.push(UnresolvedFact {
                    fact: "error_buffer".to_string(),
                    reason: "no snapshot errors field resolved to a capacity-normalising ring"
                        .to_string(),
                    span: None,
                    required: true,
                });
            }
            if session.default_filter.is_none() {
                self.unresolved.push(UnresolvedFact {
                    fact: "default_signal_filter".to_string(),
                    reason: "no identity index array was referenced as the snapshot default filter"
                        .to_string(),
                    span: None,
                    required: true,
                });
            }
            if session.stores.sentinel.is_none() {
                self.unresolved.push(UnresolvedFact {
                    fact: "latest_value_sentinel".to_string(),
                    reason: "the latest-value store's initial expression was not resolved"
                        .to_string(),
                    span: None,
                    required: true,
                });
            }
            if session.hash_chain.is_none() {
                self.unresolved.push(UnresolvedFact {
                    fact: "mask_state_hash_chain".to_string(),
                    reason: "the mask PRNG state was not traced back to a transaction-token hash"
                        .to_string(),
                    span: None,
                    required: true,
                });
            }
            if session
                .serializer
                .as_ref()
                .and_then(|serializer| serializer.default_encoder.as_ref())
                .is_none()
            {
                self.unresolved.push(UnresolvedFact {
                    fact: "default_encoder".to_string(),
                    reason: "the envelope builder's default encode fallback was not resolved"
                        .to_string(),
                    span: None,
                    required: true,
                });
            }
        }
    }
}

// ===========================================================================
// Local support types
// ===========================================================================

#[derive(Default)]
struct WireStats {
    leaves: usize,
    masked: usize,
    unknown: usize,
}

// ===========================================================================
// Direct constructor resolution
// ===========================================================================

/// How a descriptor `ctor` expression reaches the function that owns a direct
/// collector's emissions. Shared by the scanner and the wiring pass so both
/// read one identity.
pub(crate) struct CtorResolution {
    /// The constructor wrapper the expression calls, or the function the
    /// expression itself names when it is not a call.
    pub wrapper: Option<FunctionSite>,
    /// The task callback: the wrapper call's last argument, resolved through a
    /// proven single-definition binding.
    pub callback: Option<FunctionSite>,
    /// The wrapper call, when the expression reaches one.
    pub call_span: Option<Span>,
    /// Spec hooks a lifecycle-spec adapter hands an observer to, with the
    /// task's call of each (see [`resolve_spec_adapter`]); they own the
    /// emissions the task forwards.
    pub hooks: Vec<(FunctionSite, Span)>,
    /// Every reason a part of the identity stays unresolved.
    pub refusals: Vec<String>,
}

/// Resolve a direct collector's identity: descriptor `ctor` expression ->
/// constructor wrapper -> last function argument (the task callback).
///
/// The `ctor` member is either the wrapper call itself
/// (`{ctor: wrapper(task)}`) or a binding whose value is that call. The binding
/// form is commonly `let v = wrapper(task); ... v = {ctor: v}`: the member read
/// sits inside the right-hand side of the binding's only other write, so the
/// only definition that reaches it is the initializer. Any other write pattern
/// is refused rather than resolved by source order, and an accepted single
/// definition must still be the one the read is proven to reach.
///
/// The task callback is the call's last argument and nothing else: an earlier
/// function argument is never substituted. It must be a function literal or a
/// binding with a single never-written function definition, and the wrapper
/// must invoke the parameter slot that receives it (a formal or
/// `arguments[N]`, directly or through a local that only ever holds that slot).
/// The function that owns a direct slot's emissions: its task callback, or
/// the constructor call itself when the constructor names a function
/// directly (no wrapper call).
fn direct_task_function(model: &Model<'_, '_>, slot: &crate::signals::schema::DirectSlot) -> Option<SourceSpan> {
    slot.callback.as_ref().map(|callback| callback.span).or_else(|| {
        resolve_descriptor_ctor(model, &slot.ctor)
            .call_span
            .is_none()
            .then(|| slot.ctor_call.as_ref().map(|call| call.span))
            .flatten()
    })
}

pub(crate) fn resolve_descriptor_ctor<'a>(
    model: &Model<'_, 'a>,
    ctor: &ExprRecord,
) -> CtorResolution {
    let mut resolution = CtorResolution {
        wrapper: None,
        callback: None,
        call_span: None,
        hooks: Vec::new(),
        refusals: Vec::new(),
    };
    let span = Span::new(ctor.span.start, ctor.span.end);
    let call = match ctor
        .binding
        .as_ref()
        .and_then(|binding| model.symbol_by_index(binding.symbol))
    {
        Some(symbol) => match ctor_binding_call(model, symbol, span) {
            Ok(Some(call)) => Some(call),
            Ok(None) => {
                match proven_function_value(model, expression_at(model, span), 0) {
                    Ok(site) => resolution.wrapper = Some(site),
                    Err(reason) => resolution.refusals.push(format!(
                        "constructor binding is neither a wrapper call nor a function: {reason}"
                    )),
                }
                return resolution;
            }
            Err(reason) => {
                resolution.refusals.push(reason);
                return resolution;
            }
        },
        None => match expression_at(model, span).map(Expression::without_parentheses) {
            Some(Expression::CallExpression(call)) => Some(&**call),
            Some(expression) => {
                match proven_function_value(model, Some(expression), 0) {
                    Ok(site) => resolution.wrapper = Some(site),
                    Err(reason) => resolution.refusals.push(format!(
                        "constructor expression is neither a wrapper call nor a function: {reason}"
                    )),
                }
                return resolution;
            }
            None => {
                resolution
                    .refusals
                    .push("constructor expression is not a parsed expression".to_string());
                return resolution;
            }
        },
    };
    let Some(call) = call else {
        return resolution;
    };
    resolution.call_span = Some(call.span);
    match proven_function_value(model, Some(&call.callee), 0) {
        Ok(site) => resolution.wrapper = Some(site),
        Err(reason) => resolution.refusals.push(format!(
            "constructor wrapper callee is not a proven function: {reason}"
        )),
    }
    let Some(last) = call.arguments.last() else {
        resolution
            .refusals
            .push("constructor wrapper call passes no task callback".to_string());
        return resolution;
    };
    let Some(argument) = last.as_expression() else {
        resolution
            .refusals
            .push("constructor wrapper's last argument is a spread".to_string());
        return resolution;
    };
    let callback = match proven_function_value(model, Some(argument), 0) {
        Ok(site) => site,
        Err(reason) => {
            let adapter = match resolution.wrapper {
                Some(adapter) => resolve_spec_adapter(
                    model,
                    call,
                    adapter,
                    (call.arguments.len() - 1) as u32,
                    argument,
                ),
                None => Err("no proven adapter".to_string()),
            };
            match adapter {
                Ok(adapter) => {
                    resolution.wrapper = Some(adapter.wrapper);
                    resolution.callback = Some(adapter.task);
                    resolution.hooks = adapter.hooks;
                }
                Err(adapter_reason) => resolution.refusals.push(format!(
                    "constructor wrapper's last argument is not a proven task callback: {reason}; \
                     nor a lifecycle spec: {adapter_reason}"
                )),
            }
            return resolution;
        }
    };
    let Some(wrapper) = resolution.wrapper else {
        return resolution;
    };
    let slot = (call.arguments.len() - 1) as u32;
    if invokes_slot(model, wrapper, slot) {
        resolution.callback = Some(callback);
    } else {
        resolution.refusals.push(format!(
            "constructor wrapper never invokes its task-callback parameter slot {slot}"
        ));
    }
    resolution
}

/// A lifecycle-spec adapter resolved from `adapter(factory())`.
pub(crate) struct SpecAdapter {
    /// The constructor wrapper the adapter returns a call of.
    pub wrapper: FunctionSite,
    /// The task callback the adapter hands that wrapper.
    pub task: FunctionSite,
    /// Spec hooks the task calls with an observer a registrar hands out,
    /// with that call.
    pub hooks: Vec<(FunctionSite, Span)>,
}

/// The top-level statements of a function or block-bodied arrow.
fn function_statements<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
) -> Option<&'a oxc_allocator::Vec<'a, Statement<'a>>> {
    match model.nodes().kind(site.node_id) {
        AstKind::Function(function) => Some(&function.body.as_ref()?.statements),
        AstKind::ArrowFunctionExpression(arrow) if !arrow.expression => {
            Some(&arrow.body.statements)
        }
        _ => None,
    }
}

/// The single top-level `return <expression>` of a function body.
fn single_return<'a>(model: &Model<'_, 'a>, site: FunctionSite) -> Option<&'a Expression<'a>> {
    let mut returns = function_statements(model, site)?.iter().filter_map(|statement| {
        match statement {
            Statement::ReturnStatement(ret) => Some(ret.argument.as_ref()),
            _ => None,
        }
    });
    let value = returns.next()??;
    returns.next().is_none().then_some(value)
}

fn is_undefined_value(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => identifier.name == "undefined",
        Expression::UnaryExpression(unary) => {
            unary.operator == oxc_syntax::operator::UnaryOperator::Void
                && matches!(unary.argument, Expression::NumericLiteral(_))
        }
        _ => false,
    }
}

/// Resolve a lifecycle-spec adapter: the descriptor constructor is
/// `adapter(factory())`, where
///
/// * the adapter's body is a single `return wrapper(task)`, `wrapper` being a
///   proven function that invokes its task slot;
/// * `task` calls `spec.hook(observer)`, `spec` being the adapter's spec
///   parameter slot (or a local that only holds it) and `observer` the first
///   formal of a callback that `task` hands to one of its registrar formals;
/// * `factory`'s single return is a local object whose `hook` member has
///   exactly one function definition (its literal property or a
///   `local.hook = function` write in the factory), every other definition
///   being `undefined`, and the local is never reassigned.
///
/// Every step is structural; any other shape is refused with its reason.
fn resolve_spec_adapter<'a>(
    model: &Model<'_, 'a>,
    call: &CallExpression<'a>,
    adapter: FunctionSite,
    slot: u32,
    argument: &'a Expression<'a>,
) -> Result<SpecAdapter, String> {
    let Expression::CallExpression(spec_call) = argument.without_parentheses() else {
        return Err("the spec argument is not a factory call".to_string());
    };
    let factory = proven_function_value(model, Some(&spec_call.callee), 0)
        .map_err(|reason| format!("the spec factory is not a proven function: {reason}"))?;
    let Some(Expression::CallExpression(inner)) =
        single_return(model, adapter).map(Expression::without_parentheses)
    else {
        return Err("the adapter body is not a single `return wrapper(task)`".to_string());
    };
    // The adapter's `return wrapper(task)` runs when the descriptor calls the
    // adapter, so a wrapper binding written once before that call (and after
    // the adapter's declaration) is read at the descriptor's call.
    let wrapper = proven_function_value(model, Some(&inner.callee), 0)
        .or_else(|reason| {
            let (Expression::Identifier(callee), Expression::Identifier(read)) = (
                inner.callee.without_parentheses(),
                call.callee.without_parentheses(),
            ) else {
                return Err(reason);
            };
            let symbol = crate::analysis::identifier_symbol(callee, model.scoping())
                .ok_or_else(|| reason.clone())?;
            let (_, span) =
                proven_function_at_read(model.scoping(), model.nodes(), symbol, read)
                    .map_err(|late| format!("{reason}; at the descriptor call: {late}"))?;
            model
                .site_by_span(span)
                .ok_or_else(|| "function node is not indexed".to_string())
        })
        .map_err(|reason| format!("the adapter's wrapper is not a proven function: {reason}"))?;
    let task_slot = inner
        .arguments
        .len()
        .checked_sub(1)
        .ok_or_else(|| "the adapter's wrapper call passes no task".to_string())?;
    let task = proven_function_value(model, inner.arguments[task_slot].as_expression(), 0)
        .map_err(|reason| format!("the adapter's task is not a proven function: {reason}"))?;
    if !invokes_slot(model, wrapper, task_slot as u32) {
        return Err("the adapter's wrapper never invokes its task slot".to_string());
    }
    // Hook names the task calls with a registrar-handed observer.
    let mut names: Vec<(&str, Span)> = Vec::new();
    for node in model.nodes_within(task).into_iter() {
        let AstKind::CallExpression(registrar_call) = node.kind() else {
            continue;
        };
        // The registrar is a task parameter, read directly or through a
        // local that only ever holds it (`r = registrar` on try and catch).
        if !model.node_within(node.id(), task)
            || !model
                .site_slot_value(&registrar_call.callee, task)
                .is_some_and(|index| index >= 1)
        {
            continue;
        }
        for registered in call_arguments(registrar_call) {
            let Ok(callback) = proven_function_value(model, Some(registered), 0) else {
                continue;
            };
            for inner_node in model.nodes_within(callback).into_iter() {
                let AstKind::CallExpression(hook_call) = inner_node.kind() else {
                    continue;
                };
                if !model.node_within(inner_node.id(), callback) {
                    continue;
                }
                let callee = hook_call.callee.without_parentheses();
                let (Some(name), Some(object)) =
                    (static_member_name(callee), member_object_expression(callee))
                else {
                    continue;
                };
                let observer = call_arguments(hook_call).first().copied();
                if model.site_slot_value(object, adapter) == Some(slot)
                    && observer
                        .is_some_and(|value| model.site_parameter_slot(value, callback) == Some(0))
                {
                    if names.iter().any(|(seen, _)| *seen == name) {
                        return Err(format!("the task calls spec hook `{name}` more than once"));
                    }
                    names.push((name, hook_call.span));
                }
            }
        }
    }
    if names.is_empty() {
        return Err("the task hands no registrar observer to a spec hook".to_string());
    }
    // The spec object the factory returns and its hook definitions.
    let Some(Expression::Identifier(spec)) =
        single_return(model, factory).map(Expression::without_parentheses)
    else {
        return Err("the spec factory does not return a single local".to_string());
    };
    let scoping = model.scoping();
    let spec_symbol = crate::analysis::identifier_symbol(spec, scoping)
        .ok_or_else(|| "the spec local is unresolved".to_string())?;
    let declaration = scoping.symbol_span(spec_symbol);
    let body = factory.body_span.unwrap_or(factory.span);
    if declaration.start < body.start
        || declaration.end > body.end
        || !scoping.symbol_redeclarations(spec_symbol).is_empty()
        || scoping
            .get_resolved_references(spec_symbol)
            .any(|reference| reference.is_write())
    {
        return Err("the spec local is not a never-written local of the factory".to_string());
    }
    let Some(Expression::ObjectExpression(literal)) =
        model.initializer(spec_symbol).map(Expression::without_parentheses)
    else {
        return Err("the spec local is not initialized with an object literal".to_string());
    };
    let mut hooks = Vec::new();
    for (name, call) in names {
        let mut values: Vec<&'a Expression<'a>> = Vec::new();
        for property in &literal.properties {
            match property {
                ObjectPropertyKind::ObjectProperty(property) => {
                    if property_key_name(&property.key).as_deref() == Some(name) {
                        values.push(&property.value);
                    }
                }
                ObjectPropertyKind::SpreadProperty(_) => {
                    return Err("the spec literal has a spread".to_string());
                }
            }
        }
        for node in model.kind_nodes(oxc_ast::AstType::AssignmentExpression) {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let target = match &assignment.left {
                AssignmentTarget::StaticMemberExpression(member) => {
                    Some((member.property.name.as_str(), &member.object))
                }
                AssignmentTarget::ComputedMemberExpression(member) => match &member.expression {
                    Expression::StringLiteral(key) => Some((key.value.as_str(), &member.object)),
                    _ => None,
                },
                _ => None,
            };
            let Some((key, Expression::Identifier(object))) = target else {
                continue;
            };
            if key != name
                || crate::analysis::identifier_symbol(object, scoping) != Some(spec_symbol)
            {
                continue;
            }
            if !model.node_within(node.id(), factory)
                || assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign
            {
                return Err(format!("spec hook `{name}` is written outside the factory"));
            }
            values.push(&assignment.right);
        }
        // A hook the spec never defines stays `undefined`: the task skips it.
        let mut functions = values.iter().filter(|value| !is_undefined_value(value));
        let value = match (functions.next(), functions.next()) {
            (None, _) => continue,
            (Some(value), None) => value,
            _ => return Err(format!("spec hook `{name}` has more than one definition")),
        };
        let hook = proven_function_value(model, Some(value), 0)
            .map_err(|reason| format!("spec hook `{name}` is not a proven function: {reason}"))?;
        hooks.push((hook, call));
    }
    if hooks.is_empty() {
        return Err("the spec defines none of the hooks the task calls".to_string());
    }
    Ok(SpecAdapter { wrapper, task, hooks })
}

/// The reply parameter a collector forwards: the signal's emission payload is a
/// plain identifier and the same name is the callback parameter of a
/// `vmWorkerCallbacks.push(...)` registration in this collector.
/// `text` is the source slice starting at offset `base` (the scanned
/// collector function: a wave function, or a direct slot's task callback).
pub(crate) fn forwarded_parameter(text: &str, base: u32, record: &SignalRecord) -> Option<String> {
    // A source branch can register a named function while another registers
    // an inline arrow. Resolve every callback binding; looking at only the
    // first textual `push((param)` missed both worker emission variants.
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, text).ok()?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    fn parameter<'a>(expression: &'a Expression<'a>, scoping: &oxc_semantic::Scoping,
                     nodes: &oxc_semantic::AstNodes<'a>, depth: usize) -> Option<&'a BindingIdentifier<'a>> {
        if depth == 0 { return None; }
        let params = match expression.without_parentheses() {
            Expression::FunctionExpression(f) => &f.params,
            Expression::ArrowFunctionExpression(f) => &f.params,
            Expression::Identifier(id) => {
                let symbol = scoping.get_reference(id.reference_id()).symbol_id()?;
                return match nodes.kind(scoping.symbol_declaration(symbol)) {
                    AstKind::Function(f) => match &f.params.items.first()?.pattern {
                        BindingPattern::BindingIdentifier(id) => Some(id), _ => None,
                    },
                    AstKind::VariableDeclarator(d) => parameter(d.init.as_ref()?, scoping, nodes, depth-1),
                    _ => None,
                };
            }
            _ => return None,
        };
        match &params.items.first()?.pattern {
            BindingPattern::BindingIdentifier(id) => Some(id), _ => None,
        }
    }
    let mut found = BTreeSet::new();
    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else { continue; };
        let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() else { continue; };
        if member.property.name != "push"
            || !matches!(member.object.without_parentheses(), Expression::Identifier(id) if id.name == "vmWorkerCallbacks")
        { continue; }
        let Some(argument) = call.arguments.first().and_then(Argument::as_expression) else { continue; };
        // A relay function (`post(t, pc, msg, op, cb) { …push(cb); … }`)
        // registers what each of its callers passes for that parameter.
        let callbacks: Vec<&Expression<'_>> = match relay_parameter_calls(argument, &scoping, &nodes) {
            Some((slot, calls)) => calls
                .into_iter()
                .filter_map(|call| call.arguments.get(slot).and_then(Argument::as_expression))
                .collect(),
            None => vec![argument],
        };
        for callback in callbacks {
            let Some(id) = parameter(callback, &scoping, &nodes, 8) else { continue; };
            // A direct slot's record names no payload binding for its
            // emissions (its task callback is scanned by the layout pass):
            // the callback itself must hand its parameter to an observer
            // `next`.
            let unbound = record.emissions.iter().all(|emission| {
                emission.payload.as_ref().and_then(|payload| payload.binding.as_ref()).is_none()
            });
            if record.direct && unbound {
                let symbol = id.symbol_id.get();
                let forwards = nodes.iter().any(|node| {
                    let AstKind::CallExpression(next) = node.kind() else { return false; };
                    let Expression::StaticMemberExpression(member) = next.callee.without_parentheses() else { return false; };
                    member.property.name == "next"
                        && next.arguments.len() == 1
                        && matches!(next.arguments[0].as_expression().map(Expression::without_parentheses),
                            Some(Expression::Identifier(argument))
                                if symbol.is_some() && scoping.get_reference(argument.reference_id()).symbol_id() == symbol)
                });
                if forwards {
                    found.insert(id.name.to_string());
                }
                continue;
            }
            if record.emissions.iter().filter(|emission| emission.kind == EmissionKind::Next)
                .filter_map(|emission| emission.payload.as_ref()).any(|payload| {
                    payload.source.trim() == id.name.as_str() && payload.binding.as_ref().is_some_and(|binding| {
                        binding.identifier.start == base + id.span.start && binding.identifier.end == base + id.span.end
                    })
                })
            { found.insert(id.name.to_string()); }
        }
    }
    (found.len() == 1).then(|| found.into_iter().next()).flatten()
}

/// When `expression` reads formal parameter `k` of a function declaration,
/// that slot and every call of the function by name (all of its references
/// must be such calls).
fn relay_parameter_calls<'a>(
    expression: &Expression<'_>,
    scoping: &oxc_semantic::Scoping,
    nodes: &oxc_semantic::AstNodes<'a>,
) -> Option<(usize, Vec<&'a CallExpression<'a>>)> {
    let Expression::Identifier(identifier) = expression.without_parentheses() else { return None; };
    let symbol = scoping.get_reference(identifier.reference_id()).symbol_id()?;
    let declaration = scoping.symbol_declaration(symbol);
    let AstKind::FormalParameter(formal) = nodes.kind(declaration) else { return None; };
    let AstKind::FormalParameters(parameters) = nodes.kind(nodes.parent_id(declaration)) else { return None; };
    let slot = parameters.items.iter().position(|item| item.span == formal.span)?;
    let AstKind::Function(function) = nodes.kind(nodes.parent_id(nodes.parent_id(declaration))) else { return None; };
    let function_symbol = function.id.as_ref()?.symbol_id.get()?;
    let mut calls = Vec::new();
    for reference in scoping.get_resolved_references(function_symbol) {
        let AstKind::CallExpression(call) = nodes.kind(nodes.parent_id(reference.node_id())) else { return None; };
        if call.callee.span() != nodes.get_node(reference.node_id()).span() {
            return None;
        }
        calls.push(call);
    }
    (!calls.is_empty()).then_some((slot, calls))
}

/// `vmWorker.postMessage([thread, pc, …])`: the two numbers that name the
/// worker function a launcher posts to.
pub(crate) fn posted_worker_target(text: &str) -> Option<(String, String)> {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, text).ok()?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let number = |expression: &Expression<'_>| {
        let Expression::NumericLiteral(n) = expression.without_parentheses() else { return None; };
        (n.value.is_finite() && n.value >= 0.0 && n.value <= u32::MAX as f64 && n.value.fract() == 0.0)
            .then(|| (n.value as u32).to_string())
    };
    let integer = |element: &ArrayExpressionElement<'_>| number(element.as_expression()?);
    let mut targets = BTreeSet::new();
    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else { continue; };
        let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() else { continue; };
        if member.property.name != "postMessage"
            || !matches!(member.object.without_parentheses(), Expression::Identifier(id) if id.name == "vmWorker")
        { continue; }
        let Some(Expression::ArrayExpression(values)) = call.arguments.first().and_then(Argument::as_expression) else { continue; };
        if values.elements.len() < 2 { continue; }
        if let (Some(thread),Some(pc)) = (integer(&values.elements[0]),integer(&values.elements[1])) {
            targets.insert((thread,pc));
            continue;
        }
        // A relay function posts its own parameters: each caller names the
        // target with the literals it passes for them.
        let relay = |element: &ArrayExpressionElement<'_>| {
            relay_parameter_calls(element.as_expression()?, &scoping, &nodes)
        };
        if let (Some((thread_slot, calls)), Some((pc_slot, pc_calls))) =
            (relay(&values.elements[0]), relay(&values.elements[1]))
        {
            if calls.len() != pc_calls.len() { continue; }
            for call in calls {
                let argument = |slot: usize| call.arguments.get(slot).and_then(Argument::as_expression).and_then(number);
                match (argument(thread_slot), argument(pc_slot)) {
                    (Some(thread), Some(pc)) => { targets.insert((thread, pc)); }
                    _ => return None,
                }
            }
        }
    }
    (targets.len() == 1).then(|| targets.into_iter().next()).flatten()
}

/// The expression node whose exact span is `span`.
fn expression_at<'a>(model: &Model<'_, 'a>, span: Span) -> Option<&'a Expression<'a>> {
    // An expression record keeps the span of the member value (or of the
    // value of a `holder.ctor = value` write completing the literal); the
    // node is found again by that exact span in the same parse.
    model.nodes().iter().find_map(|node| match node.kind() {
        AstKind::ObjectProperty(property) if property.value.span() == span => Some(&property.value),
        AstKind::AssignmentExpression(assignment)
            if assignment.right.span() == span
                && matches!(assignment.left, AssignmentTarget::StaticMemberExpression(_)) =>
        {
            Some(&assignment.right)
        }
        _ => None,
    })
}

/// The wrapper call a descriptor constructor binding holds when the
/// descriptor reads it at `read`. `Ok(None)` means the binding's value is not
/// a call at all. A single definition is accepted only when the read is proven
/// to reach it ([`reaching_value_at_read`]); a read before the only write, or
/// before the declarator, is refused as ambiguous.
fn ctor_binding_call<'a>(
    model: &Model<'_, 'a>,
    symbol: SymbolId,
    read: Span,
) -> Result<Option<&'a CallExpression<'a>>, String> {
    let scoping = model.scoping();
    let nodes = model.nodes();
    if !scoping.symbol_redeclarations(symbol).is_empty() {
        return Err("constructor binding is declared more than once".to_string());
    }
    let mut writes = Vec::new();
    let mut other_write = false;
    for reference in scoping.get_resolved_references(symbol) {
        if !reference.is_write() {
            continue;
        }
        let parent = nodes.parent_id(reference.node_id());
        match nodes.kind(parent) {
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign =>
            {
                writes.push(assignment);
            }
            // A compound or update write cannot define a constructor value, but
            // it is not a reason to refuse the whole binding: the shared
            // reaching-definition proof below refuses a read whose nearest write
            // is one of these, and accepts a read whose nearest definition is
            // plain. Collectors reuse a scratch local for counters and objects,
            // so refusing here loses every dynamic-slot constructor.
            _ => other_write = true,
        }
    }
    let initializer = model.initializer(symbol);
    let call_of = |value: &'a Expression<'a>| match value.without_parentheses() {
        Expression::CallExpression(call) => Some(&**call),
        _ => None,
    };
    if initializer.is_none() && writes.is_empty() && !other_write {
        return Ok(None);
    }
    let reaching = reaching_value_at_read(model, read).map_err(|reason| {
        format!("constructor binding's value at the descriptor read is not proven: {reason}")
    })?;
    if let Some(value) = initializer {
        if value.span() == reaching.span() {
            return Ok(call_of(value));
        }
    }
    for assignment in &writes {
        if assignment.right.span() == reaching.span() {
            return Ok(call_of(&assignment.right));
        }
    }
    Err(
        "constructor binding's descriptor read sees a definition that is not a plain          `local = value` write"
            .to_string(),
    )
}

/// The value expression the descriptor read at `read` is proven to see (the
/// shared single-reaching-definition proof), or the reason the read's value is
/// not proven. A read outside a statement list, in a nested function, after a
/// loop or a branch, or with no preceding definition yields the reason.
fn reaching_value_at_read<'a>(
    model: &Model<'_, 'a>,
    read: Span,
) -> Result<&'a Expression<'a>, String> {
    expression_at(model, read)
        .ok_or_else(|| "the descriptor read is not a parsed expression".to_string())
        .and_then(|expression| expression_value_at_use(model.scoping(), model.nodes(), expression))
        .map(|definition| definition.value)
}

/// The function a value expression denotes through proven bindings only: a
/// function/arrow literal, or an identifier bound once to a function
/// declaration or a function-valued declarator that nothing else writes.
/// See [`proven_function_node`].
fn proven_function_value<'a>(
    model: &Model<'_, 'a>,
    expression: Option<&Expression<'a>>,
    depth: usize,
) -> Result<FunctionSite, String> {
    let (_, span) = proven_function_node(model.scoping(), model.nodes(), expression, depth)?;
    model
        .site_by_span(span)
        .ok_or_else(|| "function node is not indexed".to_string())
}

/// True when the task callback and every function nested in it never mention an
/// observer `next` property, while at least one `complete` property is used.
/// This is the source-level proof that the capture never emits a value for the
/// signal; the check is deliberately structural (member property names on the
/// parsed AST), so any `next` read or call anywhere in the subtree refuses the
/// classification.
fn task_completes_without_value(model: &Model<'_, '_>, site: FunctionSite) -> bool {
    let mut has_next = false;
    let mut has_complete = false;
    for node in model.nodes_within(site).into_iter() {
        if !model.node_within(node.id(), site) {
            continue;
        }
        match node.kind() {
            AstKind::StaticMemberExpression(member) => match member.property.name.as_str() {
                "next" => has_next = true,
                "complete" => has_complete = true,
                _ => {}
            },
            AstKind::ComputedMemberExpression(member) => {
                if let Expression::StringLiteral(value) = member.expression.without_parentheses() {
                    match value.value.as_str() {
                        "next" => has_next = true,
                        "complete" => has_complete = true,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    has_complete && !has_next
}

/// True when `wrapper` (or a function nested in it) calls the parameter slot
/// `slot` of `wrapper`, directly or through `.call`/`.apply`.
fn invokes_slot(model: &Model<'_, '_>, wrapper: FunctionSite, slot: u32) -> bool {
    model.nodes_within(wrapper).into_iter().any(|node| {
        let AstKind::CallExpression(call) = node.kind() else {
            return false;
        };
        if !model.node_within(node.id(), wrapper) {
            return false;
        }
        let callee = call.callee.without_parentheses();
        let target = match static_member_name(callee) {
            Some("call" | "apply") => member_object_expression(callee).unwrap_or(callee),
            _ => callee,
        };
        model.site_slot_value(target, wrapper) == Some(slot)
    })
}

fn wire_stats(wire: &crate::signals::extract::layout::WireOp) -> WireStats {
    let mut stats = WireStats::default();
    fn visit(wire: &crate::signals::extract::layout::WireOp, stats: &mut WireStats) {
        match wire {
            crate::signals::extract::layout::WireOp::Sequence(parts) | crate::signals::extract::layout::WireOp::Choice(parts) => {
                for part in parts {
                    visit(part, stats);
                }
            }
            crate::signals::extract::layout::WireOp::Repeat { body, .. } => visit(body, stats),
            crate::signals::extract::layout::WireOp::Leaf(leaf) => {
                stats.leaves += 1;
                if leaf.masked {
                    stats.masked += 1;
                }
            }
            crate::signals::extract::layout::WireOp::Unknown { .. } => stats.unknown += 1,
        }
    }
    visit(wire, &mut stats);
    stats
}

fn first_buffer_name(wire: &crate::signals::extract::layout::WireOp) -> Option<String> {
    match wire {
        crate::signals::extract::layout::WireOp::Sequence(parts) | crate::signals::extract::layout::WireOp::Choice(parts) => {
            parts.iter().find_map(first_buffer_name)
        }
        crate::signals::extract::layout::WireOp::Repeat { body, .. } => first_buffer_name(body),
        crate::signals::extract::layout::WireOp::Leaf(leaf) => leaf.buffer.clone(),
        crate::signals::extract::layout::WireOp::Unknown { .. } => None,
    }
}

fn buffer_record_of(emission: &crate::signals::extract::layout::Emission) -> Option<ExprRecord> {
    let source = first_buffer_name(&emission.wire)?;
    Some(ExprRecord {
        span: SourceSpan::from(emission.origin),
        source,
        kind: crate::signals::schema::ValueKind::Identifier,
        literal: None,
        binding: None,
        function: None,
    })
}

struct OverrideValue<'a> {
    value: &'a Expression<'a>,
    value_span: Span,
    assignment_span: Span,
}

struct Triple {
    row_symbol: SymbolId,
    selector_symbol: SymbolId,
    function_symbol: SymbolId,
    row_values: Vec<Vec<usize>>,
    selector_values: Vec<bool>,
    functions: Vec<FunctionSite>,
    row_span: Span,
    selector_span: Span,
    function_span: Span,
}

fn array_of_integer_arrays(expression: &Expression<'_>) -> Option<Vec<Vec<usize>>> {
    let outer = array_elements(expression)?;
    let mut rows = Vec::with_capacity(outer.len());
    for element in outer {
        rows.push(is_integer_array(element)?);
    }
    (!rows.is_empty()).then_some(rows)
}

fn integer_sequence(expression: &Expression<'_>) -> Option<Vec<usize>> {
    let values = is_integer_array(expression)?;
    if values
        .iter()
        .enumerate()
        .all(|(index, value)| index == *value)
    {
        Some(values)
    } else {
        None
    }
}

/// The object and key of a `object["key"] = ...` or `object.key = ...`
/// assignment target; both capture representations spell member writes.
fn member_target_key<'t, 'a>(
    target: &'t AssignmentTarget<'a>,
) -> Option<(&'t Expression<'a>, String)> {
    match target {
        AssignmentTarget::ComputedMemberExpression(member) => {
            Some((&member.object, computed_member_name(member)?))
        }
        AssignmentTarget::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.to_string()))
        }
        _ => None,
    }
}

fn simple_assignment_symbol(
    target: &AssignmentTarget<'_>,
    scoping: &oxc_semantic::Scoping,
) -> Option<SymbolId> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, scoping)
}

fn emitted_buffer<'a>(payload: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::ArrayExpression(array) = payload else {
        return None;
    };
    let elements: Vec<_> = array
        .elements
        .iter()
        .filter_map(ArrayExpressionElement::as_expression)
        .collect();
    if elements.len() != 2 {
        return None;
    }
    if !matches!(elements[0], Expression::ArrayExpression(inner) if inner.elements.is_empty()) {
        return None;
    }
    Some(elements[1])
}

fn observer_sink_call<'a>(
    call: &'a CallExpression<'a>,
) -> Option<(EmissionKind, &'a Expression<'a>)> {
    let name = static_member_name(&call.callee)?;
    let kind = match name {
        "next" => EmissionKind::Next,
        "error" => EmissionKind::Error,
        "complete" => EmissionKind::Complete,
        _ => {
            if name == "call" {
                let Expression::StaticMemberExpression(inner) = &call.callee else {
                    return None;
                };
                let sink = static_member_name(&inner.object)?;
                let kind = match sink {
                    "next" => EmissionKind::Next,
                    "error" => EmissionKind::Error,
                    "complete" => EmissionKind::Complete,
                    _ => return None,
                };
                let observer = member_object_expression(&inner.object)?;
                return Some((kind, observer));
            }
            return None;
        }
    };
    let observer = match &call.callee {
        Expression::StaticMemberExpression(member) => Some(&member.object),
        Expression::ComputedMemberExpression(member) => Some(&member.object),
        _ => None,
    }?;
    Some((kind, observer))
}

/// The property name of a `this.name` / `this["name"]` assignment target.
fn this_member_name(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::StaticMemberExpression(member)
            if matches!(
                member.object.without_parentheses(),
                Expression::ThisExpression(_)
            ) =>
        {
            Some(member.property.name.to_string())
        }
        AssignmentTarget::ComputedMemberExpression(member)
            if matches!(
                member.object.without_parentheses(),
                Expression::ThisExpression(_)
            ) =>
        {
            computed_member_name(member)
        }
        _ => None,
    }
}

/// The property name of a `this.name` / `this["name"]` expression.
fn this_member_expression_name<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    let object = member_object_expression(expression)?;
    if !matches!(object.without_parentheses(), Expression::ThisExpression(_)) {
        return None;
    }
    static_member_name(expression)
}

fn member_object_expression<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expression {
        Expression::StaticMemberExpression(member) => Some(&member.object),
        Expression::ComputedMemberExpression(member) => Some(&member.object),
        _ => None,
    }
}

fn sha256_hex(source: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod worker_forwarding_tests {
    use super::*;
    use serde_json::json;

    fn record(kind: &str, declaration: u32) -> SignalRecord {
        serde_json::from_value(json!({
            "signal": 1, "descriptor_index": 1, "direct": false,
            "wave": 0, "wave_position": 0, "observer": {"wave_slot": 0},
            "collector": {"binding": null, "name_label": null,
                "span": {"start": 100, "end": 500}, "body": null, "source": null},
            "emissions": [{"kind": kind, "span": {"start": 200, "end": 220},
                "observer": {"wave_slot": 0}, "buffer": null,
                "payload": {"span": {"start": 205, "end": 210}, "source": "reply",
                    "kind": "identifier", "literal": null, "function": null,
                    "binding": {"symbol": 1, "name": "reply", "owned_by_main": false,
                        "declaration": {"start": declaration, "end": declaration + 5},
                        "identifier": {"start": declaration, "end": declaration + 5}}}}],
            "emission_layouts": [], "wire_layout": "not_analyzed", "unresolved": []
        }))
        .unwrap()
    }

    #[test]
    fn errors_with_a_same_spelled_parameter_are_not_worker_replies() {
        let source = "vmWorkerCallbacks.push((reply) => sink.next(reply));";
        let binding = 100 + source.find("reply").unwrap() as u32;
        assert_eq!(forwarded_parameter(source, 100, &record("error", binding)), None);
        assert_eq!(
            forwarded_parameter(source, 100, &record("next", binding)),
            Some("reply".into())
        );
    }

    #[test]
    fn worker_reply_requires_the_callback_parameter_binding() {
        let source = "vmWorkerCallbacks.push((reply) => sink.next(reply));";
        assert_eq!(forwarded_parameter(source, 100, &record("next", 450)), None);
    }

    #[test]
    fn named_worker_callback_does_not_hide_the_later_inline_registration() {
        let source = "function branch(reply){sink.next(reply);}vmWorkerCallbacks.push(branch);vmWorkerCallbacks.push((reply)=>sink.next(reply));";
        let named = 100 + source.find("reply").unwrap() as u32;
        let inline = 100 + source.rfind("(reply)=>").unwrap() as u32 + 1;
        assert_eq!(forwarded_parameter(source, 100, &record("next", named)), Some("reply".into()));
        assert_eq!(forwarded_parameter(source, 100, &record("next", inline)), Some("reply".into()));
        assert_eq!(forwarded_parameter(source, 100, &record("next", 450)), None);
    }

    #[test]
    fn worker_dispatch_accepts_integer_exponent_spelling_and_rejects_conflicts() {
        assert_eq!(posted_worker_target("vmWorker.postMessage([3,122e3,reply,157]);"),
                   Some(("3".into(),"122000".into())));
        assert_eq!(posted_worker_target("vmWorker.postMessage([3,122e3]);vmWorker.postMessage([4,122e3]);"),None);
        assert_eq!(posted_worker_target("vmWorker.postMessage([3,12.5]);"),None);
    }
}

#[cfg(test)]
mod spec_adapter_tests {
    use super::*;

    const SOURCE: &str = r#"
let wrap;
function adapt(spec) {
  let held = spec;
  return wrap((context, init, collection, lastChance, cleanup) => {
    if (held.setup) { held.setup(context); }
    if (held.immediately) { init((observer) => { held.immediately(observer); }); }
    if (held.afterReady) { collection((observer) => { held.afterReady(observer); }); }
  });
}
wrap = function (task) { return task(1, 2, 3, 4, 5); };
function makeSpec() {
  let cell;
  function setup(context) { cell = context.global.document; }
  let spec = { setup: setup, immediately: void 0 };
  function immediately(observer) { observer.next({ value: cell.cookie }); observer.complete(); }
  spec.immediately = immediately;
  return spec;
}
let entry = { ctor: adapt(makeSpec()) };
"#;

    fn resolve(source: &str) -> (Option<String>, Option<String>, Vec<String>, Vec<String>) {
        let allocator = oxc_allocator::Allocator::default();
        let parsed = crate::analysis::parse(&allocator, source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let (scoping, nodes) = crate::analysis::analyze(&parsed.program);
        let model = Model::new(source, &parsed.program, &scoping, &nodes, ProgramPropertiesLimits::default())
            .expect("model");
        let value = "adapt(makeSpec())";
        let start = source.find(value).unwrap() as u32;
        let ctor = ExprRecord {
            span: SourceSpan::new(start, start + value.len() as u32),
            source: value.to_string(),
            kind: crate::signals::schema::ValueKind::Call,
            literal: None,
            binding: None,
            function: None,
        };
        let text = |site: FunctionSite| source[site.span.start as usize..site.span.end as usize].to_string();
        let resolution = resolve_descriptor_ctor(&model, &ctor);
        (
            resolution.wrapper.map(text),
            resolution.callback.map(text),
            resolution.hooks.iter().map(|(site, _)| text(*site)).collect(),
            resolution.refusals,
        )
    }

    /// `adapt(makeSpec())`: the wrapper is the late-bound `wrap`, the task is
    /// the adapter's arrow and the only defined observer hook is `immediately`.
    #[test]
    fn lifecycle_spec_adapter_resolves_its_hooks() {
        let (wrapper, task, hooks, refusals) = resolve(SOURCE);
        assert!(refusals.is_empty(), "{refusals:?}");
        assert!(wrapper.unwrap().starts_with("function (task)"));
        assert!(task.unwrap().starts_with("(context, init"));
        assert_eq!(hooks.len(), 1);
        assert!(hooks[0].starts_with("function immediately(observer)"));
    }

    /// A hook defined twice has no single definition: the spec is refused.
    #[test]
    fn spec_hook_with_two_definitions_is_refused() {
        let source = SOURCE.replace(
            "spec.immediately = immediately;",
            "spec.immediately = immediately; spec.immediately = setup;",
        );
        let (_, task, hooks, refusals) = resolve(&source);
        assert!(task.is_none() && hooks.is_empty());
        assert!(refusals.iter().any(|reason| reason.contains("more than one definition")), "{refusals:?}");
    }

    /// A wrapper binding written twice is not read at the descriptor call.
    #[test]
    fn late_wrapper_written_twice_is_refused() {
        let source = SOURCE.replace("let entry", "wrap = null; let entry");
        let (_, task, _, refusals) = resolve(&source);
        assert!(task.is_none());
        assert!(refusals.iter().any(|reason| reason.contains("2 writes")), "{refusals:?}");
    }
}

#[cfg(test)]
mod consumed_registry_tests {
    use super::*;
    fn registry(body: &str) -> Result<DescriptorRegistry, ProgramPropertiesError> {
        let source = format!("(function main() {{ let r = [{{ctor:null,priority:false}}]; {body} r.forEach(d=>d.ctor); return {{init(){{}}}}; }})();");
        let arena = oxc_allocator::Allocator::default();
        let parsed = crate::analysis::parse(&arena, &source);
        assert!(parsed.errors.is_empty());
        let (scopes, nodes) = crate::analysis::analyze(&parsed.program);
        let model = Model::new(&source, &parsed.program, &scopes, &nodes, Default::default())?;
        descriptor_registry_from_model(&model)?.ok_or(ProgramPropertiesError::Missing { what: "registry" })
    }
    #[test]
    fn consumes_ordered_alias_population_without_dormant_late_or_wrong_array_donations() {
        let found = registry(r#"
            let alias=r;
            alias[1]={name:"17",ctor:null,priority:true};
            alias[2]={name:17,ctor:null,priority:false};
            alias[3]={name:"17",ctor:null,priority:false};
            function dormant(){r[4]={name:"dormant",ctor:null};}
            let wrong=[]; wrong[4]={name:"wrong",ctor:null};
            r.forEach(d=>d.ctor);
            r[5]={name:"late",ctor:null};
        "#).unwrap();
        assert_eq!(found.length, 4);
        assert_eq!(found.elements.len(), 4);
        assert_eq!(found.elements[1].name, Some(LiteralValue::String("17".into())));
        assert_eq!(found.elements[2].name, Some(LiteralValue::Number(17.0)));
        assert_eq!(found.elements[3].name, found.elements[1].name);
        assert_eq!(found.elements[1].priority, Some(LiteralValue::Boolean(true)));
    }
    #[test]
    fn refuses_guarded_dynamic_and_unknown_mutations() {
        for body in [
            "if(flag) r[1]={name:'guarded',ctor:null};",
            "r[index]={name:'dynamic',ctor:null};",
            "r.push({name:'push',ctor:null});",
            "let a=r; a.splice(0,1);",
            "r.length=0;",
            "unknown(r);",
            "r.length++;",
            "delete r[0];",
            "r[0].name='mutated';",
            "function mutate(){r[1]={ctor:null};} mutate();",
        ] {
            assert!(registry(body).is_err(), "{body}");
        }
    }
}
