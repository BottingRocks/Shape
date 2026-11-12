//! Software arithmetic only. The explicit protocol below is deliberately one
//! lowering, not an IEEE equivalence rule or an inventory of source bodies.
use crate::naming::support::interaction::*;
use crate::naming::*;
use oxc_span::GetSpan;

#[derive(Debug)]
enum Pattern {
    Atom(String),
    List(Vec<Pattern>),
}
impl Pattern {
    fn atom(&self) -> Option<&str> {
        match self {
            Self::Atom(a) => Some(a),
            _ => None,
        }
    }
    fn list(&self) -> Option<&[Pattern]> {
        match self {
            Self::List(a) => Some(a),
            _ => None,
        }
    }
}
fn protocol() -> Pattern {
    fn read(tokens: &[&str], cursor: &mut usize) -> Pattern {
        let token = tokens[*cursor];
        *cursor += 1;
        if token != "(" {
            return Pattern::Atom(token.into());
        }
        let mut children = Vec::new();
        while tokens[*cursor] != ")" {
            children.push(read(tokens, cursor));
        }
        *cursor += 1;
        Pattern::List(children)
    }
    let text = PROTOCOL.replace('(', " ( ").replace(')', " ) ");
    let tokens: Vec<_> = text.split_whitespace().collect();
    read(&tokens, &mut 0)
}

struct Proof<'m, 's, 'a> {
    model: &'m Model<'s, 'a>,
    site: FunctionSite,
    locals: BTreeMap<String, SymbolId>,
    tag: Option<SymbolId>,
}
impl Proof<'_, '_, '_> {
    fn bind(&mut self, label: &str, symbol: SymbolId) -> Option<()> {
        if self.locals.contains_key(label)
            || self.locals.values().any(|s| *s == symbol)
            || !self
                .site
                .span
                .contains_inclusive(self.model.scoping.symbol_span(symbol))
            || !self.model.scoping.symbol_redeclarations(symbol).is_empty()
            || !self.model.scoping.get_resolved_references(symbol).all(|r| {
                self.site
                    .span
                    .contains_inclusive(self.model.nodes.kind(r.node_id()).span())
            })
        {
            return None;
        }
        self.locals.insert(label.into(), symbol);
        Some(())
    }
    fn target(&self, p: &Pattern, target: &AssignmentTarget<'_>) -> Option<()> {
        (target_symbol(self.model, target)? == *self.locals.get(p.atom()?)?).then_some(())
    }
    fn expr(&mut self, p: &Pattern, e: &Expression<'_>) -> Option<()> {
        let e = e.without_parentheses();
        if let Some(atom) = p.atom() {
            if let Ok(n) = atom.parse::<f64>() {
                return number(e, n).then_some(());
            }
            if atom == "Math" {
                return crate::naming::support::seeded_flow::is_global(self.model, e, &["Math"])
                    .then_some(());
            }
            if atom == "tags" {
                let s = symbol(self.model, e)?;
                captured_symbol(self.model, self.site, s)?;
                if self.tag.is_some_and(|tag| tag != s) {
                    return None;
                }
                self.tag = Some(s);
                return Some(());
            }
            return (symbol(self.model, e)? == *self.locals.get(atom)?).then_some(());
        }
        let a = p.list()?;
        let op = a.first()?.atom()?;
        match (op, e) {
            ("member", Expression::StaticMemberExpression(m)) if a.len() == 3 && !m.optional => {
                if m.property.name != a[2].atom()? {
                    return None;
                }
                self.expr(&a[1], &m.object)
            }
            ("index", Expression::ComputedMemberExpression(m)) if a.len() == 3 && !m.optional => {
                self.expr(&a[1], &m.object)?;
                self.expr(&a[2], &m.expression)
            }
            ("call", Expression::CallExpression(c))
                if a.len() >= 2 && !c.optional && c.type_arguments.is_none() =>
            {
                self.expr(&a[1], &c.callee)?;
                if c.arguments.len() != a.len() - 2 {
                    return None;
                }
                for (p, arg) in a[2..].iter().zip(&c.arguments) {
                    self.expr(p, arg.as_expression()?)?;
                }
                Some(())
            }
            ("array", Expression::ArrayExpression(array))
                if array.elements.len() == a.len() - 1 =>
            {
                for (p, e) in a[1..].iter().zip(&array.elements) {
                    self.expr(p, e.as_expression()?)?;
                }
                Some(())
            }
            ("?", Expression::ConditionalExpression(c)) if a.len() == 4 => {
                self.expr(&a[1], &c.test)?;
                self.expr(&a[2], &c.consequent)?;
                self.expr(&a[3], &c.alternate)
            }
            ("pre" | "post", Expression::UpdateExpression(u)) if a.len() == 3 => {
                if u.prefix != (op == "pre") || u.operator.as_str() != a[1].atom()? {
                    return None;
                }
                let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument else {
                    return None;
                };
                (crate::analysis::identifier_symbol(id, self.model.scoping)?
                    == *self.locals.get(a[2].atom()?)?)
                .then_some(())
            }
            (_, Expression::AssignmentExpression(x))
                if a.len() == 3 && x.operator.as_str() == op =>
            {
                self.target(&a[1], &x.left)?;
                self.expr(&a[2], &x.right)
            }
            (_, Expression::BinaryExpression(x)) if a.len() == 3 && x.operator.as_str() == op => {
                self.expr(&a[1], &x.left)?;
                self.expr(&a[2], &x.right)
            }
            (_, Expression::LogicalExpression(x)) if a.len() == 3 && x.operator.as_str() == op => {
                self.expr(&a[1], &x.left)?;
                self.expr(&a[2], &x.right)
            }
            (_, Expression::UnaryExpression(x)) if a.len() == 2 && x.operator.as_str() == op => {
                self.expr(&a[1], &x.argument)
            }
            _ => None,
        }
    }
    fn statements(&mut self, ps: &[Pattern], ss: &[Statement<'_>]) -> Option<()> {
        if ps.len() != ss.len() {
            return None;
        }
        for (p, s) in ps.iter().zip(ss) {
            self.statement(p, s)?;
        }
        Some(())
    }
    fn statement(&mut self, p: &Pattern, s: &Statement<'_>) -> Option<()> {
        let a = p.list()?;
        match (a.first()?.atom()?, s) {
            ("block", Statement::BlockStatement(b)) => self.statements(&a[1..], &b.body),
            ("let", Statement::VariableDeclaration(d))
                if d.kind == VariableDeclarationKind::Let
                    && d.declarations.len() == a.len() - 1 =>
            {
                for (p, v) in a[1..].iter().zip(&d.declarations) {
                    let p = p.list()?;
                    if !(1..=2).contains(&p.len()) {
                        return None;
                    }
                    self.bind(p[0].atom()?, plain_var(self.model, v)?)?;
                    match (&v.init, p.get(1)) {
                        (Some(e), Some(p)) => self.expr(p, e)?,
                        (None, None) => {}
                        _ => return None,
                    }
                }
                Some(())
            }
            ("if", Statement::IfStatement(i)) if a.len() == 3 || a.len() == 4 => {
                self.expr(&a[1], &i.test)?;
                self.statement(&a[2], &i.consequent)?;
                match (a.get(3), &i.alternate) {
                    (Some(p), Some(s)) => self.statement(p, s),
                    (None, None) => Some(()),
                    _ => None,
                }
            }
            ("while", Statement::WhileStatement(w)) if a.len() == 3 => {
                self.expr(&a[1], &w.test)?;
                self.statement(&a[2], &w.body)
            }
            ("for", Statement::ForStatement(f)) if a.len() == 4 && f.init.is_none() => {
                self.expr(&a[1], f.test.as_ref()?)?;
                self.expr(&a[2], f.update.as_ref()?)?;
                self.statement(&a[3], &f.body)
            }
            ("expr", Statement::ExpressionStatement(e)) if a.len() == 2 => {
                self.expr(&a[1], &e.expression)
            }
            _ => None,
        }
    }
}

// Roles: value/output formals; sign/absolute magnitude; fraction/exponent;
// normalization exponent/significand; explicit parity rounding; chosen tag;
// exponent/fraction widths; MSB-first bit list and byte cursor.
// In particular 536870911 is both the mask AND divisor. It is not 2^29.
// Exponent adjustment precedes the narrow subnormal branch in this lowering.
const PROTOCOL: &str = r#"(
 (let (sign (|| (< value 0) (&& (=== value 0) (< (/ 1 value) 0)))))
 (let (magnitude (call (member Math abs) value)))
 (let (fraction) (exponent))
 (if (=== magnitude 0)
  (block (expr (= fraction 0)) (expr (= exponent 1023)))
  (if (>= magnitude (call (member Math pow) 2 (- 1022)))
   (block
    (let (normalExponent (call (member Math min) (call (member Math floor) (/ (call (member Math log) magnitude) (member Math LN2))) 1023)))
    (let (significand (/ magnitude (call (member Math pow) 2 normalExponent))))
    (let (adjusted (< significand 1)))
    (expr (= significand (? adjusted (* significand 2) significand)))
    (expr (= adjusted (? adjusted (- normalExponent 1) normalExponent)))
    (if (>= significand 2) (block (expr (pre ++ adjusted)) (expr (= significand (/ significand 2)))))
    (let (unit (call (member Math pow) 2 52)))
    (let (rounded (call (member Math floor) (* significand unit))))
    (expr (= significand (- (? (< (- (* significand unit) rounded) 0.5) rounded (? (> (- (* significand unit) rounded) 0.5) (+ rounded 1) (? (% rounded 2) (+ rounded 1) rounded))) unit)))
    (expr (+= adjusted 1023))
    (expr (= unit (>= (/ significand unit) 1)))
    (expr (= exponent (? unit (+ adjusted 1) adjusted)))
    (if (> exponent 2046)
     (block (expr (= fraction 0)) (expr (= exponent 2047)))
     (block (expr (= fraction (? unit 0 significand)))))
   )
   (block
    (let (subnormal (/ magnitude (call (member Math pow) 2 (- 1074)))))
    (expr (= fraction (call (member Math floor) subnormal)))
    (if (! (< (- subnormal fraction) 0.5))
     (block (if (> (- subnormal fraction) 0.5)
      (block (expr (pre ++ fraction)))
      (block (expr (= fraction (array fraction (% fraction 2)))) (expr (= fraction (? (index fraction 1) (+ (index fraction 0) 1) (index fraction 0))))))))
    (expr (= exponent 0))
   )
  )
 )
 (let (tag (member tags DOUBLE64)))
 (let (exponentWidth) (fractionWidth))
 (if (&& (=== (& fraction 536870911) 0) (< (call (member Math abs) (- exponent 1023)) 256))
  (block
   (expr (= tag (member tags FLOAT32)))
   (expr (= fraction (/ fraction 536870911)))
   (expr (= exponent (- (+ exponent 1023) 127)))
   (if (< magnitude (call (member Math pow) 2 (- 126)))
    (block
     (expr (= magnitude (/ magnitude (call (member Math pow) 2 (- (- 1 127) 23)))))
     (expr (= fraction (call (member Math floor) magnitude)))
     (if (! (< (- magnitude fraction) 0.5))
      (block (if (> (- magnitude fraction) 0.5)
       (block (expr (pre ++ fraction)))
       (block (expr (= fraction (array fraction (% fraction 2)))) (expr (= fraction (? (index fraction 1) (+ (index fraction 0) 1) (index fraction 0))))))))
     (expr (= exponent 0))
    )
   )
   (expr (= exponentWidth 8)) (expr (= fractionWidth 23))
  )
  (block (expr (= exponentWidth 11)) (expr (= fractionWidth 52)))
 )
 (let (bits (array)))
 (let (bitCursor (- fractionWidth 1)))
 (for (>= bitCursor 0) (post -- bitCursor)
  (block (expr (call (member bits unshift) (& fraction 1))) (expr (= fraction (call (member Math floor) (/ fraction 2))))))
 (while (> exponentWidth 0)
  (block (expr (call (member bits unshift) (& exponent 1))) (expr (pre -- exponentWidth)) (expr (= exponent (call (member Math floor) (/ exponent 2))))))
 (expr (call (member bits unshift) (? sign 1 0)))
 (expr (call (member output push) tag))
 (let (byteCursor 0))
 (while (< byteCursor (member bits length))
  (block
   (expr (call (member output push) (| (| (| (| (| (| (| (<< (index bits byteCursor) 7) (<< (index bits (+ byteCursor 1)) 6)) (<< (index bits (+ byteCursor 2)) 5)) (<< (index bits (+ byteCursor 3)) 4)) (<< (index bits (+ byteCursor 4)) 3)) (<< (index bits (+ byteCursor 5)) 2)) (<< (index bits (+ byteCursor 6)) 1)) (index bits (+ byteCursor 7)))))
   (expr (+= byteCursor 8))
  )
 )
)"#;

pub(in crate::naming) fn evaluate(model: &Model<'_, '_>, site: FunctionSite, f: &Facts<'_>) -> Vec<Proposal> {
    fn prove(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
        let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
            return None;
        };
        // f.id on an expression is private self, never exposure evidence.
        if !matches!(
            model.nodes.parent_kind(site.node_id),
            AstKind::FunctionBody(_) | AstKind::BlockStatement(_)
        ) || function.r#async
            || function.generator
            || function.params.rest.is_some()
            || function.id.as_ref()?.symbol_id.get() != site.binding_symbol
        {
            return None;
        }
        let [value, output] = function.params.items.as_slice() else {
            return None;
        };
        let body = function.body.as_ref()?;
        if !body.directives.is_empty() || body.statements.len() != 15 {
            return None;
        }
        let mut proof = Proof {
            model,
            site,
            locals: BTreeMap::new(),
            tag: None,
        };
        proof.bind("value", plain_param(model, value)?)?;
        proof.bind("output", plain_param(model, output)?)?;
        let p = protocol();
        proof.statements(p.list()?, &body.statements)?;
        crate::naming::functions::encoding::numeric_byte_writers::software_float_capture_ready(model, site, proof.tag?)
    }
    if prove(model, site).is_none() {
        return Vec::new();
    }
    vec![proposal("writeTaggedArithmeticFloatBytes", "shared_software_float_arithmetic_writer",
        "complete exact arithmetic lowering: signed-zero sign test, abs/log/LN2 normalization, explicit half/parity rounding, overflow and both subnormal branches, literal 536870911 mask and divisor, exponent adjustment before narrow subnormal branch, exact widths, MSB-first unshift and eight-bit push order; Math is unresolved global; every local identity and effect consumed, capture numeric values/all writers/nonescape and callable exposures proved; no IEEE/native identity asserted", f)]
}
