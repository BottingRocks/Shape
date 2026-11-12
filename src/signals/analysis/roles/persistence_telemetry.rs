//! Package `p14_persistence_telemetry` finders: the telemetry record and
//! aggregate it maintains, the ephemeral AES-GCM envelope those records are
//! stored in, the reports restored from storage, the browser-storage holders
//! and the ready-event dispatcher alias. See `roles/mod.rs` for the rules.
//!
//! # How this package decides
//!
//! The anchors are the storage API the program must call (`getItem`, `setItem`,
//! `removeItem`, `localStorage`), the field layout of the two telemetry records
//! (`lit`/`tlr`/`et`/`top`/`lvTop` and `aggregate.a.{c,ci,ts}`/`b`), the
//! envelope layout (`{a,b}` records under one storage key, the map under
//! another) and the AES-GCM call wiring (`generateKey`, `exportKey`,
//! `encrypt`, `importKey`, `decrypt`). A function is only claimed through a
//! role that its own body or its own promise/`map`/`findIndex` wiring proves.
//!
//! Both tracer shapes are read: a named formal and `arguments[N]` are one slot
//! under `bindings::Model`, `x.m(...)` and `x["m"].call(x, ...)` are one call,
//! and a callback may be an inline arrow or a hoisted declaration.
//!
//! # Delegated roles
//!
//! The AES-GCM string encryptor and its packed-blob decryptor are already named
//! by `semantic_names` (`encryptStringAesGcm`, `decryptPackedAesGcmBlob`) and
//! their single MAIN aliases too, so this package claims only the steps inside
//! those functions. The runtime init is already named
//! (`initializeSessionRuntime`), and the global object and `crypto.subtle`
//! holders have existing roles. Claiming any of them here would be a role
//! conflict, so they are read as anchors and never claimed.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};

use crate::analysis;

use crate::signals::extract::bindings::{call_arguments, object_members_of, Model};
use crate::signals::schema::{Confidence, ProgramPropertiesLimits};
use crate::signals::extract::reaching::expression_value_at_use;
use super::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};

// ---------------------------------------------------------------------------
// Roles and names
// ---------------------------------------------------------------------------

const TELEMETRY_STATE: &str = "persistence_telemetry_state";
const TELEMETRY_AGGREGATE: &str = "persistence_telemetry_aggregate";
const STORED_REPORTS: &str = "persistence_stored_reports";
const DECRYPTED_CACHE: &str = "persistence_decrypted_cache";
const NATIVE_DATE: &str = "persistence_native_date";
const LAST_SAMPLE_TIMESTAMP: &str = "persistence_last_sample_timestamp";
const INIT_TIMESTAMP: &str = "persistence_init_timestamp";
const EPOCH_BASELINE: &str = "persistence_epoch_baseline";
const HAS_LOCAL_STORAGE: &str = "persistence_has_local_storage";
const LOCAL_STORAGE_HANDLE: &str = "persistence_local_storage_handle";
const STORAGE_READY: &str = "persistence_storage_ready";
const LOCAL_STORAGE_KEY: &str = "persistence_local_storage_key";
const LOCAL_STORAGE_SEED: &str = "persistence_local_storage_seed";
const UNSET_HOSTNAME: &str = "persistence_unset_hostname";
const UNSET_PROTOCOL: &str = "persistence_unset_protocol";
const UNSET_PORT: &str = "persistence_unset_port";
const UNSET_REPORT_KEY: &str = "persistence_unset_report_key";
const READY_EVENT_ALIAS: &str = "persistence_ready_event_alias";

const RECORD_EVENT: &str = "persistence_record_event";
const STATE_PERSISTENCE: &str = "persistence_state_persistence";
const STATE_PERSISTENCE_ALIAS: &str = "persistence_state_persistence_alias";
const RESTORE_REPORTS: &str = "persistence_restore_reports";
const RESTORE_AGGREGATE: &str = "persistence_restore_aggregate";
const INIT_PRODUCER: &str = "persistence_init_visibility_producer";

const EVENT_ROW_MATCHER: &str = "persistence_event_row_matcher";
const STRINGIFY_ENVELOPE: &str = "persistence_stringify_envelope";
const STORE_ENCRYPTED_RECORD: &str = "persistence_store_encrypted_record";
const REPORT_RECORD_FAILURE: &str = "persistence_report_record_failure";
const MERGE_ENCRYPTED_STATE: &str = "persistence_merge_encrypted_state";
const REPORT_PERSIST_FAILURE: &str = "persistence_report_persist_failure";
const KEEP_EPHEMERAL_KEY: &str = "persistence_keep_ephemeral_key";
const ENCRYPT_PAYLOAD: &str = "persistence_encrypt_payload";
const EXPORT_EPHEMERAL_KEY: &str = "persistence_export_ephemeral_key";
const PACK_ENVELOPE: &str = "persistence_pack_envelope";
const PARSE_ENVELOPE_BYTE: &str = "persistence_parse_envelope_byte";
const DECRYPT_PAYLOAD: &str = "persistence_decrypt_payload";
const KEEP_PLAINTEXT: &str = "persistence_keep_plaintext";
const PARSE_DECRYPTED_JSON: &str = "persistence_parse_decrypted_json";
const DECRYPT_STORED_ENTRY: &str = "persistence_decrypt_stored_entry";
const COLLECT_SETTLED: &str = "persistence_collect_settled";
const RESTORE_SETTLED_ENTRY: &str = "persistence_restore_settled_entry";
const REPORT_RESTORE_FAILURE: &str = "persistence_report_restore_failure";
const APPLY_DECRYPTED_AGGREGATE: &str = "persistence_apply_decrypted_aggregate";
const REPORT_AGGREGATE_FAILURE: &str = "persistence_report_aggregate_failure";

const fn function(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::Function }
}

const fn alias(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::FunctionAlias }
}

const fn main(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::MainBinding }
}

/// Every role this package can claim, with the one name it receives.
pub(crate) const VOCABULARY: &[FinderRole] = &[
    main(TELEMETRY_STATE, "telemetryState"),
    main(TELEMETRY_AGGREGATE, "telemetryAggregate"),
    main(STORED_REPORTS, "storedReports"),
    main(DECRYPTED_CACHE, "decryptedStorageCache"),
    main(NATIVE_DATE, "nativeDate"),
    main(LAST_SAMPLE_TIMESTAMP, "lastSampleTimestampMs"),
    main(INIT_TIMESTAMP, "initTimestampMs"),
    main(EPOCH_BASELINE, "telemetryEpochBaselineMs"),
    main(HAS_LOCAL_STORAGE, "hasLocalStorage"),
    main(LOCAL_STORAGE_HANDLE, "localStorageHandle"),
    main(STORAGE_READY, "storagePersistenceReady"),
    main(LOCAL_STORAGE_KEY, "localStorageKey"),
    main(LOCAL_STORAGE_SEED, "localStorageSeedValue"),
    main(UNSET_HOSTNAME, "unsetHostnameComparison"),
    main(UNSET_PROTOCOL, "unsetProtocolComparison"),
    main(UNSET_PORT, "unsetPortComparison"),
    main(UNSET_REPORT_KEY, "undefinedReportKey"),
    main(READY_EVENT_ALIAS, "readyEventDispatcher"),
    function(RECORD_EVENT, "recordSignalEvent"),
    function(STATE_PERSISTENCE, "loadAndPersistTelemetryState"),
    alias(STATE_PERSISTENCE_ALIAS, "telemetryStatePersistence"),
    function(RESTORE_REPORTS, "restoreDecryptedStatRecords"),
    function(RESTORE_AGGREGATE, "restoreDecryptedTelemetryAggregate"),
    function(INIT_PRODUCER, "initVisibilitySignalProducer"),
    function(EVENT_ROW_MATCHER, "matchesTelemetryEventRow"),
    function(STRINGIFY_ENVELOPE, "stringifyEnvelopeBytes"),
    function(STORE_ENCRYPTED_RECORD, "storeEncryptedTelemetryRecord"),
    function(REPORT_RECORD_FAILURE, "reportTelemetryRecordFailure"),
    function(MERGE_ENCRYPTED_STATE, "mergeEncryptedTelemetryState"),
    function(REPORT_PERSIST_FAILURE, "reportTelemetryPersistFailure"),
    function(KEEP_EPHEMERAL_KEY, "keepEphemeralAesKey"),
    function(ENCRYPT_PAYLOAD, "encryptAesGcmPayload"),
    function(EXPORT_EPHEMERAL_KEY, "exportEphemeralAesKey"),
    function(PACK_ENVELOPE, "packAesGcmEnvelope"),
    function(PARSE_ENVELOPE_BYTE, "parseEnvelopeByte"),
    function(DECRYPT_PAYLOAD, "decryptAesGcmPayload"),
    function(KEEP_PLAINTEXT, "keepDecryptedBuffer"),
    function(PARSE_DECRYPTED_JSON, "parseDecryptedEnvelopeJson"),
    function(DECRYPT_STORED_ENTRY, "decryptStoredReportEntry"),
    function(COLLECT_SETTLED, "collectSettledReportRestores"),
    function(RESTORE_SETTLED_ENTRY, "restoreSettledReportEntry"),
    function(REPORT_RESTORE_FAILURE, "reportReportRestoreFailure"),
    function(APPLY_DECRYPTED_AGGREGATE, "applyDecryptedAggregate"),
    function(REPORT_AGGREGATE_FAILURE, "reportAggregateRestoreFailure"),
];

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let Ok(model) = Model::new(
        ctx.source,
        ctx.program,
        ctx.scoping,
        ctx.nodes,
        ProgramPropertiesLimits::default(),
    ) else {
        return Vec::new();
    };
    let mut scan = Scan::new(&model);
    scan.run();
    scan.finish()
}

// ---------------------------------------------------------------------------
// Claims
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Claims {
    claims: Vec<FinderClaim>,
    seen: BTreeSet<(&'static str, u32, u32)>,
}

impl Claims {
    /// A role names one thing in a program. Two proven spans for one role
    /// cannot be told apart, so no candidate is claimed.
    fn finish(self) -> Vec<FinderClaim> {
        let mut spans: BTreeMap<&'static str, usize> = BTreeMap::new();
        for (role, _, _) in &self.seen {
            *spans.entry(*role).or_default() += 1;
        }
        self.claims
            .into_iter()
            .filter(|claim| spans.get(claim.role()) == Some(&1))
            .collect()
    }

    fn function(
        &mut self,
        role: &'static str,
        span: Span,
        alias_role: Option<&'static str>,
        evidence: &[&str],
    ) {
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::Function {
                role,
                function_span: span,
                alias_role,
                confidence: Confidence::High,
                evidence: evidence.iter().map(|line| line.to_string()).collect(),
            });
        }
    }

    fn main(&mut self, role: &'static str, span: Span, evidence: &[&str]) {
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::MainBinding {
                role,
                identifier_span: span,
                confidence: Confidence::High,
                evidence: evidence.iter().map(|line| line.to_string()).collect(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Small AST readers
// ---------------------------------------------------------------------------

/// A call with `.call(...)` indirection removed, so `f.call(void 0, a)` reads as
/// `f(a)` and `r.m.call(r, a)` as `r.m(a)`; both tracer shapes use them.
struct Call<'a> {
    span: Span,
    callee: &'a Expression<'a>,
    method: Option<&'a str>,
    receiver: Option<&'a Expression<'a>>,
    args: Vec<&'a Expression<'a>>,
}

fn member_object<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expression {
        Expression::StaticMemberExpression(member) => Some(&member.object),
        Expression::ComputedMemberExpression(member) => Some(&member.object),
        _ => None,
    }
}

fn static_member_name<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression {
        Expression::StaticMemberExpression(member) => Some(member.property.name.as_str()),
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn member_of<'a>(expression: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a str)> {
    let expression = expression.without_parentheses();
    let name = static_member_name(expression)?;
    Some((member_object(expression)?, name))
}

fn target_member<'a>(
    target: &'a AssignmentTarget<'a>,
) -> Option<(&'a Expression<'a>, &'a str)> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.as_str()))
        }
        AssignmentTarget::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some((&member.object, literal.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

fn target_object<'a, 'e>(target: &'e AssignmentTarget<'a>) -> Option<&'e Expression<'a>> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => Some(&member.object),
        AssignmentTarget::ComputedMemberExpression(member) => Some(&member.object),
        _ => None,
    }
}

fn is_void(expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void
    )
}

fn normal_call<'a>(call: &'a CallExpression<'a>) -> Call<'a> {
    let span = call.span;
    let mut args = call_arguments(call);
    let callee = call.callee.without_parentheses();
    if static_member_name(callee) == Some("call") {
        if let Some(object) = member_object(callee) {
            let object = object.without_parentheses();
            if args.first().is_some_and(|first| is_void(first)) {
                args.remove(0);
                return plain_call(object, args, span);
            }
            if let (Some(method), false) = (static_member_name(object), args.is_empty()) {
                let receiver = args.remove(0);
                return Call {
                    span,
                    callee: object,
                    method: Some(method),
                    receiver: Some(receiver),
                    args,
                };
            }
        }
    }
    plain_call(callee, args, span)
}

fn plain_call<'a>(
    callee: &'a Expression<'a>,
    args: Vec<&'a Expression<'a>>,
    span: Span,
) -> Call<'a> {
    Call {
        span,
        callee,
        method: static_member_name(callee),
        receiver: member_object(callee).map(Expression::without_parentheses),
        args,
    }
}

fn string_value<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression.without_parentheses() {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

fn is_true(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::BooleanLiteral(literal) if literal.value)
}

fn is_false(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::BooleanLiteral(literal) if !literal.value)
}

fn is_null(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::NullLiteral(_))
}

fn is_function_kind(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
    )
}

// ---------------------------------------------------------------------------
// One function's own body
// ---------------------------------------------------------------------------

/// What one function's own body holds; nested functions are separate units.
#[derive(Default)]
struct Own<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    tries: Vec<&'a TryStatement<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    assigns: Vec<&'a AssignmentExpression<'a>>,
    declarators: Vec<&'a VariableDeclarator<'a>>,
    objects: Vec<&'a ObjectExpression<'a>>,
    /// `(member name, object)` of every static or string-keyed member access.
    members: Vec<(&'a str, &'a Expression<'a>)>,
    strings: Vec<&'a str>,
    /// `return <expr>` arguments and arrow expression bodies.
    returns: Vec<&'a Expression<'a>>,
    bare_returns: usize,
    /// `if (test)` and `test ? ... : ...` conditions.
    tests: Vec<&'a Expression<'a>>,
}

// ---------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------

struct Scan<'m, 's, 'a> {
    model: &'m Model<'s, 'a>,
    own: HashMap<NodeId, Own<'a>>,
    values: HashMap<SymbolId, Vec<&'a Expression<'a>>>,
    main_symbols: Vec<SymbolId>,
    sites: Vec<crate::signals::extract::bindings::FunctionSite>,
    claims: Claims,
}

impl<'m, 's, 'a> Scan<'m, 's, 'a> {
    fn new(model: &'m Model<'s, 'a>) -> Self {
        let sites = model.all_sites().to_vec();
        let main_symbols = model.main_symbols.clone();
        let mut scan = Self {
            model,
            own: HashMap::new(),
            values: HashMap::new(),
            main_symbols,
            sites,
            claims: Claims::default(),
        };
        scan.build();
        scan
    }

    fn build(&mut self) {
        let nodes = self.model.nodes();
        let mut owners: HashMap<NodeId, NodeId> = HashMap::new();
        for (id, node) in nodes.iter_enumerated() {
            let parent = nodes.parent_id(id);
            let owner = if parent == id {
                None
            } else if is_function_kind(nodes.kind(parent)) {
                Some(parent)
            } else {
                owners.get(&parent).copied()
            };
            if let Some(owner) = owner {
                owners.insert(id, owner);
            }
            let kind = node.kind();
            if let AstKind::ArrowFunctionExpression(arrow) = kind {
                if arrow.expression {
                    if let Some(Statement::ExpressionStatement(statement)) =
                        arrow.body.statements.first()
                    {
                        self.own.entry(id).or_default().returns.push(&statement.expression);
                    }
                }
            }
            self.record_values(kind);
            let Some(owner) = owner else {
                continue;
            };
            let entry = self.own.entry(owner).or_default();
            match kind {
                AstKind::CallExpression(call) => entry.calls.push(call),
                AstKind::TryStatement(try_statement) => entry.tries.push(try_statement),
                AstKind::NewExpression(new) => entry.news.push(new),
                AstKind::AssignmentExpression(assignment) => entry.assigns.push(assignment),
                AstKind::VariableDeclarator(declarator) => entry.declarators.push(declarator),
                AstKind::ObjectExpression(object) => entry.objects.push(object),
                AstKind::ReturnStatement(statement) => match &statement.argument {
                    Some(argument) => entry.returns.push(argument),
                    None => entry.bare_returns += 1,
                },
                AstKind::IfStatement(statement) => entry.tests.push(&statement.test),
                AstKind::ConditionalExpression(conditional) => entry.tests.push(&conditional.test),
                AstKind::StaticMemberExpression(member) => {
                    entry.members.push((member.property.name.as_str(), &member.object));
                }
                AstKind::ComputedMemberExpression(member) => {
                    if let Expression::StringLiteral(literal) = &member.expression {
                        entry.members.push((literal.value.as_str(), &member.object));
                    }
                }
                AstKind::StringLiteral(literal) => entry.strings.push(literal.value.as_str()),
                _ => {}
            }
        }
    }

    /// The declarator initializer and every plain `=` value of a binding.
    fn record_values(&mut self, kind: AstKind<'a>) {
        match kind {
            AstKind::VariableDeclarator(declarator) => {
                if let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                    (&declarator.id, declarator.init.as_ref())
                {
                    if let Some(symbol) = binding.symbol_id.get() {
                        self.values.entry(symbol).or_default().push(init);
                    }
                }
            }
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign =>
            {
                if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                    if let Some(symbol) =
                        analysis::identifier_symbol(identifier, self.model.scoping())
                    {
                        self.values.entry(symbol).or_default().push(&assignment.right);
                    }
                }
            }
            _ => {}
        }
    }

    fn run(&mut self) {
        let state = self.claim_telemetry_state();
        let aggregate = self.claim_telemetry_aggregate();
        let date = self.claim_native_date();
        self.claim_timestamps(date);
        self.claim_storage_presence_and_handle();
        self.claim_storage_ready();
        let _ = self.claim_storage_key_and_seed();
        let stored = self.claim_stored_reports();
        let cache = self.claim_decrypted_cache(stored);
        self.claim_unset_placeholders(stored);
        self.claim_ready_event_alias();
        self.claim_visibility_producer();

        let (Some(state), Some(aggregate)) = (state, aggregate) else {
            return;
        };

        let encryptor = self.aes_site(true);
        let decryptor = self.aes_site(false);

        let record = self.claim_record_event(state, aggregate, encryptor);
        // Base-first: the original persistence proof keeps every applied base
        // name.  The base search is evaluated without claiming first, because
        // a captured/foreign serializer argument must be refused before the
        // original proof records its claim.  The strict variant arm is only
        // consulted when the base search is empty or ambiguous, or when the
        // single base candidate resolves a captured serializer argument.
        let base_sites = match encryptor {
            Some(encryptor) => self.base_persistence_sites(state, encryptor),
            None => Vec::new(),
        };
        let persist = match base_sites.as_slice() {
            [site]
                if encryptor.is_some_and(|encryptor| {
                    self.base_persistence_argument_is_local(*site, state, encryptor)
                }) =>
            {
                self.claim_state_persistence(state, encryptor)
            }
            _ => self.claim_state_persistence_variant(state, encryptor),
        };
        let restore = self.claim_restore_reports(stored, decryptor);
        let aggregate_restore = self.claim_restore_aggregate(decryptor);

        if let Some(encryptor) = encryptor {
            self.claim_encryptor_steps(encryptor);
        }
        if let Some(decryptor) = decryptor {
            self.claim_decryptor_steps(decryptor);
        }
        if let Some(record) = record {
            self.claim_record_steps(record, aggregate, encryptor);
        }
        if let Some(persist) = persist {
            self.claim_persist_steps(persist, state, encryptor);
        }
        if let Some(restore) = restore {
            self.claim_restore_steps(restore, stored, cache, decryptor);
        }
        if let Some(aggregate_restore) = aggregate_restore {
            self.claim_aggregate_steps(aggregate_restore, aggregate, decryptor);
        }
    }

    fn finish(self) -> Vec<FinderClaim> {
        self.claims.finish()
    }

    // -----------------------------------------------------------------------
    // Readers
    // -----------------------------------------------------------------------

    fn own_of(&self, site: crate::signals::extract::bindings::FunctionSite) -> Option<&Own<'a>> {
        self.own.get(&site.node_id)
    }

    fn calls(&self, site: crate::signals::extract::bindings::FunctionSite) -> Vec<Call<'a>> {
        self.own_of(site)
            .map(|own| own.calls.iter().map(|call| normal_call(call)).collect())
            .unwrap_or_default()
    }

    fn symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        let expression = expression.without_parentheses();
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        analysis::identifier_symbol(identifier, self.model.scoping())
    }

    fn values_of(&self, symbol: SymbolId) -> &[&'a Expression<'a>] {
        self.values.get(&symbol).map(Vec::as_slice).unwrap_or(&[])
    }

    /// A binding's one value, when it is declared once and written once.
    fn single_value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        if !self.model.scoping().symbol_redeclarations(symbol).is_empty() {
            return None;
        }
        match self.values_of(symbol) {
            [value] => Some(*value),
            _ => None,
        }
    }

    fn origin(&self, expression: &'a Expression<'a>) -> &'a Expression<'a> {
        let mut current = expression.without_parentheses();
        for _ in 0..8 {
            let Some(symbol) = self.symbol(current) else {
                break;
            };
            let Some(value) = self.single_value(symbol) else {
                break;
            };
            let value = value.without_parentheses();
            if value.span() == current.span() {
                break;
            }
            current = value;
        }
        current
    }

    fn main_symbol(&self, expression: &'a Expression<'a>) -> Option<SymbolId> {
        let symbol = self.symbol(self.origin(expression))?;
        self.main_symbols.contains(&symbol).then_some(symbol)
    }

    /// Whether an expression denotes `symbol` directly or through one local
    /// alias whose single value is the binding.
    fn refers_to(&self, expression: &'a Expression<'a>, symbol: SymbolId) -> bool {
        self.symbol(expression) == Some(symbol)
            || self.symbol(self.origin(expression)) == Some(symbol)
    }

    /// Whether some function writes `local`'s value into `stored`
    /// (`stored = local`), the install step of the restored report map.
    fn installed_into_stored(&self, local: SymbolId, stored: SymbolId) -> bool {
        self.sites.iter().any(|site| {
            self.own_of(*site).is_some_and(|own| {
                own.assigns.iter().any(|assignment| {
                    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left
                    else {
                        return false;
                    };
                    analysis::identifier_symbol(identifier, self.model.scoping()) == Some(stored)
                        && self.refers_to(&assignment.right, local)
                })
            })
        })
    }

    fn is_global(&self, expression: &Expression<'a>, name: &str) -> bool {
        matches!(
            expression.without_parentheses(),
            Expression::Identifier(identifier)
                if identifier.name == name
                    && analysis::identifier_symbol(identifier, self.model.scoping()).is_none()
        )
    }

    fn function_of(
        &self,
        expression: &Expression<'a>,
    ) -> Option<crate::signals::extract::bindings::FunctionSite> {
        self.model.function_from_expression(expression.without_parentheses())
    }

    fn identifier(&self, symbol: SymbolId) -> Span {
        self.model.scoping().symbol_span(symbol)
    }

    fn binding_kind_is_variable(&self, symbol: SymbolId) -> bool {
        matches!(
            self.model
                .nodes()
                .get_node(self.model.scoping().symbol_declaration(symbol))
                .kind(),
            AstKind::VariableDeclarator(_)
        )
    }

    fn is_object_create_null(&self, expression: &'a Expression<'a>) -> bool {
        let Expression::CallExpression(call) = self.origin(expression) else {
            return false;
        };
        member_of(&call.callee).is_some_and(|(object, name)| {
            name == "create"
                && self.is_global(object, "Object")
                && matches!(
                    call.arguments.first().and_then(Argument::as_expression),
                    Some(Expression::NullLiteral(_))
                )
        })
    }

    fn is_global_method(
        &self,
        call: &Call<'a>,
        global: &str,
        method: &str,
    ) -> bool {
        call.method == Some(method)
            && call.receiver.is_some_and(|receiver| self.is_global(receiver, global))
    }

    fn is_json_call(&self, expression: &'a Expression<'a>, method: &str) -> bool {
        let Expression::CallExpression(call) = self.origin(expression) else {
            return false;
        };
        let call = normal_call(call);
        self.is_global_method(&call, "JSON", method)
    }

    fn member_path(&self, expression: &'a Expression<'a>) -> Vec<String> {
        let mut names = Vec::new();
        let mut current = self.origin(expression);
        for _ in 0..8 {
            match current {
                Expression::StaticMemberExpression(member) => {
                    names.push(member.property.name.to_string());
                    current = member.object.without_parentheses();
                }
                Expression::ComputedMemberExpression(member) => match &member.expression {
                    Expression::StringLiteral(literal) => {
                        names.push(literal.value.to_string());
                        current = member.object.without_parentheses();
                    }
                    _ => break,
                },
                _ => break,
            }
        }
        names.reverse();
        names
    }

    fn has_member_path(&self, expression: &'a Expression<'a>, path: &[&str]) -> bool {
        let mut operands = Vec::new();
        self.addition_operands(expression, &mut operands);
        operands.iter().any(|operand| {
            let names = self.member_path(operand);
            names.len() >= path.len()
                && names[names.len() - path.len()..]
                    .iter()
                    .zip(path.iter())
                    .all(|(found, wanted)| found == wanted)
        })
    }

    fn addition_operands(
        &self,
        expression: &'a Expression<'a>,
        out: &mut Vec<&'a Expression<'a>>,
    ) {
        match expression.without_parentheses() {
            Expression::BinaryExpression(binary)
                if binary.operator == BinaryOperator::Addition =>
            {
                self.addition_operands(&binary.left, out);
                self.addition_operands(&binary.right, out);
            }
            other => out.push(other),
        }
    }

    fn is_local_storage_member(&self, expression: &'a Expression<'a>) -> bool {
        matches!(
            member_of(expression),
            Some((_, "localStorage"))
        )
    }

    /// A MAIN binding that holds a `localStorage` handle: one write is the
    /// `.localStorage` member and another write is the null fallback.
    fn storage_handle(&self, symbol: SymbolId) -> bool {
        let values = self.values_of(symbol);
        values.iter().any(|value| self.is_local_storage_member(value))
            && values.iter().any(|value| is_null(value))
    }

    fn is_storage_receiver(&self, expression: &'a Expression<'a>) -> bool {
        let origin = self.origin(expression);
        if self.is_local_storage_member(origin) {
            return true;
        }
        self.symbol(origin)
            .is_some_and(|symbol| self.main_symbols.contains(&symbol) && self.storage_handle(symbol))
    }

    fn is_storage_method(&self, call: &Call<'a>, method: &str) -> bool {
        call.method == Some(method)
            && call.receiver.is_some_and(|receiver| self.is_storage_receiver(receiver))
    }

    fn site_gets_storage_item(&self, site: crate::signals::extract::bindings::FunctionSite) -> bool {
        self.calls(site).iter().any(|call| self.is_storage_method(call, "getItem"))
    }

    fn site_sets_storage_item(&self, site: crate::signals::extract::bindings::FunctionSite) -> bool {
        self.calls(site)
            .iter()
            .any(|call| self.is_storage_method(call, "setItem"))
    }

    fn site_mentions_in_test(
        &self,
        site: crate::signals::extract::bindings::FunctionSite,
        symbol: SymbolId,
    ) -> bool {
        self.own_of(site).is_some_and(|own| {
            own.tests
                .iter()
                .any(|test| self.expression_mentions_symbol(test, symbol))
        })
    }

    fn expression_mentions_symbol(&self, expression: &Expression<'a>, symbol: SymbolId) -> bool {
        match expression.without_parentheses() {
            Expression::Identifier(identifier) => {
                analysis::identifier_symbol(identifier, self.model.scoping()) == Some(symbol)
            }
            Expression::UnaryExpression(unary) => {
                self.expression_mentions_symbol(&unary.argument, symbol)
            }
            Expression::LogicalExpression(logical) => {
                self.expression_mentions_symbol(&logical.left, symbol)
                    || self.expression_mentions_symbol(&logical.right, symbol)
            }
            Expression::BinaryExpression(binary) => {
                self.expression_mentions_symbol(&binary.left, symbol)
                    || self.expression_mentions_symbol(&binary.right, symbol)
            }
            Expression::SequenceExpression(sequence) => {
                sequence.expressions.iter().any(|expression| {
                    self.expression_mentions_symbol(expression, symbol)
                })
            }
            Expression::ParenthesizedExpression(inner) => {
                self.expression_mentions_symbol(&inner.expression, symbol)
            }
            _ => false,
        }
    }

    fn reads_member(&self, site: crate::signals::extract::bindings::FunctionSite, name: &str) -> bool {
        self.own_of(site)
            .is_some_and(|own| own.members.iter().any(|(member, _)| *member == name))
    }

    fn reads_members(&self, site: crate::signals::extract::bindings::FunctionSite, names: &[&str]) -> bool {
        names.iter().all(|name| self.reads_member(site, name))
    }

    fn owns_call(&self, site: crate::signals::extract::bindings::FunctionSite, method: &str) -> bool {
        self.calls(site)
            .iter()
            .any(|call| call.method == Some(method))
    }

    fn owns_storage_get(&self, site: crate::signals::extract::bindings::FunctionSite) -> bool {
        self.calls(site)
            .iter()
            .any(|call| self.is_storage_method(call, "getItem"))
    }

    fn owns_json_parse(&self, site: crate::signals::extract::bindings::FunctionSite) -> bool {
        self.calls(site).iter().any(|call| self.is_global_method(call, "JSON", "parse"))
    }

    fn owns_object_keys_of(
        &self,
        site: crate::signals::extract::bindings::FunctionSite,
        symbol: SymbolId,
    ) -> bool {
        self.calls(site).iter().any(|call| {
            self.is_global_method(call, "Object", "keys")
                && call.args.first().is_some_and(|argument| self.refers_to(argument, symbol))
        })
    }

    fn json_stringify_of_member(
        &self,
        expression: &'a Expression<'a>,
        symbol: SymbolId,
    ) -> bool {
        let Expression::CallExpression(call) = self.origin(expression) else {
            return false;
        };
        let call = normal_call(call);
        self.is_global_method(&call, "JSON", "stringify")
            && call.args.first().is_some_and(|argument| {
                member_of(self.origin(argument))
                    .is_some_and(|(object, _)| self.refers_to(object, symbol))
            })
    }

    /// The persistence role has a deliberately local proof.  In particular,
    /// it must not be inferred from a bag of storage/JSON calls elsewhere in
    /// the function: the read and the later upload have to share the same
    /// storage receiver and key, with the parsed value and the encryptor call
    /// joined by their scoped bindings.
    fn state_persistence_protocol(
        &self,
        site: crate::signals::extract::bindings::FunctionSite,
        state: SymbolId,
        encryptor: crate::signals::extract::bindings::FunctionSite,
    ) -> bool {
        let Some(own) = self.own_of(site) else {
            return false;
        };
        if !self.zero_formals(site) || own.tries.len() != 2 {
            return false;
        }
        let tries = &own.tries;
        if tries.iter().any(|try_statement| {
            try_statement.finalizer.is_some() || try_statement.handler.is_none()
        }) {
            return false;
        }

        let storage_gets: Vec<Call<'a>> = self
            .calls(site)
            .into_iter()
            .filter(|call| self.is_storage_method(call, "getItem"))
            .collect();
        let parses: Vec<Call<'a>> = self
            .calls(site)
            .into_iter()
            .filter(|call| self.is_global_method(call, "JSON", "parse"))
            .collect();
        let [get] = storage_gets.as_slice() else {
            return false;
        };
        let [parse] = parses.as_slice() else {
            return false;
        };
        if get.args.len() != 1 || parse.args.len() != 1 {
            return false;
        }
        let Some((read_symbol, read_assignment)) = own.assigns.iter().find_map(|assignment| {
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                return None;
            };
            let value = self.origin(&assignment.right);
            let Expression::CallExpression(call) = value else {
                return None;
            };
            let candidate = normal_call(call);
            (candidate.span == get.span).then(|| {
                (
                    analysis::identifier_symbol(identifier, self.model.scoping()),
                    assignment,
                )
            })
        }) else {
            return false;
        };
        let Some(read_symbol) = read_symbol else {
            return false;
        };
        let Some((parsed_symbol, parse_assignment)) = own.assigns.iter().find_map(|assignment| {
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                return None;
            };
            let value = self.origin(&assignment.right);
            let Expression::CallExpression(call) = value else {
                return None;
            };
            let candidate = normal_call(call);
            (candidate.span == parse.span).then(|| {
                (
                    analysis::identifier_symbol(identifier, self.model.scoping()),
                    assignment,
                )
            })
        }) else {
            return false;
        };
        let Some(parsed_symbol) = parsed_symbol else {
            return false;
        };
        if !self.refers_to(parse.args[0], read_symbol)
            || !self.is_object_create_null_binding(parsed_symbol)
            || !self.in_span(get.span, tries[0].block.span)
            || !self.in_span(parse.span, tries[1].block.span)
            || !self.in_span(read_assignment.span, tries[0].block.span)
            || !self.in_span(parse_assignment.span, tries[1].block.span)
            || !self.has_read_null_guard(site, read_symbol)
            || !self.has_object_keys_length_guard(site, parsed_symbol, true)
            || !self.has_object_keys_length_guard(site, state, false)
        {
            return false;
        }
        let Some(get_receiver) = get.receiver else {
            return false;
        };
        let Some(first_sink) = self.error_report_call(site, tries[0]) else {
            return false;
        };
        let Some(second_sink) = self.error_report_call(site, tries[1]) else {
            return false;
        };
        if !self.same_scoped_value(first_sink.callee, second_sink.callee)
            || !self.is_captured_immutable_sink(site, first_sink.callee)
        {
            return false;
        }

        let starts: Vec<Call<'a>> = self
            .calls(site)
            .into_iter()
            .filter(|call| {
                self.function_of(call.callee)
                    .is_some_and(|callee| callee.span == encryptor.span)
                    && call.args.first().is_some_and(|argument| {
                        self.json_stringify_binding_of(site, argument, state, encryptor)
                    })
            })
            .collect();
        let [start] = starts.as_slice() else {
            return false;
        };

        // The promise result may be bound two ways in the corpus lowering:
        //   (a) the actual self-reassignment `temporary = encryptor(temporary)`;
        //   (b) a plain declarator `let result = encryptor(temporary)`.
        // Both must name exactly one result binding for this start call, so a
        // second/donor binding cannot smuggle in an unrelated `then` receiver.
        let mut result_symbols: Vec<SymbolId> = Vec::new();
        for assignment in own.assigns.iter() {
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                continue;
            };
            if self.origin(&assignment.right).span() != start.span {
                continue;
            }
            if let Some(symbol) = analysis::identifier_symbol(identifier, self.model.scoping()) {
                result_symbols.push(symbol);
            }
        }
        for declarator in own.declarators.iter() {
            let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                continue;
            };
            let Some(init) = declarator.init.as_ref() else {
                continue;
            };
            if self.origin(init).span() != start.span {
                continue;
            }
            if let Some(symbol) = binding.symbol_id.get() {
                result_symbols.push(symbol);
            }
        }
        let [result_symbol] = result_symbols.as_slice() else {
            return false;
        };
        let result_symbol = *result_symbol;
        let then_calls: Vec<Call<'a>> = self
            .calls(site)
            .into_iter()
            .filter(|call| {
                call.method == Some("then")
                    && call
                        .receiver
                        .is_some_and(|receiver| self.refers_to(receiver, result_symbol))
            })
            .collect();
        let [then_call] = then_calls.as_slice() else {
            return false;
        };
        let Some(upload) = then_call
            .args
            .first()
            .and_then(|argument| self.function_of(argument))
        else {
            return false;
        };
        let upload_sets: Vec<Call<'a>> = self
            .calls(upload)
            .into_iter()
            .filter(|call| self.is_storage_method(call, "setItem"))
            .collect();
        let [upload_set] = upload_sets.as_slice() else {
            return false;
        };
        let Some(upload_receiver) = upload_set.receiver else {
            return false;
        };
        if upload_set.args.len() != 2
            || !self.same_scoped_value(
                get.args[0],
                upload_set.args[0],
            )
            || !self.same_scoped_value(
                get_receiver,
                upload_receiver,
            )
            || !self.upload_serializes_parsed(upload_set.args[1], parsed_symbol)
        {
            return false;
        }
        let catch_calls: Vec<Call<'a>> = self
            .calls(site)
            .into_iter()
            .filter(|call| {
                call.method == Some("catch")
                    && call
                        .receiver
                        .is_some_and(|receiver| receiver.span() == then_call.span)
            })
            .collect();
        let [catch_call] = catch_calls.as_slice() else {
            return false;
        };
        let Some(tail) = catch_call
            .args
            .first()
            .and_then(|argument| self.function_of(argument))
        else {
            return false;
        };
        self.calls(tail).iter().any(|call| {
            self.same_scoped_value(call.callee, first_sink.callee)
        })
    }

    fn zero_formals(&self, site: crate::signals::extract::bindings::FunctionSite) -> bool {
        match self.model.nodes().kind(site.node_id) {
            AstKind::Function(function) => {
                function.params.items.is_empty() && function.params.rest.is_none()
            }
            AstKind::ArrowFunctionExpression(arrow) => {
                arrow.params.items.is_empty() && arrow.params.rest.is_none()
            }
            _ => false,
        }
    }

    fn in_span(&self, span: Span, outer: Span) -> bool {
        span.start >= outer.start && span.end <= outer.end
    }

    fn is_object_create_null_binding(&self, symbol: SymbolId) -> bool {
        self.values_of(symbol)
            .iter()
            .any(|value| self.is_object_create_null(value))
    }

    fn same_scoped_value(
        &self,
        left: &'a Expression<'a>,
        right: &'a Expression<'a>,
    ) -> bool {
        let left = self.origin(left);
        let right = self.origin(right);
        if left.span() == right.span() {
            return true;
        }
        if let (Some(left), Some(right)) = (self.symbol(left), self.symbol(right)) {
            return left == right;
        }
        if let (Some(left), Some(right)) = (member_of(left), member_of(right)) {
            return left.1 == right.1 && self.same_scoped_value(left.0, right.0);
        }
        match (string_value(left), string_value(right)) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        }
    }

    fn has_read_null_guard(
        &self,
        site: crate::signals::extract::bindings::FunctionSite,
        symbol: SymbolId,
    ) -> bool {
        self.own_of(site).is_some_and(|own| {
            own.tests.iter().any(|test| {
                self.contains_null_comparison(test, symbol)
            })
        })
    }

    fn contains_null_comparison(&self, expression: &'a Expression<'a>, symbol: SymbolId) -> bool {
        match expression.without_parentheses() {
            Expression::BinaryExpression(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Equality
                        | BinaryOperator::Inequality
                        | BinaryOperator::StrictEquality
                        | BinaryOperator::StrictInequality
                ) =>
            {
                (self.refers_to(&binary.left, symbol) && is_null(&binary.right))
                    || (self.refers_to(&binary.right, symbol) && is_null(&binary.left))
            }
            Expression::LogicalExpression(logical) => {
                self.contains_null_comparison(&logical.left, symbol)
                    || self.contains_null_comparison(&logical.right, symbol)
            }
            _ => false,
        }
    }

    fn is_object_keys_length_of(
        &self,
        expression: &'a Expression<'a>,
        symbol: SymbolId,
    ) -> bool {
        let Some((object, name)) = member_of(self.origin(expression)) else {
            return false;
        };
        if name != "length" {
            return false;
        }
        let Expression::CallExpression(call) = self.origin(object) else {
            return false;
        };
        let call = normal_call(call);
        self.is_global_method(&call, "Object", "keys")
            && call.args.first().is_some_and(|argument| {
                self.refers_to(argument, symbol)
            })
    }

    fn has_object_keys_length_guard(
        &self,
        site: crate::signals::extract::bindings::FunctionSite,
        symbol: SymbolId,
        parsed_guard: bool,
    ) -> bool {
        self.own_of(site).is_some_and(|own| {
            own.tests.iter().any(|test| {
                self.contains_keys_length_guard(test, symbol, parsed_guard)
            })
        })
    }

    fn contains_keys_length_guard(
        &self,
        expression: &'a Expression<'a>,
        symbol: SymbolId,
        parsed_guard: bool,
    ) -> bool {
        match expression.without_parentheses() {
            Expression::BinaryExpression(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::GreaterThan
                        | BinaryOperator::Inequality
                        | BinaryOperator::StrictInequality
                ) =>
            {
                let left = self.is_object_keys_length_of(&binary.left, symbol);
                let right = self.is_object_keys_length_of(&binary.right, symbol);
                if !(left || right) {
                    return false;
                }
                let other = if left { &binary.right } else { &binary.left };
                matches!(
                    other.without_parentheses(),
                    Expression::NumericLiteral(literal) if literal.value == 0.0
                ) && (parsed_guard == (binary.operator == BinaryOperator::GreaterThan))
            }
            Expression::LogicalExpression(logical) => {
                self.contains_keys_length_guard(&logical.left, symbol, parsed_guard)
                    || self.contains_keys_length_guard(&logical.right, symbol, parsed_guard)
            }
            _ => false,
        }
    }

    fn error_report_call(
        &self,
        site: crate::signals::extract::bindings::FunctionSite,
        try_statement: &TryStatement<'a>,
    ) -> Option<Call<'a>> {
        let catch = try_statement.handler.as_deref()?;
        let body = catch.body.span;
        let calls: Vec<Call<'a>> = self
            .calls(site)
            .into_iter()
            .filter(|call| self.in_span(call.span, body))
            .filter(|call| {
                call.args.first().is_some_and(|argument| {
                    self.error_record_argument(argument, body)
                })
            })
            .collect();
        (calls.len() == 1).then(|| calls.into_iter().next()).flatten()
    }

    fn error_record_argument(&self, expression: &'a Expression<'a>, catch: Span) -> bool {
        let Some(object) = self.object_for(expression) else {
            return false;
        };
        let members = object_members_of(object);
        let has_message = members.iter().any(|member| member.name == "message");
        let has_stack = members.iter().any(|member| member.name == "stack");
        if has_message && has_stack {
            return true;
        }
        if !has_message {
            return false;
        }
        let Some(symbol) = self.symbol(expression) else {
            return false;
        };
        self.sites.iter().any(|site| {
            self.own_of(*site).is_some_and(|own| {
                own.assigns.iter().any(|assignment| {
                    self.in_span(assignment.span, catch)
                        && target_member(&assignment.left).is_some_and(|(target, name)| {
                            name == "stack" && self.refers_to(target, symbol)
                        })
                })
            })
        })
    }

    fn object_for(&self, expression: &'a Expression<'a>) -> Option<&'a ObjectExpression<'a>> {
        match self.origin(expression) {
            Expression::ObjectExpression(object) => Some(object),
            _ => None,
        }
    }

    fn json_stringify_of(
        &self,
        expression: &'a Expression<'a>,
        symbol: SymbolId,
    ) -> bool {
        let Expression::CallExpression(call) = self.origin(expression) else {
            return false;
        };
        let call = normal_call(call);
        self.is_global_method(&call, "JSON", "stringify")
            && call.args.first().is_some_and(|argument| self.refers_to(argument, symbol))
    }

    fn is_custom_event_dispatcher(&self, site: crate::signals::extract::bindings::FunctionSite) -> bool {
        let calls = self.calls(site);
        if calls.len() != 3 {
            return false;
        }
        let creates = calls.iter().any(|call| {
            call.method == Some("createEvent")
                && call.args.first().and_then(|arg| string_value(arg)) == Some("CustomEvent")
        });
        let initializes = calls.iter().any(|call| call.method == Some("initCustomEvent"));
        let dispatches = calls.iter().any(|call| {
            self.is_global(call.callee, "dispatchEvent") || call.method == Some("dispatchEvent")
        });
        creates && initializes && dispatches
    }

    fn returns_slot(&self, site: crate::signals::extract::bindings::FunctionSite, slot: u32) -> bool {
        let Some(own) = self.own_of(site) else {
            return false;
        };
        if own.bare_returns != 0 || !own.calls.is_empty() || !own.assigns.is_empty() {
            return false;
        }
        match own.returns.as_slice() {
            [value] => self.model.site_slot_value(value, site) == Some(slot),
            _ => false,
        }
    }

    fn returns_error_report(&self, site: crate::signals::extract::bindings::FunctionSite) -> bool {
        let Some(own) = self.own_of(site) else {
            return false;
        };
        let [value] = own.returns.as_slice() else {
            return false;
        };
        let Expression::CallExpression(call) = self.origin(value) else {
            return false;
        };
        let call = normal_call(call);
        call.args.first().is_some_and(|argument| {
            let Expression::ObjectExpression(object) = self.origin(argument) else {
                return false;
            };
            let keys: BTreeSet<String> =
                object_members_of(object).into_iter().map(|member| member.name).collect();
            keys.contains("message") && keys.contains("stack")
        })
    }

    // -----------------------------------------------------------------------
    // Recognizers: MAIN state
    // -----------------------------------------------------------------------

    fn claim_telemetry_state(&mut self) -> Option<SymbolId> {
        let mut found = Vec::new();
        for &symbol in &self.main_symbols.clone() {
            let Some(value) = self.single_value(symbol) else {
                continue;
            };
            let Expression::ObjectExpression(object) = self.origin(value) else {
                continue;
            };
            let keys: BTreeSet<String> =
                object_members_of(object).into_iter().map(|member| member.name).collect();
            if ["et", "lit", "tlr", "top", "lvTop"].iter().all(|key| keys.contains(*key)) {
                found.push(symbol);
            }
        }
        let [symbol] = found.as_slice() else {
            return None;
        };
        self.claims.main(
            TELEMETRY_STATE,
            self.identifier(*symbol),
            &["MAIN object literal with telemetry counters, an event list (et) and the elapsed-time fields"],
        );
        Some(*symbol)
    }

    fn claim_telemetry_aggregate(&mut self) -> Option<SymbolId> {
        let mut found = Vec::new();
        for &symbol in &self.main_symbols.clone() {
            let Some(value) = self.single_value(symbol) else {
                continue;
            };
            let Expression::ObjectExpression(object) = self.origin(value) else {
                continue;
            };
            let members = object_members_of(object);
            let has_summary = members.iter().any(|member| {
                member.name == "a"
                    && matches!(self.origin(member.value), Expression::ObjectExpression(inner)
                        if object_members_of(inner).iter().any(|field| field.name == "c")
                            && object_members_of(inner).iter().any(|field| field.name == "ci")
                            && object_members_of(inner).iter().any(|field| field.name == "ts"))
            });
            let has_total = members.iter().any(|member| member.name == "b");
            if has_summary && has_total {
                found.push(symbol);
            }
        }
        let [symbol] = found.as_slice() else {
            return None;
        };
        self.claims.main(
            TELEMETRY_AGGREGATE,
            self.identifier(*symbol),
            &["MAIN aggregate record: a nested count/interval summary (a.c, a.ci, a.ts) and a total (b)"],
        );
        Some(*symbol)
    }

    fn claim_native_date(&mut self) -> Option<SymbolId> {
        let mut found = Vec::new();
        for &symbol in &self.main_symbols.clone() {
            let Some(value) = self.single_value(symbol) else {
                continue;
            };
            if member_of(self.origin(value)).is_some_and(|(_, name)| name == "Date") {
                found.push(symbol);
            }
        }
        let [symbol] = found.as_slice() else {
            return None;
        };
        let constructed = self
            .sites
            .iter()
            .any(|site| self.own_of(*site).is_some_and(|own| {
                own.news.iter().any(|new| self.refers_to(&new.callee, *symbol))
            }));
        if !constructed {
            return None;
        }
        self.claims.main(
            NATIVE_DATE,
            self.identifier(*symbol),
            &["MAIN binding holding the host Date constructor; the timestamp readings construct it"],
        );
        Some(*symbol)
    }

    fn number_of_new_date(
        &self,
        value: &'a Expression<'a>,
        date: SymbolId,
    ) -> Option<&'a NewExpression<'a>> {
        let Expression::CallExpression(call) = self.origin(value) else {
            return None;
        };
        let call = normal_call(call);
        if !self.is_global(call.callee, "Number") {
            return None;
        }
        let [argument] = call.args.as_slice() else {
            return None;
        };
        let Expression::NewExpression(new) = self.origin(argument) else {
            return None;
        };
        let callee = self.origin(&new.callee);
        let is_date = self.symbol(callee) == Some(date)
            || member_of(callee).is_some_and(|(_, name)| name == "Date");
        is_date.then_some(new)
    }

    fn claim_timestamps(&mut self, date: Option<SymbolId>) {
        let Some(date) = date else {
            return;
        };
        let mut last = Vec::new();
        let mut init = Vec::new();
        let mut epoch = Vec::new();
        for &symbol in &self.main_symbols.clone() {
            let values = self.values_of(symbol);
            if values.is_empty() {
                continue;
            }
            let dates: Vec<&NewExpression> = values
                .iter()
                .filter_map(|value| self.number_of_new_date(value, date))
                .collect();
            if dates.len() != values.len() {
                continue;
            }
            if values.len() >= 2 {
                last.push(symbol);
            } else if dates[0].arguments.is_empty() {
                init.push(symbol);
            } else if dates[0].arguments.len() == 1
                && dates[0]
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .is_some_and(|argument| string_value(argument).is_some())
            {
                epoch.push(symbol);
            }
        }
        if let [symbol] = last.as_slice() {
            self.claims.main(
                LAST_SAMPLE_TIMESTAMP,
                self.identifier(*symbol),
                &["mutable MAIN timestamp: Number(new Date()) written again when a sample is recorded"],
            );
        }
        if let [symbol] = init.as_slice() {
            self.claims.main(
                INIT_TIMESTAMP,
                self.identifier(*symbol),
                &["MAIN timestamp: Number(new Date()) with no configured date argument"],
            );
        }
        if let [symbol] = epoch.as_slice() {
            self.claims.main(
                EPOCH_BASELINE,
                self.identifier(*symbol),
                &["MAIN epoch reading: Number(new Date(<string>)) configured with a date literal and read as the elapsed-time baseline"],
            );
        }
    }

    fn claim_storage_presence_and_handle(&mut self) {
        let mut presence = Vec::new();
        let mut handle = Vec::new();
        for &symbol in &self.main_symbols.clone() {
            if !self.binding_kind_is_variable(symbol) {
                continue;
            }
            if let Some(value) = self.single_value(symbol) {
                if let Expression::BinaryExpression(binary) = self.origin(value) {
                    if binary.operator == BinaryOperator::In
                        && string_value(&binary.left) == Some("localStorage")
                    {
                        presence.push(symbol);
                    }
                }
            }
            if self.storage_handle(symbol) {
                handle.push(symbol);
            }
        }
        if let [symbol] = presence.as_slice() {
            self.claims.main(
                HAS_LOCAL_STORAGE,
                self.identifier(*symbol),
                &["MAIN presence probe: the `in` test of the localStorage member on the global object"],
            );
        }
        if let [symbol] = handle.as_slice() {
            self.claims.main(
                LOCAL_STORAGE_HANDLE,
                self.identifier(*symbol),
                &["MAIN storage handle: one write is the global localStorage member, the other its null fallback"],
            );
        }
    }

    fn claim_storage_ready(&mut self) {
        let mut found = Vec::new();
        for &symbol in &self.main_symbols.clone() {
            if !self.binding_kind_is_variable(symbol) {
                continue;
            }
            let values = self.values_of(symbol);
            if values.len() < 2
                || !values.iter().all(|value| {
                    is_true(value) || is_false(value)
                })
                || !values.iter().any(|value| is_true(value))
                || !values.iter().any(|value| is_false(value))
            {
                continue;
            }
            let read_in_producer = self.sites.iter().any(|site| {
                self.site_gets_storage_item(*site) && self.site_mentions_in_test(*site, symbol)
            });
            let written_in_initializer = self
                .model
                .scoping()
                .get_resolved_references(symbol)
                .filter(|reference| reference.is_write())
                .filter_map(|reference| self.model.owning_site(reference.node_id()))
                .any(|site| self.site_sets_storage_item(site));
            if read_in_producer && written_in_initializer {
                found.push(symbol);
            }
        }
        if let [symbol] = found.as_slice() {
            self.claims.main(
                STORAGE_READY,
                self.identifier(*symbol),
                &["MAIN boolean flag: cleared/raised around the storage writes and read as a producer gate"],
            );
        }
    }

    fn claim_storage_key_and_seed(&mut self) -> (Option<SymbolId>, Option<SymbolId>) {
        let mut keys: BTreeSet<SymbolId> = BTreeSet::new();
        let mut set_items: Vec<(SymbolId, &'a Expression<'a>)> = Vec::new();
        for site in self.sites.clone() {
            for call in self.calls(site) {
                if !self.is_storage_method(&call, "setItem") {
                    continue;
                }
                let [key, value, ..] = call.args.as_slice() else {
                    continue;
                };
                let Some(key_symbol) = self.main_symbol(key) else {
                    continue;
                };
                let has_persistent_key = self
                    .values_of(key_symbol)
                    .iter()
                    .any(|value| self.has_member_path(value, &["persistentId", "key"]));
                if has_persistent_key {
                    keys.insert(key_symbol);
                    set_items.push((key_symbol, value));
                }
            }
        }
        let mut keys = keys.into_iter();
        let (Some(key), None) = (keys.next(), keys.next()) else {
            return (None, None);
        };
        let key = &key;
        self.claims.main(
            LOCAL_STORAGE_KEY,
            self.identifier(*key),
            &["MAIN storage key: concatenates the persistent-id key and is passed as the setItem name"],
        );
        let seeds: BTreeSet<SymbolId> = set_items
            .iter()
            .filter(|(stored_key, _)| stored_key == key)
            .filter_map(|(_, value)| self.main_symbol(value))
            .collect();
        let mut seeds = seeds.into_iter();
        let (Some(seed), None) = (seeds.next(), seeds.next()) else {
            return (Some(*key), None);
        };
        if seed == *key {
            return (Some(*key), None);
        }
        self.claims.main(
            LOCAL_STORAGE_SEED,
            self.identifier(seed),
            &["MAIN storage value: the timestamp/entropy string written under the persistent storage key"],
        );
        (Some(*key), Some(seed))
    }

    fn claim_stored_reports(&mut self) -> Option<SymbolId> {
        let mut found = Vec::new();
        for &symbol in &self.main_symbols.clone() {
            let values = self.values_of(symbol);
            let null_prototype = values.iter().any(|value| self.is_object_create_null(value));
            let parsed = values.iter().any(|value| self.is_json_call(value, "parse"));
            let folded = values.iter().any(|value| {
                let Expression::CallExpression(call) = self.origin(value) else {
                    return false;
                };
                let call = normal_call(call);
                self.is_global_method(&call, "Object", "fromEntries")
            });
            if null_prototype && parsed && folded {
                found.push(symbol);
            }
        }
        let [symbol] = found.as_slice() else {
            return None;
        };
        self.claims.main(
            STORED_REPORTS,
            self.identifier(*symbol),
            &["MAIN report map: null-prototype object replaced by JSON.parse of the stored map and folded to its last entries"],
        );
        Some(*symbol)
    }

    fn claim_decrypted_cache(&mut self, stored: Option<SymbolId>) -> Option<SymbolId> {
        let mut found = Vec::new();
        for &symbol in &self.main_symbols.clone() {
            if Some(symbol) == stored {
                continue;
            }
            let values = self.values_of(symbol);
            if !values.iter().any(|value| self.is_object_create_null(value)) {
                continue;
            }
            let settlement_write = self.sites.iter().any(|site| {
                let reads_status = self.reads_member(*site, "status")
                    && self.own_of(*site).is_some_and(|own| {
                        own.strings.contains(&"fulfilled")
                    });
                reads_status
                    && self.own_of(*site).is_some_and(|own| {
                        own.assigns.iter().any(|assignment| {
                            target_object(&assignment.left)
                                .is_some_and(|object| self.refers_to(object, symbol))
                                && member_of(self.origin(&assignment.right))
                                    .is_some_and(|(_, name)| name == "value")
                        })
                    })
            });
            if settlement_write {
                found.push(symbol);
            }
        }
        let [symbol] = found.as_slice() else {
            return None;
        };
        self.claims.main(
            DECRYPTED_CACHE,
            self.identifier(*symbol),
            &["MAIN null-prototype cache: a settled decrypt result's value is stored under the decoded report key"],
        );
        Some(*symbol)
    }

    fn claim_unset_placeholders(&mut self, stored: Option<SymbolId>) {
        for &symbol in &self.main_symbols.clone() {
            if !self.binding_kind_is_variable(symbol) || !self.values_of(symbol).is_empty() {
                continue;
            }
            let mut roles: BTreeSet<&'static str> = BTreeSet::new();
            for reference in self.model.scoping().get_resolved_references(symbol) {
                if reference.is_write() {
                    continue;
                }
                let node = reference.node_id();
                let parent = self.model.nodes().parent_id(node);
                let span = self.model.nodes().get_node(node).span();
                match self.model.nodes().kind(parent) {
                    AstKind::BinaryExpression(binary)
                        if matches!(
                            binary.operator,
                            BinaryOperator::StrictEquality
                                | BinaryOperator::StrictInequality
                                | BinaryOperator::Equality
                                | BinaryOperator::Inequality
                        ) =>
                    {
                        let other = if binary.left.span().start == span.start {
                            &binary.right
                        } else if binary.right.span().start == span.start {
                            &binary.left
                        } else {
                            continue;
                        };
                        match member_of(other) {
                            Some((_, "hostname")) => {
                                roles.insert(UNSET_HOSTNAME);
                            }
                            Some((_, "protocol")) => {
                                roles.insert(UNSET_PROTOCOL);
                            }
                            Some((_, "port")) => {
                                roles.insert(UNSET_PORT);
                            }
                            _ => {}
                        }
                    }
                    AstKind::ComputedMemberExpression(member)
                        if member.expression.span() == span =>
                    {
                        let Some(stored) = stored else {
                            continue;
                        };
                        let assignment = self.model.nodes().parent_id(parent);
                        let AstKind::AssignmentExpression(assignment) =
                            self.model.nodes().kind(assignment)
                        else {
                            continue;
                        };
                        let AssignmentTarget::ComputedMemberExpression(target) = &assignment.left
                        else {
                            continue;
                        };
                        if target.span == member.span {
                            let installed = target_object(&assignment.left)
                                .and_then(|object| self.symbol(object))
                                .is_some_and(|local| self.installed_into_stored(local, stored));
                            if self.refers_to(&target.object, stored) || installed {
                                roles.insert(UNSET_REPORT_KEY);
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let [role] = roles.iter().copied().collect::<Vec<_>>().as_slice() {
                self.claims.main(
                    role,
                    self.identifier(symbol),
                    &["MAIN binding declared without initializer and never assigned: every read observes undefined; named from the slot it fills"],
                );
            }
        }
    }

    fn claim_ready_event_alias(&mut self) {
        let dispatchers: Vec<crate::signals::extract::bindings::FunctionSite> = self
            .sites
            .clone()
            .into_iter()
            .filter(|site| self.is_custom_event_dispatcher(*site))
            .collect();

        for &symbol in &self.main_symbols.clone() {
            let Some(value) = self.single_value(symbol) else {
                continue;
            };
            let Some(function) = self.function_of(value) else {
                continue;
            };
            if dispatchers.iter().any(|dispatcher| dispatcher.span == function.span) {
                self.claims.main(
                    READY_EVENT_ALIAS,
                    self.identifier(symbol),
                    &["MAIN alias of the CustomEvent dispatcher registered on the scheduler onEmpty hook"],
                );
            }
        }
    }

    fn claim_visibility_producer(&mut self) {
        for site in self.sites.clone() {
            let calls = self.calls(site);
            let has = |name: &str| {
                calls.iter().any(|call| {
                    call.method == Some("addEventListener")
                        && call.args.first().and_then(|arg| string_value(arg)) == Some(name)
                })
            };
            if has("visibilitychange") && has("wheel") {
                self.claims.function(
                    INIT_PRODUCER,
                    site.span,
                    None,
                    &["registers the visibilitychange and wheel listeners of the browser telemetry producer"],
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Recognizers: functions
    // -----------------------------------------------------------------------

    /// Methods and strings of `site`'s own body plus every function it hands
    /// to a call as a callback.
    fn deep_calls(
        &self,
        site: crate::signals::extract::bindings::FunctionSite,
        calls: &mut BTreeSet<&'a str>,
        strings: &mut BTreeSet<&'a str>,
        visited: &mut BTreeSet<(u32, u32)>,
        depth: usize,
    ) {
        if depth > 8 || !visited.insert((site.span.start, site.span.end)) {
            return;
        }
        let Some(own) = self.own_of(site) else {
            return;
        };
        strings.extend(own.strings.iter().copied());
        for call in own.calls.iter().map(|call| normal_call(call)) {
            if let Some(method) = call.method {
                calls.insert(method);
            }
            for argument in &call.args {
                if let Some(callback) = self.function_of(argument) {
                    self.deep_calls(callback, calls, strings, visited, depth + 1);
                }
            }
        }
    }

    fn aes_site(&self, encrypt: bool) -> Option<crate::signals::extract::bindings::FunctionSite> {
        let mut found = Vec::new();
        for site in self.sites.clone() {
            let mut calls = BTreeSet::new();
            let mut strings = BTreeSet::new();
            let mut visited = BTreeSet::new();
            self.deep_calls(site, &mut calls, &mut strings, &mut visited, 0);
            let matches = if encrypt {
                strings.contains("AES-GCM")
                    && calls.contains("generateKey")
                    && calls.contains("exportKey")
                    && calls.contains("encrypt")
            } else {
                strings.contains("AES-GCM")
                    && calls.contains("importKey")
                    && calls.contains("decrypt")
            };
            if matches {
                found.push(site);
            }
        }
        match found.as_slice() {
            [site] => Some(*site),
            _ => None,
        }
    }

    fn claim_record_event(
        &mut self,
        state: SymbolId,
        aggregate: SymbolId,
        encryptor: Option<crate::signals::extract::bindings::FunctionSite>,
    ) -> Option<crate::signals::extract::bindings::FunctionSite> {
        let encryptor = encryptor?;
        let mut found = Vec::new();
        for site in self.sites.clone() {
            let Some(own) = self.own_of(site) else {
                continue;
            };
            let fields: BTreeSet<&str> = own
                .assigns
                .iter()
                .filter_map(|assignment| {
                    target_member(&assignment.left).and_then(|(object, name)| {
                        self.refers_to(object, state).then_some(name)
                    })
                })
                .collect();
            if fields.len() < 4 || !fields.contains("et") {
                continue;
            }
            let encrypted_aggregate = own.calls.iter().map(|call| normal_call(call)).any(|call| {
                self.function_of(call.callee).is_some_and(|callee| callee.span == encryptor.span)
                    && call.args.first().is_some_and(|argument| {
                        self.json_stringify_of_member(argument, aggregate)
                    })
            });
            if !encrypted_aggregate {
                continue;
            }
            if self.find_index_callback(site).is_none() {
                continue;
            }
            found.push(site);
        }
        let [site] = found.as_slice() else {
            return None;
        };
        self.claims.function(
            RECORD_EVENT,
            site.span,
            None,
            &[
                "writes the telemetry record's event list and counters, folds them into the aggregate, and hands the aggregate to the AES-GCM encryptor and the storage persister",
                "its findIndex callback compares the p1..p4 event-row fields",
            ],
        );
        Some(*site)
    }

    fn find_index_callback(
        &self,
        site: crate::signals::extract::bindings::FunctionSite,
    ) -> Option<crate::signals::extract::bindings::FunctionSite> {
        let mut found = Vec::new();
        for call in self.calls(site) {
            if call.method != Some("findIndex") {
                continue;
            }
            let Some(callback) = call.args.first().and_then(|arg| self.function_of(arg)) else {
                continue;
            };
            if self.reads_members(callback, &["p1", "p2", "p3", "p4"]) {
                found.push(callback);
            }
        }
        Self::only_site(found)
    }

    fn claim_state_persistence(
        &mut self,
        state: SymbolId,
        encryptor: Option<crate::signals::extract::bindings::FunctionSite>,
    ) -> Option<crate::signals::extract::bindings::FunctionSite> {
        let encryptor = encryptor?;
        let mut found = Vec::new();
        for site in self.sites.clone() {
            if !self.owns_storage_get(site) || !self.owns_json_parse(site) {
                continue;
            }
            if !self.owns_object_keys_of(site, state) {
                continue;
            }
            let encrypts_state = self.calls(site).iter().any(|call| {
                self.function_of(call.callee).is_some_and(|callee| callee.span == encryptor.span)
                    && call.args.first().is_some_and(|argument| {
                        self.json_stringify_of(argument, state)
                    })
            });
            if encrypts_state {
                found.push(site);
            }
        }
        let [site] = found.as_slice() else {
            return None;
        };
        self.claims.function(
            STATE_PERSISTENCE,
            site.span,
            Some(STATE_PERSISTENCE_ALIAS),
            &[
                "reads the stored report map, parses it, and when the telemetry record is non-empty hands it to the AES-GCM encryptor and writes the map back",
            ],
        );
        Some(*site)
    }

    /// Non-claiming mirror of the original `claim_state_persistence` search.
    ///
    /// The original proof is kept byte-for-byte, so its search is duplicated
    /// here only to decide whether the base candidate is allowed to claim.  It
    /// applies exactly the same three own-body conditions and the same direct
    /// `JSON.stringify(state)` encryptor test; it never claims a role.
    fn base_persistence_sites(
        &self,
        state: SymbolId,
        encryptor: crate::signals::extract::bindings::FunctionSite,
    ) -> Vec<crate::signals::extract::bindings::FunctionSite> {
        let mut found = Vec::new();
        for site in self.sites.clone() {
            if !self.owns_storage_get(site) || !self.owns_json_parse(site) {
                continue;
            }
            if !self.owns_object_keys_of(site, state) {
                continue;
            }
            let encrypts_state = self.calls(site).iter().any(|call| {
                self.function_of(call.callee).is_some_and(|callee| callee.span == encryptor.span)
                    && call.args.first().is_some_and(|argument| {
                        self.json_stringify_of(argument, state)
                    })
            });
            if encrypts_state {
                found.push(site);
            }
        }
        found
    }

    /// Additive lexical-variant arm for the same persistence role.
    ///
    /// The original proof above is kept byte-for-byte so every applied
    /// `loadAndPersistTelemetryState` base body keeps its name.  This arm is
    /// only consulted when the base search finds no allowed candidate: the
    /// original search is empty/ambiguous, or its single candidate resolves a
    /// captured serializer argument.  A matching site uses the
    /// reused-serialization lowering: the serializer temporary is defined
    /// once, optionally self-reassigned through the captured encryptor, and
    /// the promise result is bound either by that self-reassignment or by a
    /// declarator.  It is deliberately stricter about the upload/error-sink
    /// identity than the base proof because the variant shapes are newly
    /// accepted, not grandfathered.
    fn claim_state_persistence_variant(
        &mut self,
        state: SymbolId,
        encryptor: Option<crate::signals::extract::bindings::FunctionSite>,
    ) -> Option<crate::signals::extract::bindings::FunctionSite> {
        let encryptor = encryptor?;
        let mut found = Vec::new();
        for site in self.sites.clone() {
            if self.state_persistence_protocol(site, state, encryptor) {
                found.push(site);
            }
        }
        let [site] = found.as_slice() else {
            return None;
        };
        self.claims.function(
            STATE_PERSISTENCE,
            site.span,
            Some(STATE_PERSISTENCE_ALIAS),
            &[
                "reads the stored report map, parses it, and when the telemetry record is non-empty hands it to the AES-GCM encryptor and writes the map back",
            ],
        );
        Some(*site)
    }

    /// Whether the base proof's stringify value is a direct `JSON.stringify`
    /// argument or a serializer local declared by the persistence site itself.
    ///
    /// This is not a new recognition rule: it only refuses a captured/foreign
    /// serializer temporary that the base value-bag lookup would otherwise
    /// resolve through its single value.  All 83 applied baseline bodies bind
    /// the encryptor argument directly or through a site-owned temporary, so
    /// this guard is true for every applied name (see the static audit in the
    /// revision-003 packet).
    fn base_persistence_argument_is_local(
        &self,
        site: crate::signals::extract::bindings::FunctionSite,
        state: SymbolId,
        encryptor: crate::signals::extract::bindings::FunctionSite,
    ) -> bool {
        let mut matched = false;
        for call in self.calls(site) {
            if !self
                .function_of(call.callee)
                .is_some_and(|callee| callee.span == encryptor.span)
            {
                continue;
            }
            let Some(argument) = call.args.first() else {
                continue;
            };
            if !self.json_stringify_of(argument, state) {
                continue;
            }
            matched = true;
            if let Some(local) = self.symbol(argument) {
                let declaration = self.model.scoping().symbol_declaration(local);
                if self.model.owning_site(declaration).map(|owner| owner.node_id)
                    != Some(site.node_id)
                {
                    return false;
                }
            }
        }
        matched
    }

    fn claim_restore_reports(
        &mut self,
        stored: Option<SymbolId>,
        decryptor: Option<crate::signals::extract::bindings::FunctionSite>,
    ) -> Option<crate::signals::extract::bindings::FunctionSite> {
        let (stored, decryptor) = (stored?, decryptor?);
        let mut found = Vec::new();
        for site in self.sites.clone() {
            if !self.owns_call(site, "allSettled") {
                continue;
            }
            let decrypts_entries = self.calls(site).iter().any(|call| {
                if call.method != Some("map") {
                    return false;
                }
                let Some(receiver) = call.receiver else {
                    return false;
                };
                if !self.is_object_keys_of(receiver, stored) {
                    return false;
                }
                call.args
                    .first()
                    .and_then(|arg| self.function_of(arg))
                    .is_some_and(|callback| self.calls(callback).iter().any(|inner| {
                        self.function_of(inner.callee)
                            .is_some_and(|callee| callee.span == decryptor.span)
                    }))
            });
            if decrypts_entries {
                found.push(site);
            }
        }
        let [site] = found.as_slice() else {
            return None;
        };
        self.claims.function(
            RESTORE_REPORTS,
            site.span,
            None,
            &[
                "reads the stored map and decrypts one stored report per key, then collects the settlements into the decrypted cache",
            ],
        );
        Some(*site)
    }

    fn is_object_keys_of(&self, expression: &'a Expression<'a>, symbol: SymbolId) -> bool {
        let Expression::CallExpression(call) = self.origin(expression) else {
            return false;
        };
        let call = normal_call(call);
        self.is_global_method(&call, "Object", "keys")
            && call.args.first().is_some_and(|argument| {
                self.refers_to(argument, symbol)
            })
    }

    /// Resolve a reused serialization temporary at this actual call argument.
    /// The reaching-definition proof is intentionally use-site based: a
    /// matching write elsewhere in the function is not evidence for this
    /// call.  The only additional write allowed on the local is the optional
    /// `temporary = encryptor(temporary)` self-reassignment used by the
    /// lowered corpus form.
    ///
    /// A non-call argument must be a serializer local declared by this site
    /// whose single write of the serializer value is a literal
    /// `JSON.stringify(state)`.  A captured/foreign alias is never accepted,
    /// even when its single value happens to serialize the state.
    fn json_stringify_binding_of(
        &self,
        site: crate::signals::extract::bindings::FunctionSite,
        expression: &'a Expression<'a>,
        symbol: SymbolId,
        encryptor: crate::signals::extract::bindings::FunctionSite,
    ) -> bool {
        if self.is_literal_json_stringify_of(expression, symbol) {
            return true;
        }
        let Some(local) = self.symbol(expression) else {
            return false;
        };
        let declaration = self.model.scoping().symbol_declaration(local);
        if self.model.owning_site(declaration).map(|owner| owner.node_id) != Some(site.node_id)
            || !matches!(self.model.nodes().kind(declaration), AstKind::VariableDeclarator(_))
            || !self.model.scoping().symbol_redeclarations(local).is_empty()
        {
            return false;
        }
        let AstKind::VariableDeclarator(declarator) = self.model.nodes().kind(declaration) else {
            return false;
        };
        if let Some(init) = declarator.init.as_ref() {
            if !self.is_literal_json_stringify_of(init, symbol) {
                return false;
            }
        }

        let Ok(definition) = expression_value_at_use(
            self.model.scoping(),
            self.model.nodes(),
            expression,
        ) else {
            return false;
        };
        if !self.is_literal_json_stringify_of(definition.value, symbol) {
            return false;
        }

        let mut stringify_writes = if declarator.init.as_ref().is_some() { 1 } else { 0 };
        let mut self_reassignments = 0;
        for reference in self.model.scoping().get_resolved_references(local) {
            if !reference.is_write() {
                continue;
            }
            if self.model.owning_site(reference.node_id()).map(|owner| owner.node_id)
                != Some(site.node_id)
            {
                return false;
            }
            let AstKind::AssignmentExpression(assignment) =
                self.model.nodes().parent_kind(reference.node_id())
            else {
                return false;
            };
            if assignment.operator != AssignmentOperator::Assign {
                return false;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                return false;
            };
            if analysis::identifier_symbol(identifier, self.model.scoping()) != Some(local) {
                return false;
            }
            if self.is_literal_json_stringify_of(&assignment.right, symbol) {
                stringify_writes += 1;
                continue;
            }
            if self.is_encryptor_self_reassignment(&assignment.right, local, encryptor) {
                self_reassignments += 1;
                continue;
            }
            return false;
        }

        // A single stringify definition must be the one reached at this use;
        // otherwise a later donor or a second branch/definition could still
        // make the value bag look convincing.
        stringify_writes == 1 && self_reassignments <= 1
    }

    /// A literal `JSON.stringify(<state>)` call, without resolving the
    /// expression through a captured/foreign single-value alias.  This is the
    /// only serializer definition the strict variant arm accepts.
    fn is_literal_json_stringify_of(
        &self,
        expression: &'a Expression<'a>,
        symbol: SymbolId,
    ) -> bool {
        let Expression::CallExpression(call) = expression.without_parentheses() else {
            return false;
        };
        let call = normal_call(call);
        self.is_global_method(&call, "JSON", "stringify")
            && call
                .args
                .first()
                .is_some_and(|argument| self.refers_to(argument, symbol))
    }

    /// Whether an upload `JSON.stringify(...)` argument ties back to the parsed
    /// report map.  The upload temporary is often a generated outer binding that
    /// the corpus coalesces across functions, so a strict use-site reaching
    /// definition cannot be required here; instead the recorded values of the
    /// binding are searched for the parsed symbol, following the
    /// `Object.fromEntries(Object.entries(alias).slice(-N))` fold.  A captured
    /// stringify of the telemetry state does not tie to the parsed map.
    fn upload_serializes_parsed(
        &self,
        expression: &'a Expression<'a>,
        parsed_symbol: SymbolId,
    ) -> bool {
        let Expression::CallExpression(call) = self.origin(expression) else {
            return false;
        };
        let call = normal_call(call);
        if !self.is_global_method(&call, "JSON", "stringify") {
            return false;
        }
        call.args
            .first()
            .is_some_and(|argument| self.value_ties_parsed(argument, parsed_symbol, 0))
    }

    /// Whether the value of `expression` is `symbol` directly, a binding with a
    /// recorded value that denotes `symbol`, or an expression built from such a
    /// value.
    fn value_ties_parsed(
        &self,
        expression: &'a Expression<'a>,
        symbol: SymbolId,
        depth: u8,
    ) -> bool {
        if depth > 8 {
            return false;
        }
        let expression = expression.without_parentheses();
        if self.refers_to(expression, symbol) {
            return true;
        }
        if let Some(local) = self.symbol(expression) {
            return self.values_of(local).iter().any(|value| {
                self.refers_to(value, symbol)
                    || self.expression_ties_symbol(value, symbol, depth + 1)
            });
        }
        self.expression_ties_symbol(expression, symbol, depth + 1)
    }

    /// Follow the reads used to build an expression (a call, member access or
    /// conditional) to a recorded value that denotes `symbol`.
    fn expression_ties_symbol(
        &self,
        expression: &'a Expression<'a>,
        symbol: SymbolId,
        depth: u8,
    ) -> bool {
        if depth > 8 {
            return false;
        }
        let expression = expression.without_parentheses();
        if self.refers_to(expression, symbol) {
            return true;
        }
        if let Some(local) = self.symbol(expression) {
            return self.values_of(local).iter().any(|value| {
                self.refers_to(value, symbol)
                    || self.expression_ties_symbol(value, symbol, depth + 1)
            });
        }
        match expression {
            Expression::CallExpression(call) => {
                self.expression_ties_symbol(&call.callee, symbol, depth + 1)
                    || call
                        .arguments
                        .iter()
                        .filter_map(Argument::as_expression)
                        .any(|argument| {
                            self.expression_ties_symbol(argument, symbol, depth + 1)
                        })
            }
            Expression::StaticMemberExpression(member) => {
                self.expression_ties_symbol(&member.object, symbol, depth + 1)
            }
            Expression::ComputedMemberExpression(member) => {
                self.expression_ties_symbol(&member.object, symbol, depth + 1)
                    || self.expression_ties_symbol(&member.expression, symbol, depth + 1)
            }
            Expression::SequenceExpression(sequence) => sequence
                .expressions
                .iter()
                .any(|item| self.expression_ties_symbol(item, symbol, depth + 1)),
            Expression::ConditionalExpression(conditional) => {
                self.expression_ties_symbol(&conditional.test, symbol, depth + 1)
                    || self.expression_ties_symbol(&conditional.consequent, symbol, depth + 1)
                    || self.expression_ties_symbol(&conditional.alternate, symbol, depth + 1)
            }
            Expression::LogicalExpression(logical) => {
                self.expression_ties_symbol(&logical.left, symbol, depth + 1)
                    || self.expression_ties_symbol(&logical.right, symbol, depth + 1)
            }
            _ => false,
        }
    }

    /// The error sink must come from outside the persistence function and must
    /// not be written inside its body, so the identity compared at the two
    /// catch sites and the promise tail cannot be a reassigned local.  A
    /// program-wide "no writes" rule is too strong for the coalesced corpus:
    /// the same captured binding is written by unrelated generated code
    /// elsewhere in the module, which does not change the value at these uses.
    fn is_captured_immutable_sink(
        &self,
        site: crate::signals::extract::bindings::FunctionSite,
        callee: &'a Expression<'a>,
    ) -> bool {
        let Some(symbol) = self.symbol(callee) else {
            return false;
        };
        let declaration = self.model.scoping().symbol_declaration(symbol);
        if self.model.owning_site(declaration).map(|owner| owner.node_id) == Some(site.node_id) {
            return false;
        }
        let Some(body) = site.body_span else {
            return false;
        };
        !self
            .model
            .scoping()
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .any(|reference| {
                self.in_span(
                    self.model.nodes().get_node(reference.node_id()).span(),
                    body,
                )
            })
    }

    fn is_encryptor_self_reassignment(
        &self,
        expression: &'a Expression<'a>,
        local: SymbolId,
        encryptor: crate::signals::extract::bindings::FunctionSite,
    ) -> bool {
        let Expression::CallExpression(call) = self.origin(expression) else {
            return false;
        };
        let call = normal_call(call);
        self.function_of(call.callee)
            .is_some_and(|callee| callee.span == encryptor.span)
            && call.args.len() == 1
            && self.symbol(call.args[0]) == Some(local)
    }

    fn claim_restore_aggregate(
        &mut self,
        decryptor: Option<crate::signals::extract::bindings::FunctionSite>,
    ) -> Option<crate::signals::extract::bindings::FunctionSite> {
        let decryptor = decryptor?;
        let mut found = Vec::new();
        for site in self.sites.clone() {
            if !self.owns_storage_get(site) || !self.owns_json_parse(site) {
                continue;
            }
            if self.owns_call(site, "allSettled") {
                continue;
            }
            let decrypts_summary = self.calls(site).iter().any(|call| {
                self.function_of(call.callee).is_some_and(|callee| callee.span == decryptor.span)
                    && call.args.first().is_some_and(|argument| {
                        self.is_summary_of_parsed_record(argument)
                    })
            });
            if decrypts_summary {
                found.push(site);
            }
        }
        let [site] = found.as_slice() else {
            return None;
        };
        self.claims.function(
            RESTORE_AGGREGATE,
            site.span,
            None,
            &[
                "reads the stored envelope, parses it, and decrypts the aggregate summary field before applying it to the aggregate record",
            ],
        );
        Some(*site)
    }

    /// `parsed.a` where `parsed` is a local holding `JSON.parse(...)`.
    fn is_summary_of_parsed_record(&self, expression: &'a Expression<'a>) -> bool {
        let Some((object, name)) = member_of(self.origin(expression)) else {
            return false;
        };
        if name != "a" {
            return false;
        }
        self.is_json_call(object, "parse")
    }

    // -----------------------------------------------------------------------
    // Recognizers: promise and callback wiring
    // -----------------------------------------------------------------------

    /// The `.then`/`.catch` call in `site` whose receiver is exactly the call at
    /// `receiver`, with its callback.
    fn step(
        &self,
        site: crate::signals::extract::bindings::FunctionSite,
        receiver: Span,
        method: &str,
    ) -> Option<(Span, crate::signals::extract::bindings::FunctionSite)> {
        self.own_of(site)?.calls.iter().find_map(|call| {
            let call = normal_call(call);
            if call.method != Some(method) {
                return None;
            }
            let value = call.receiver?;
            if self.origin(value).span() != receiver {
                return None;
            }
            let callback = call.args.first().and_then(|arg| self.function_of(arg))?;
            Some((call.span, callback))
        })
    }

    /// The one function of a candidate list, deduplicated by span. Two
    /// different candidates cannot be told apart, so none is used.
    fn only_site(
        items: Vec<crate::signals::extract::bindings::FunctionSite>,
    ) -> Option<crate::signals::extract::bindings::FunctionSite> {
        let mut spans = BTreeSet::new();
        let mut found = None;
        for item in items {
            if spans.insert((item.span.start, item.span.end)) {
                found = Some(item);
            }
        }
        (spans.len() == 1).then_some(found).flatten()
    }

    /// The one call span of a candidate list, deduplicated by span.
    fn only_span(items: Vec<Call<'a>>) -> Option<Span> {
        let mut spans = BTreeSet::new();
        for item in items {
            spans.insert((item.span.start, item.span.end));
            if spans.len() > 1 {
                return None;
            }
        }
        spans.iter().next().map(|(start, end)| Span::new(*start, *end))
    }

    fn claim_record_steps(
        &mut self,
        record: crate::signals::extract::bindings::FunctionSite,
        aggregate: SymbolId,
        encryptor: Option<crate::signals::extract::bindings::FunctionSite>,
    ) {
        let Some(encryptor) = encryptor else {
            return;
        };
        if let Some(callback) = self.find_index_callback(record) {
            self.claims.function(
                EVENT_ROW_MATCHER,
                callback.span,
                None,
                &["findIndex callback of the telemetry record: compares the p1..p4 fields of a stored event row"],
            );
        }
        let starts: Vec<Call<'a>> = self
            .own_of(record)
            .map(|own| {
                own.calls
                    .iter()
                    .map(|call| normal_call(call))
                    .filter(|call| {
                        self.function_of(call.callee)
                            .is_some_and(|callee| callee.span == encryptor.span)
                            && call.args.first().is_some_and(|argument| {
                                self.json_stringify_of_member(argument, aggregate)
                            })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let Some(start) = Self::only_span(starts) else {
            return;
        };
        let Some((then1, stringify)) = self.step(record, start, "then") else {
            return;
        };
        if self.own_of(stringify).is_some_and(|own| {
            own.news.iter().any(|new| {
                self.is_global(&new.callee, "Uint8Array")
            }) && self.owns_call(stringify, "toString")
        }) {
            self.claims.function(
                STRINGIFY_ENVELOPE,
                stringify.span,
                None,
                &["first then of the encrypted-record chain: converts the envelope ArrayBuffer to decimal bytes"],
            );
        }
        let Some((then2, store)) = self.step(record, then1, "then") else {
            return;
        };
        if self.calls(store).iter().any(|call| self.is_storage_method(call, "setItem")) {
            self.claims.function(
                STORE_ENCRYPTED_RECORD,
                store.span,
                None,
                &["second then of the encrypted-record chain: stores the encrypted report and its count with localStorage.setItem"],
            );
        }
        if let Some((_, report)) = self.step(record, then2, "catch") {
            if self.returns_error_report(report) {
                self.claims.function(
                    REPORT_RECORD_FAILURE,
                    report.span,
                    None,
                    &["catch of the encrypted-record chain: reports the error object to the error sink"],
                );
            }
        }
    }

    fn claim_persist_steps(
        &mut self,
        persist: crate::signals::extract::bindings::FunctionSite,
        state: SymbolId,
        encryptor: Option<crate::signals::extract::bindings::FunctionSite>,
    ) {
        let Some(encryptor) = encryptor else {
            return;
        };
        let starts: Vec<Call<'a>> = self
            .own_of(persist)
            .map(|own| {
                own.calls
                    .iter()
                    .map(|call| normal_call(call))
                    .filter(|call| {
                        self.function_of(call.callee)
                            .is_some_and(|callee| callee.span == encryptor.span)
                            && call
                                .args
                                .first()
                                .is_some_and(|argument| self.json_stringify_of(argument, state))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let Some(start) = Self::only_span(starts) else {
            return;
        };
        let Some((then1, merge)) = self.step(persist, start, "then") else {
            return;
        };
        if self.owns_call(merge, "fromEntries") {
            self.claims.function(
                MERGE_ENCRYPTED_STATE,
                merge.span,
                None,
                &["then of the state-encrypt chain: installs the encrypted state under the map key and keeps the last entries"],
            );
        }
        if let Some((_, report)) = self.step(persist, then1, "catch") {
            if self.returns_error_report(report) {
                self.claims.function(
                    REPORT_PERSIST_FAILURE,
                    report.span,
                    None,
                    &["catch of the state-encrypt chain: reports the persistence error"],
                );
            }
        }
    }

    fn claim_restore_steps(
        &mut self,
        restore: crate::signals::extract::bindings::FunctionSite,
        stored: Option<SymbolId>,
        cache: Option<SymbolId>,
        decryptor: Option<crate::signals::extract::bindings::FunctionSite>,
    ) {
        let Some(decryptor) = decryptor else {
            return;
        };
        let mut entry_callbacks = Vec::new();
        for call in self.calls(restore) {
            if call.method != Some("map") {
                continue;
            }
            let Some(receiver) = call.receiver else {
                continue;
            };
            let Some(stored) = stored else {
                continue;
            };
            if !self.is_object_keys_of(receiver, stored) {
                continue;
            }
            let Some(callback) = call.args.first().and_then(|arg| self.function_of(arg)) else {
                continue;
            };
            let decrypts = self.calls(callback).iter().any(|inner| {
                self.function_of(inner.callee)
                    .is_some_and(|callee| callee.span == decryptor.span)
            });
            if decrypts {
                entry_callbacks.push(callback);
            }
        }
        if let Some(callback) = Self::only_site(entry_callbacks) {
            self.claims.function(
                DECRYPT_STORED_ENTRY,
                callback.span,
                None,
                &["map callback of the restore step: decrypts the stored report under one decoded key"],
            );
        }
        let all_settled = Self::only_span(
            self.calls(restore)
                .into_iter()
                .filter(|call| call.method == Some("allSettled"))
                .collect(),
        );
        let Some(all_settled) = all_settled else {
            return;
        };
        let Some((then1, collect)) = self.step(restore, all_settled, "then") else {
            return;
        };
        let settles = self.calls(collect).iter().any(|call| call.method == Some("map"));
        if settles {
            self.claims.function(
                COLLECT_SETTLED,
                collect.span,
                None,
                &["then of Promise.allSettled: maps every settlement record of the stored-report decrypts"],
            );
        }
        let mut restore_callbacks = Vec::new();
        for call in self.calls(collect) {
            if call.method != Some("map") {
                continue;
            }
            let Some(callback) = call.args.first().and_then(|arg| self.function_of(arg)) else {
                continue;
            };
            let restores = self.own_of(callback).is_some_and(|own| {
                own.strings.contains(&"fulfilled")
                    && cache.is_some_and(|cache| {
                        own.assigns.iter().any(|assignment| {
                            target_object(&assignment.left)
                                .is_some_and(|object| self.refers_to(object, cache))
                                && member_of(self.origin(&assignment.right))
                                    .is_some_and(|(_, name)| name == "value")
                        })
                    })
            });
            if restores {
                restore_callbacks.push(callback);
            }
        }
        if let Some(callback) = Self::only_site(restore_callbacks) {
            self.claims.function(
                RESTORE_SETTLED_ENTRY,
                callback.span,
                None,
                &["settlement map callback: stores a fulfilled decrypt value in the cache and reports a rejected one"],
            );
        }
        if let Some((_, report)) = self.step(restore, then1, "catch") {
            if self.returns_error_report(report) {
                self.claims.function(
                    REPORT_RESTORE_FAILURE,
                    report.span,
                    None,
                    &["catch of the restore chain: reports the settlement failure"],
                );
            }
        }
    }

    fn claim_aggregate_steps(
        &mut self,
        aggregate_restore: crate::signals::extract::bindings::FunctionSite,
        aggregate: SymbolId,
        decryptor: Option<crate::signals::extract::bindings::FunctionSite>,
    ) {
        let Some(decryptor) = decryptor else {
            return;
        };
        let starts: Vec<Call<'a>> = self
            .own_of(aggregate_restore)
            .map(|own| {
                own.calls
                    .iter()
                    .map(|call| normal_call(call))
                    .filter(|call| {
                        self.function_of(call.callee)
                            .is_some_and(|callee| callee.span == decryptor.span)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let Some(start) = Self::only_span(starts) else {
            return;
        };
        let Some((then1, apply)) = self.step(aggregate_restore, start, "then") else {
            return;
        };
        let applies = self.own_of(apply).is_some_and(|own| {
            own.assigns.iter().any(|assignment| {
                target_member(&assignment.left).is_some_and(|(object, name)| {
                    self.refers_to(object, aggregate) && name == "a"
                })
            })
        });
        if applies {
            self.claims.function(
                APPLY_DECRYPTED_AGGREGATE,
                apply.span,
                None,
                &["then of the aggregate decrypt: replaces the aggregate summary field with the decrypted value"],
            );
        }
        if let Some((_, report)) = self.step(aggregate_restore, then1, "catch") {
            if self.returns_error_report(report) {
                self.claims.function(
                    REPORT_AGGREGATE_FAILURE,
                    report.span,
                    None,
                    &["catch of the aggregate decrypt: reports the aggregate restore failure"],
                );
            }
        }
    }

    fn claim_encryptor_steps(&mut self, encryptor: crate::signals::extract::bindings::FunctionSite) {
        let generate = Self::only_span(
            self.calls(encryptor)
                .into_iter()
                .filter(|call| {
                    call.method == Some("generateKey") || self.is_global(call.callee, "generateKey")
                })
                .collect(),
        );
        if let Some(generate) = generate {
            if let Some((_, keep)) = self.step(encryptor, generate, "then") {
                if self.returns_slot(keep, 0) {
                    self.claims.function(
                        KEEP_EPHEMERAL_KEY,
                        keep.span,
                        None,
                        &["then of generateKey: returns the generated CryptoKey unchanged for the ephemeral envelope"],
                    );
                }
            }
        }
        let mut encrypt_callbacks = Vec::new();
        for call in self.calls(encryptor) {
            if call.method != Some("then") {
                continue;
            }
            let Some(callback) = call.args.first().and_then(|arg| self.function_of(arg)) else {
                continue;
            };
            if callback.span != encryptor.span && self.owns_call(callback, "encrypt") {
                encrypt_callbacks.push(callback);
            }
        }
        let Some(encrypt_callback) = Self::only_site(encrypt_callbacks) else {
            return;
        };
        let draws_iv = self.owns_call(encrypt_callback, "getRandomValues");
        if draws_iv {
            self.claims.function(
                ENCRYPT_PAYLOAD,
                encrypt_callback.span,
                None,
                &["draws the 12-byte AES-GCM IV and calls subtle.encrypt on the encoded string bytes"],
            );
        }
        let export_callbacks: Vec<_> = self
            .calls(encrypt_callback)
            .into_iter()
            .filter(|call| call.method == Some("then"))
            .filter_map(|call| call.args.first().and_then(|arg| self.function_of(arg)))
            .filter(|callback| self.owns_call(*callback, "exportKey"))
            .collect();
        let Some(export_callback) = Self::only_site(export_callbacks) else {
            return;
        };
        self.claims.function(
            EXPORT_EPHEMERAL_KEY,
            export_callback.span,
            None,
            &["then of the encrypt call: exports the ephemeral AES key in raw format"],
        );
        let pack_callbacks: Vec<_> = self
            .calls(export_callback)
            .into_iter()
            .filter(|call| call.method == Some("then"))
            .filter_map(|call| call.args.first().and_then(|arg| self.function_of(arg)))
            .filter(|callback| {
                self.own_of(*callback).is_some_and(|own| {
                    own.news
                        .iter()
                        .any(|new| self.is_global(&new.callee, "Uint8Array"))
                        && own.calls.iter().any(|call| normal_call(call).method == Some("set"))
                })
            })
            .collect();
        if let Some(pack_callback) = Self::only_site(pack_callbacks) {
            self.claims.function(
                PACK_ENVELOPE,
                pack_callback.span,
                None,
                &["then of exportKey: concatenates raw key, IV and ciphertext into the packed envelope buffer"],
            );
        }
    }

    fn claim_decryptor_steps(&mut self, decryptor: crate::signals::extract::bindings::FunctionSite) {
        let mut map_callbacks = Vec::new();
        for call in self.calls(decryptor) {
            if call.method != Some("map") {
                continue;
            }
            let Some(callback) = call.args.first().and_then(|arg| self.function_of(arg)) else {
                continue;
            };
            let parses = self.own_of(callback).is_some_and(|own| {
                matches!(own.returns.as_slice(), [value]
                    if matches!(self.origin(value), Expression::CallExpression(call)
                        if self.is_global(normal_call(call).callee, "parseInt")))
            });
            if parses {
                map_callbacks.push(callback);
            }
        }
        if let Some(map_callback) = Self::only_site(map_callbacks) {
            self.claims.function(
                PARSE_ENVELOPE_BYTE,
                map_callback.span,
                None,
                &["map callback of the stored envelope split: parses one comma-separated decimal byte"],
            );
        }
        let mut decrypt_callbacks = Vec::new();
        let mut json_callbacks = Vec::new();
        for call in self.calls(decryptor) {
            if call.method != Some("then") {
                continue;
            }
            let Some(callback) = call.args.first().and_then(|arg| self.function_of(arg)) else {
                continue;
            };
            if self.owns_call(callback, "decrypt") {
                decrypt_callbacks.push(callback);
            } else if self.owns_json_parse(callback)
                && self.own_of(callback).is_some_and(|own| {
                    own.news
                        .iter()
                        .any(|new| self.is_global(&new.callee, "TextDecoder"))
                })
            {
                json_callbacks.push(callback);
            }
        }
        if let Some(json_callback) = Self::only_site(json_callbacks) {
            self.claims.function(
                PARSE_DECRYPTED_JSON,
                json_callback.span,
                None,
                &["last then of the decrypt chain: UTF-8 decodes and JSON-parses the plaintext, reporting a null payload"],
            );
        }
        let Some(decrypt_callback) = Self::only_site(decrypt_callbacks) else {
            return;
        };
        let decrypt_call = Self::only_span(
            self.calls(decrypt_callback)
                .into_iter()
                .filter(|call| {
                    call.method == Some("decrypt") || self.is_global(call.callee, "decrypt")
                })
                .collect(),
        );
        let Some(decrypt_call) = decrypt_call else {
            return;
        };
        self.claims.function(
            DECRYPT_PAYLOAD,
            decrypt_callback.span,
            None,
            &["then of importKey: builds the AES-GCM parameter object and calls subtle.decrypt on the ciphertext"],
        );
        if let Some((_, keep)) = self.step(decrypt_callback, decrypt_call, "then") {
            if self.returns_slot(keep, 0) {
                self.claims.function(
                    KEEP_PLAINTEXT,
                    keep.span,
                    None,
                    &["then of the decrypt call: returns the decrypted buffer unchanged"],
                );
            }
        }
    }
}
