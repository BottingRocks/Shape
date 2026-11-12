//! Reusable exact structural labels. No generated names, PCs, source offsets or
//! cross-build function order participate in a signature. Literals, API names,
//! operators, control flow, lexical reference relationships and capture origins
//! remain significant. Unknown/mutable captures fail closed.
use crate::signals::extract::bindings::{FunctionSite, Model};
use crate::signals::schema::{ProgramPropertiesLimits, SourceIdentity, SourceSpan};
use crate::analysis::{self, Edit};
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_span::{GetSpan, Span};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructuralLabel {
    pub signature: String,
    pub name: String,
    pub confidence: String,
    pub original_span: SourceSpan,
    pub labeled_span: SourceSpan,
    pub evidence: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LabelReference {
    pub schema_version: u32,
    pub original_source: SourceIdentity,
    pub labeled_source: SourceIdentity,
    pub equal_build_proof: String,
    pub labels: Vec<StructuralLabel>,
    pub excluded: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LabelMatch {
    pub function_span: SourceSpan,
    pub signature: Option<String>,
    pub candidates: Vec<String>,
    pub status: String,
    pub reason: Option<String>,
}

fn identity(source: &str) -> SourceIdentity {
    SourceIdentity {
        sha256: crate::source::sha256_hex(source.as_bytes()),
        bytes: source.len(),
    }
}

/// This same-build proof is deliberately stronger than a matching row table.
/// Only comments and unused expression names are discarded. Symbols and their
/// references share canonical ordinals; static property keys stay unchanged.
fn canonical_build(source: &str) -> Result<String, String> {
    let alloc = Allocator::default();
    let p = analysis::parse_program(&alloc, source)?;
    let (scope, nodes) = analysis::analyze(&p);
    let mut edits = Vec::new();
    for c in &p.comments {
        edits.push(Edit::delete(c.span));
    }
    for node in nodes.iter() {
        if let AstKind::Function(f) = node.kind() {
            if f.r#type == FunctionType::FunctionExpression {
                if let Some(id) = &f.id {
                    if id
                        .symbol_id
                        .get()
                        .is_some_and(|s| scope.get_resolved_reference_ids(s).is_empty())
                    {
                        edits.push(Edit::delete(id.span));
                    }
                }
            }
        }
    }
    let clean = analysis::apply_edits(source, edits).output;
    let alloc = Allocator::default();
    let p = analysis::parse_program(&alloc, &clean)?;
    let (scope, nodes) = analysis::analyze(&p);
    let mut symbols: Vec<_> = scope.symbol_ids().collect();
    symbols.sort_by_key(|s| scope.symbol_span(*s).start);
    let names: BTreeMap<_, _> = symbols
        .into_iter()
        .enumerate()
        .map(|(i, s)| (s, format!("v{i}")))
        .collect();
    let mut edits = Vec::new();
    for node in nodes.iter() {
        let (symbol, span) = match node.kind() {
            AstKind::BindingIdentifier(i) => (i.symbol_id.get(), i.span),
            AstKind::IdentifierReference(i) => (analysis::identifier_symbol(i, &scope), i.span),
            _ => continue,
        };
        let Some(symbol) = symbol else { continue };
        let shorthand = matches!(nodes.parent_kind(node.id()),AstKind::ObjectProperty(p) if p.shorthand)
            || matches!(nodes.parent_kind(node.id()),AstKind::BindingProperty(p) if p.shorthand)
            || matches!(
                nodes.parent_kind(node.id()),
                AstKind::AssignmentTargetPropertyIdentifier(_)
            );
        let text = if shorthand {
            format!(
                "{}: {}",
                &clean[span.start as usize..span.end as usize],
                names[&symbol]
            )
        } else {
            names[&symbol].clone()
        };
        edits.push(Edit::replace(span, text));
    }
    let applied = analysis::apply_edits(&clean, edits);
    if !applied.rejected.is_empty() {
        return Err("canonical edits overlap".into());
    }
    let alloc = Allocator::default();
    let p = analysis::parse_program(&alloc, &applied.output)?;
    Ok(analysis::codegen(&p))
}

pub fn build_label_reference(original: &str, labeled: &str) -> Result<LabelReference, String> {
    let proof = canonical_build(original)?;
    if proof != canonical_build(labeled)? {
        return Err("label reference requires equal-build scoped AST/reference equivalence; unrelated builds cannot be joined by function order".into());
    }
    let oa = Allocator::default();
    let op = analysis::parse_program(&oa, original)?;
    let (os, on) = analysis::analyze(&op);
    let om = Model::new(original, &op, &os, &on, ProgramPropertiesLimits::default())
        .map_err(|e| e.to_string())?;
    let la = Allocator::default();
    let lp = analysis::parse_program(&la, labeled)?;
    let (ls, ln) = analysis::analyze(&lp);
    let lm = Model::new(labeled, &lp, &ls, &ln, ProgramPropertiesLimits::default())
        .map_err(|e| e.to_string())?;
    if om.all_sites().len() != lm.all_sites().len() {
        return Err("equal-build function inventories differ".into());
    }
    let mut labels = Vec::new();
    let mut excluded = Vec::new();
    for (o, l) in om.all_sites().iter().zip(lm.all_sites()) {
        let Some(name) = l.binding_symbol.map(|s| ls.symbol_name(s).to_string()) else {
            continue;
        };
        if name.starts_with('$') || name.starts_with("waveCollector") {
            continue;
        }
        match signature(&om,*o){Ok(sig)=>labels.push(StructuralLabel{signature:sig,name,confidence:"inherited_source_label_structural_identity".into(),original_span:o.span.into(),labeled_span:l.span.into(),evidence:"same-build scoped provenance join; exact normalized AST/body and capture-origin signature (literal/API preserving)".into()}),Err(reason)=>excluded.push(format!("{name}: {reason}"))}
    }
    Ok(LabelReference {
        schema_version: 1,
        original_source: identity(original),
        labeled_source: identity(labeled),
        equal_build_proof: crate::source::sha256_hex(proof.as_bytes()),
        labels,
        excluded,
    })
}

pub(crate) fn match_labels(
    model: &Model<'_, '_>,
    reference: &LabelReference,
) -> Result<Vec<LabelMatch>, String> {
    if reference.schema_version != 1 {
        return Err("unsupported thread label reference schema".into());
    }
    let mut lookup: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for label in &reference.labels {
        lookup
            .entry(&label.signature)
            .or_default()
            .insert(label.name.clone());
    }
    Ok(model
        .all_sites()
        .iter()
        .map(|site| match signature(model, *site) {
            Ok(sig) => {
                let candidates: Vec<_> = lookup
                    .get(sig.as_str())
                    .into_iter()
                    .flatten()
                    .cloned()
                    .collect();
                let status = match candidates.len() {
                    0 => "unmatched",
                    1 => "matched",
                    _ => "ambiguous",
                };
                LabelMatch {
                    function_span: site.span.into(),
                    signature: Some(sig),
                    candidates,
                    status: status.into(),
                    reason: None,
                }
            }
            Err(e) => LabelMatch {
                function_span: site.span.into(),
                signature: None,
                candidates: Vec::new(),
                status: "unsupported".into(),
                reason: Some(e),
            },
        })
        .collect())
}

fn signature(model: &Model<'_, '_>, site: FunctionSite) -> Result<String, String> {
    if matches!(model.nodes.get_node(site.node_id).kind(),AstKind::Function(f) if f.body.as_ref().is_some_and(|b|b.statements.is_empty()&&b.directives.is_empty()))
    {
        return Err("empty bodies carry no semantic identity; referenced callbacks are retained and dead declarations are handled separately".into());
    }
    let text = normalize_region(
        model,
        site.span,
        2,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
    )?;
    let allocator = Allocator::default();
    // Declaration/function-expression/arrow forms share a parseable wrapper. Do
    // not guess how to turn method syntax into a function expression.
    let wrapped = format!("({text});");
    let p = analysis::parse_program(&allocator, &wrapped)?;
    Ok(crate::source::sha256_hex(analysis::codegen(&p).as_bytes()))
}
fn normalize_region(
    model: &Model<'_, '_>,
    span: Span,
    depth: usize,
    visiting: &mut BTreeSet<u32>,
    capture_graph: &mut BTreeMap<u32, usize>,
) -> Result<String, String> {
    let local: Vec<_> = model
        .scoping
        .symbol_ids()
        .filter(|s| span.contains_inclusive(model.scoping.symbol_span(*s)))
        .collect();
    let mut local = local;
    local.sort_by_key(|s| model.scoping.symbol_span(*s).start);
    let locals: BTreeMap<_, _> = local
        .into_iter()
        .enumerate()
        .map(|(i, s)| (s, format!("v{i}")))
        .collect();
    let mut captures: BTreeMap<_, String> = BTreeMap::new();
    let mut edits = Vec::new();
    for node in model.nodes.iter() {
        let pos = node.span();
        if !span.contains_inclusive(pos) {
            continue;
        }
        let (symbol, pos) = match node.kind() {
            AstKind::BindingIdentifier(i) => (i.symbol_id.get(), i.span),
            AstKind::IdentifierReference(i) => {
                (analysis::identifier_symbol(i, model.scoping), i.span)
            }
            _ => continue,
        };
        let Some(symbol) = symbol else { continue };
        let name = if let Some(name) = locals.get(&symbol) {
            name.clone()
        } else if let Some(name) = captures.get(&symbol) {
            name.clone()
        } else {
            let key = symbol.index() as u32;
            if depth == 0 || !visiting.insert(key) {
                return Err("capture origin exceeds bounded structural proof or cycles".into());
            }
            let declaration = model
                .nodes
                .get_node(model.scoping.symbol_declaration(symbol));
            let next = capture_graph.len();
            let ordinal = *capture_graph.entry(key).or_insert(next);
            let writes = model.binding_writes(symbol);
            let allowed = if matches!(declaration.kind(), AstKind::Function(_)) {
                0
            } else {
                usize::from(model.initializer(symbol).is_none())
            };
            if writes > allowed {
                return Err("mutable/multiple-origin capture".into());
            }
            let origin = if let AstKind::Function(f) = declaration.kind() {
                normalize_region(model, f.span, depth - 1, visiting, capture_graph)?
            } else {
                let writes = model.binding_writes(symbol);
                if writes > usize::from(model.initializer(symbol).is_none()) {
                    return Err("mutable/multiple-origin capture".into());
                }
                let value = model
                    .symbol_value(symbol)
                    .ok_or("capture has no proven static origin")?;
                normalize_region(model, value.span(), depth - 1, visiting, capture_graph)?
            };
            visiting.remove(&key);
            let a = Allocator::default();
            let wrapped = format!("({origin});");
            let p = analysis::parse_program(&a, &wrapped)?;
            let name = format!(
                "capture_{}_{}",
                ordinal,
                crate::source::sha256_hex(analysis::codegen(&p).as_bytes())
            );
            captures.insert(symbol, name.clone());
            name
        };
        // Property keys remain literal, including generated-looking shorthand keys.
        let shorthand = matches!(model.nodes.parent_kind(node.id()),AstKind::ObjectProperty(p) if p.shorthand)
            || matches!(model.nodes.parent_kind(node.id()),AstKind::BindingProperty(p) if p.shorthand)
            || matches!(
                model.nodes.parent_kind(node.id()),
                AstKind::AssignmentTargetPropertyIdentifier(_)
            );
        let replacement = if shorthand {
            format!("{}: {name}", model.exact(pos))
        } else {
            name
        };
        edits.push(Edit::replace(
            Span::new(pos.start - span.start, pos.end - span.start),
            replacement,
        ));
    }
    // Ignore source comments without erasing string or regex content.
    let allocator = Allocator::default();
    let slice = model.exact(span);
    let wrapper = format!("({slice});");
    if let Ok(p) = analysis::parse_program(&allocator, &wrapper) {
        for c in &p.comments {
            if c.span.start > 0 && c.span.end <= span.size() + 1 {
                edits.push(Edit::delete(Span::new(c.span.start - 1, c.span.end - 1)));
            }
        }
    }
    let applied = analysis::apply_edits(slice, edits);
    if !applied.rejected.is_empty() {
        return Err("structural normalization edits overlap".into());
    }
    Ok(applied.output)
}
