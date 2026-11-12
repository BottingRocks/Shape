//! Existing source-count and worker input joins, without filesystem I/O.
#[derive(Debug)]
pub struct Audit {
    pub counts: Vec<serde_json::Value>,
    pub token_writes: serde_json::Value,
}
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, SourceType};
use serde_json::{json, Value};

#[derive(Default)]
struct Scan<'a> {
    exprs: Vec<&'a Expression<'a>>,
    defs: Vec<(SymbolId, &'a Expression<'a>)>,
    assigns: Vec<&'a AssignmentExpression<'a>>,
    controls: Vec<(u32, u32, u32, u32)>,
}
impl<'a> Visit<'a> for Scan<'a> {
    fn visit_do_while_statement(&mut self, n: &DoWhileStatement<'a>) {
        self.controls.push((
            n.span.start,
            n.span.end,
            n.test.span().start,
            n.test.span().end,
        ));
        break_controls(&n.body, n.span.start, n.span.end, &mut self.controls);
        walk::walk_do_while_statement(self, n)
    }
    fn visit_while_statement(&mut self, n: &WhileStatement<'a>) {
        self.controls.push((
            n.span.start,
            n.span.end,
            n.test.span().start,
            n.test.span().end,
        ));
        break_controls(&n.body, n.span.start, n.span.end, &mut self.controls);
        walk::walk_while_statement(self, n)
    }
    fn visit_for_statement(&mut self, n: &ForStatement<'a>) {
        if let Some(t) = &n.test {
            self.controls
                .push((n.span.start, n.span.end, t.span().start, t.span().end))
        }
        break_controls(&n.body, n.span.start, n.span.end, &mut self.controls);
        walk::walk_for_statement(self, n)
    }
    fn visit_expression(&mut self, e: &Expression<'a>) {
        let e = self.alloc(e);
        self.exprs.push(e);
        walk::walk_expression(self, e)
    }
    fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
        let d = self.alloc(d);
        if let (BindingPattern::BindingIdentifier(i), Some(e)) = (&d.id, &d.init) {
            if let Some(s) = i.symbol_id.get() {
                self.defs.push((s, e))
            }
        }
        walk::walk_variable_declarator(self, d)
    }
    fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
        let a = self.alloc(a);
        self.assigns.push(a);
        walk::walk_assignment_expression(self, a)
    }
}
fn breaks(s: &Statement) -> bool {
    match s {
        Statement::BreakStatement(_) => true,
        Statement::BlockStatement(b) => b.body.iter().any(breaks),
        _ => false,
    }
}
fn break_controls(s: &Statement, start: u32, end: u32, out: &mut Vec<(u32, u32, u32, u32)>) {
    match s {
        Statement::BlockStatement(b) => {
            for s in &b.body {
                break_controls(s, start, end, out)
            }
        }
        Statement::IfStatement(i) => {
            if breaks(&i.consequent) || i.alternate.as_ref().is_some_and(breaks) {
                out.push((start, end, i.test.span().start, i.test.span().end))
            }
            break_controls(&i.consequent, start, end, out);
            if let Some(a) = &i.alternate {
                break_controls(a, start, end, out)
            }
        }
        _ => {}
    }
}
fn sym(s: &Scoping, i: &IdentifierReference) -> Option<SymbolId> {
    i.reference_id
        .get()
        .and_then(|r| s.get_reference(r).symbol_id())
}
fn norm(
    e: &Expression,
    s: &Scoping,
    defs: &[(SymbolId, &Expression)],
    at: u32,
    depth: usize,
) -> String {
    if depth > 30 {
        return "depth".into();
    }
    match e.without_parentheses() {
        Expression::Identifier(i) => {
            let Some(id) = sym(s, i) else {
                return format!("global:{}", i.name);
            };
            if let Some((_, v)) = defs
                .iter()
                .filter(|(k, v)| *k == id && v.span().end <= at)
                .max_by_key(|(_, v)| v.span().end)
            {
                norm(v, s, defs, v.span().start, depth + 1)
            } else {
                format!("symbol:{id:?}")
            }
        }
        Expression::StaticMemberExpression(m) => format!(
            "{}.{}",
            norm(&m.object, s, defs, at, depth + 1),
            m.property.name
        ),
        Expression::ComputedMemberExpression(m) => format!(
            "{}[{}]",
            norm(&m.object, s, defs, at, depth + 1),
            norm(&m.expression, s, defs, at, depth + 1)
        ),
        Expression::NumericLiteral(n) => format!("number:{}", n.value),
        Expression::StringLiteral(n) => format!("string:{}", n.value),
        Expression::CallExpression(c) => format!("call:{}", c.span.start),
        Expression::NewExpression(c) => format!("new:{}", c.span.start),
        Expression::ArrayExpression(c) => format!("array:{}", c.span.start),
        Expression::ObjectExpression(c) => format!("object:{}", c.span.start),
        _ => format!("expression:{}:{}", e.span().start, e.span().end),
    }
}
fn origin(
    e: &Expression,
    s: &Scoping,
    defs: &[(SymbolId, &Expression)],
    at: u32,
    depth: usize,
) -> Option<(u32, u32, Option<SymbolId>)> {
    if depth > 30 {
        return None;
    }
    match e.without_parentheses() {
        Expression::Identifier(i) => {
            let id = sym(s, i)?;
            let (_, v) = defs
                .iter()
                .filter(|(k, v)| *k == id && v.span().end <= at)
                .max_by_key(|(_, v)| v.span().end)?;
            origin(v, s, defs, v.span().start, depth + 1)
        }
        Expression::StaticMemberExpression(m) if m.property.name == "length" => Some((
            m.span.start,
            m.span.end,
            if let Expression::Identifier(i) = m.object.without_parentheses() {
                sym(s, i)
            } else {
                None
            },
        )),
        _ => None,
    }
}
fn links(g: &Value, out: &mut Vec<(u32, u32, Value)>) {
    match g["op"].as_str() {
        Some("sequence") => {
            let mut prev: Option<Value> = None;
            for n in g["items"].as_array().unwrap() {
                if n["op"] == "repeat" {
                    if let Some(p) = &prev {
                        out.push((
                            n["span"][0].as_u64().unwrap() as u32,
                            n["span"][1].as_u64().unwrap() as u32,
                            p.clone(),
                        ))
                    }
                }
                links(n, out);
                if n["op"] == "number" {
                    prev = Some(n.clone())
                }
            }
        }
        Some("repeat") => links(&g["body"], out),
        Some("choice") => {
            for a in g["arms"].as_array().unwrap() {
                links(&a["body"], out)
            }
        }
        _ => {}
    }
}
pub fn audit(source: &str, schema: &Value) -> Result<Audit, Box<dyn std::error::Error>> {
    let a = oxc_allocator::Allocator::default();
    let p = oxc_parser::Parser::new(&a, source, SourceType::mjs()).parse();
    if !p.errors.is_empty() {
        return Err("invalid program syntax".into());
    }
    let semantic = oxc_semantic::SemanticBuilder::new()
        .build(&p.program)
        .semantic;
    let scoping = semantic.scoping();
    let mut scan = Scan::default();
    scan.visit_program(&p.program);
    for a in &scan.assigns {
        if a.operator.as_str() != "=" {
            continue;
        }
        if let AssignmentTarget::AssignmentTargetIdentifier(i) = &a.left {
            if let Some(s) = sym(scoping, i) {
                scan.defs.push((s, &a.right))
            }
        }
    }
    let mut token_writes = vec![];
    for a in &scan.assigns {
        let token = match &a.left {
            AssignmentTarget::StaticMemberExpression(m) => m.property.name == "transactionToken",
            AssignmentTarget::ComputedMemberExpression(m) => {
                matches!(m.expression.without_parentheses(),Expression::StringLiteral(s) if s.value=="transactionToken")
            }
            _ => false,
        };
        if token {
            token_writes.push(json!({"span":[a.span.start,a.span.end],"valueSpan":[a.right.span().start,a.right.span().end],"assignment":a.operator.as_str(),"rhsIsIdentifier":matches!(a.right.without_parentheses(),Expression::Identifier(_))}));
        }
    }
    let mut out = vec![];
    for slot in schema["wire"]["slots"].as_array().unwrap() {
        let Some(pos) = slot["routing"]["positional"].as_u64() else {
            continue;
        };
        for (em, e) in slot["contract"]["emissions"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let mut ls = vec![];
            links(&e["grammar"], &mut ls);
            for (start, end, w) in ls {
                let vs = w["value_span"][0].as_u64().unwrap() as u32;
                let ve = w["value_span"][1].as_u64().unwrap() as u32;
                let count = scan
                    .exprs
                    .iter()
                    .find(|e| e.span().start == vs && e.span().end == ve);
                let mut candidates = vec![];
                if let Some(count) = count {
                    let expected = norm(count, scoping, &scan.defs, vs, 0);
                    let origin = origin(count, scoping, &scan.defs, vs, 0);
                    for e in &scan.exprs {
                        if e.span().start < start
                            || e.span().end > end
                            || !scan.controls.iter().any(|(a, b, c, d)| {
                                *a == start
                                    && *b == end
                                    && *c <= e.span().start
                                    && e.span().end <= *d
                            })
                        {
                            continue;
                        }
                        if let Expression::BinaryExpression(b) = e.without_parentheses() {
                            if ["<", "<=", ">", ">="].contains(&b.operator.as_str()) {
                                for x in [&b.left, &b.right] {
                                    let mut n = norm(x, scoping, &scan.defs, start, 0);
                                    if let Expression::UpdateExpression(u) = x.without_parentheses()
                                    {
                                        if let SimpleAssignmentTarget::AssignmentTargetIdentifier(
                                            i,
                                        ) = &u.argument
                                        {
                                            if let Some(id) = sym(scoping, i) {
                                                if let Some((_, v)) = scan
                                                    .defs
                                                    .iter()
                                                    .filter(|(k, v)| {
                                                        *k == id && v.span().end <= start
                                                    })
                                                    .max_by_key(|(_, v)| v.span().end)
                                                {
                                                    n = norm(
                                                        v,
                                                        scoping,
                                                        &scan.defs,
                                                        v.span().start,
                                                        0,
                                                    )
                                                }
                                            }
                                        }
                                    }
                                    if n == expected {
                                        candidates.push(json!({"span":[x.span().start,x.span().end],"relation":"same source value as emitted count"}))
                                    }
                                }
                            }
                        }
                    }
                    out.push(json!({"position":pos,"emission":em,"loop":[start,end],"countWrite":w["span"],"countValueSpan":[vs,ve],"proven":!candidates.is_empty(),"matches":candidates,"lengthEvaluation":origin.map(|(a,b,_)|vec![a,b]),"collectionBinding":origin.and_then(|(_,_,id)|id).map(|id|{let s=scoping.symbol_span(id);vec![s.start,s.end]})}));
                } else {
                    out.push(json!({"position":pos,"emission":em,"loop":[start,end],"countWrite":w["span"],"proven":false,"reason":"value expression lookup"}));
                }
            }
        }
    }
    Ok(Audit {
        counts: out,
        token_writes: json!({"writes":token_writes,"sourceAssignmentsOnly":true}),
    })
}

/// Retain the concrete source input link separately from strict lifecycle
/// certification. The existing signal finder/binder identifies the source format;
/// these spans join the launcher's token field to the worker's string input.
pub fn worker_inputs(
    source: &str,
    slots: &[crate::signals::wire::WireSlot],
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let arena = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&arena, source, SourceType::mjs()).parse();
    let semantic = oxc_semantic::SemanticBuilder::new()
        .build(&parsed.program)
        .semantic;
    let scopes = semantic.scoping();
    let mut scan = Scan::default();
    scan.visit_program(&parsed.program);
    for a in &scan.assigns {
        if a.operator.as_str() == "=" {
            if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                if let Some(id) = sym(scopes, id) {
                    scan.defs.push((id, &a.right));
                }
            }
        }
    }
    let mut result = vec![];
    for slot in slots {
        for emission in &slot.contract.emissions {
            if slot.naming.as_ref().map(|n| n.canonical_name.as_str()) != Some("WorkerRealmProfile")
            {
                continue;
            }
            let (container, payload) = semantic
                .nodes()
                .iter()
                .filter_map(|node| {
                    let oxc_ast::AstKind::Function(f) = node.kind() else {
                        return None;
                    };
                    (f.span.start <= emission.site.start
                        && emission.site.end <= f.span.end
                        && f.params.items.len() == 2)
                        .then(|| {
                            let BindingPattern::BindingIdentifier(id) = &f.params.items[1].pattern
                            else {
                                return None;
                            };
                            Some((f.span, id.symbol_id.get()?))
                        })
                        .flatten()
                })
                .min_by_key(|(span, _)| span.end - span.start)
                .ok_or("worker source container missing")?;
            super::ensure(
                scopes
                    .get_resolved_references(payload)
                    .all(|r| !r.is_write()),
                "worker payload formal is overwritten",
            )?;
            let launcher = slot
                .contract
                .producer
                .body
                .ok_or("worker launcher missing")?;
            let mut posted = vec![];
            let mut inputs = vec![];
            for e in &scan.exprs {
                let span = e.span();
                if launcher.start <= span.start && span.end <= launcher.end {
                    if let Expression::ObjectExpression(object) = e.without_parentheses() {
                        for p in &object.properties {
                            if let ObjectPropertyKind::ObjectProperty(p) = p {
                                if p.key.static_name().as_deref() == Some("a")
                                    && norm(&p.value, scopes, &scan.defs, p.value.span().start, 0)
                                        .ends_with(".transactionToken")
                                {
                                    posted.push(json!({"payloadProperty":[p.span.start,p.span.end],"tokenRead":[p.value.span().start,p.value.span().end]}));
                                }
                            }
                        }
                    }
                }
                if container.start <= span.start && span.end <= container.end {
                    if let Expression::StaticMemberExpression(member) = e.without_parentheses() {
                        if member.property.name == "a"
                            && norm(&member.object, scopes, &scan.defs, span.start, 0)
                                == format!("symbol:{payload:?}")
                        {
                            inputs.push(json!([span.start, span.end]));
                        }
                    }
                }
            }
            if posted.is_empty() || inputs.is_empty() || emission.forwarding_sites.is_empty() {
                return Err(format!(
                    "worker input source link: posted={} reads={} forwards={}",
                    posted.len(),
                    inputs.len(),
                    emission.forwarding_sites.len()
                )
                .into());
            }
            result.push(json!({"emission":emission.site,"forwardingSites":emission.forwarding_sites,"workerContainer":[container.start,container.end],"postedToken":posted,"workerPayloadAReads":inputs,"input":"supplied initializer transactionToken; posted payload.a","strictLifecycleCertificate":false}));
        }
    }
    Ok(result)
}
