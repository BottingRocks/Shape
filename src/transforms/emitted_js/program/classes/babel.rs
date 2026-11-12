//! Babel's compiled constructors (`_classCallCheck`, `_createClass`) back to `class`.

use super::*;
use crate::transforms::emitted_js::support::is_void_zero;

/// Babel's derived constructor, read back:
///
/// ```js
/// if (!(this instanceof K)) { throw new TypeError("Cannot call a class as a function"); }
/// …checks that do not touch this…
/// let a = (K["__proto__"] || Object["getPrototypeOf"](K))["call"](this, …args);
/// if (this) {
///     a = a && (typeof a === "object" || typeof a === "function") ? a : this;
///     …body on a…
///     return a;
/// } else {
///     throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
/// }
/// ```
///
/// `a` is `this` once `super(…args)` ran: the body reads it as `this`.
pub(super) struct BabelConstructor {
    /// The variable the constructor checks `this` against: the class.
    pub(super) alias: SymbolId,
    /// The constructor's statements as class text: checks, `super(…)`,
    /// the body. Lines after a statement's first carry the constructor
    /// body's indentation.
    pub(super) body: Vec<String>,
}

/// Babel's `_inherits` inlined after `K = C;`:
/// `if (typeof S !== "function" && S !== null) { throw … } else { … }`.
struct Inherits<'t, 'a> {
    statement: &'t IfStatement<'a>,
    /// The text of `S`.
    base: &'t str,
    /// The else block's five fixed statements that link the prototypes.
    fixed: [&'t Statement<'a>; 5],
    /// The else block's statements after them.
    rest: &'t [Statement<'a>],
}

/// A descriptor (`key` and its stored functions) or an array of them.
#[derive(Clone)]
enum Value {
    Descriptor(String, Vec<(&'static str, SymbolId)>),
    Array(Vec<Value>),
}

type Values = std::collections::HashMap<SymbolId, Value>;

/// `{ key: "k", value: void 0 }`: the key.
fn plain_descriptor(object: &ObjectExpression<'_>) -> Option<String> {
    let mut key = None;
    let mut plain = true;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            plain = false;
            break;
        };
        let name = match &property.key {
            PropertyKey::StaticIdentifier(name) => name.name.to_string(),
            PropertyKey::StringLiteral(name) => name.value.to_string(),
            _ => {
                plain = false;
                break;
            }
        };
        match (name.as_str(), property.value.get_inner_expression()) {
            ("key", Expression::StringLiteral(value)) => key = Some(value.value.to_string()),
            ("value", value) if is_void_zero(value) => {}
            _ => plain = false,
        }
    }
    if let (Some(key), true) = (key, plain) {
        Some(key)
    } else {
        None
    }
}

impl<'s, 'a> Scan<'s, 'a> {
    pub(super) fn babel_constructor(&self, constructor: &Function<'a>) -> Option<BabelConstructor> {
        if !strict(constructor) || !constructor.params.items.is_empty() {
            return None;
        }
        let statements = &constructor.body.as_ref()?.statements;
        let [check, rest @ ..] = statements.as_slice() else {
            return None;
        };
        let Statement::IfStatement(check) = check else {
            return None;
        };
        let Expression::UnaryExpression(not) = check.test.get_inner_expression() else {
            return None;
        };
        let Expression::BinaryExpression(instance) = not.argument.get_inner_expression() else {
            return None;
        };
        if !matches!(instance.left.get_inner_expression(), Expression::ThisExpression(_)) || check.alternate.is_some() {
            return None;
        }
        let alias = self.symbol(&instance.right)?;
        let alias_name = self.scoping.symbol_name(alias);
        if !text(self.source, check.consequent.span()).contains("Cannot call a class as a function") {
            return None;
        }
        // The super call, after checks that do not touch `this`.
        let call_at = rest.iter().position(|statement| matches!(statement, Statement::VariableDeclaration(_)))?;
        let (checks, rest) = rest.split_at(call_at);
        let [Statement::VariableDeclaration(declaration), Statement::IfStatement(initialised)] = rest else {
            return None;
        };
        let mut this_uses = ThisUses::default();
        for statement in checks {
            this_uses.visit_statement(statement);
        }
        if this_uses.found {
            return None;
        }
        let [declarator] = declaration.declarations.as_slice() else {
            return None;
        };
        let BindingPattern::BindingIdentifier(result) = &declarator.id else {
            return None;
        };
        let Expression::CallExpression(call) = declarator.init.as_ref()?.get_inner_expression() else {
            return None;
        };
        if squeeze(text(self.source, call.callee.span())) != format!("({alias_name}[\"__proto__\"]||Object[\"getPrototypeOf\"]({alias_name}))[\"call\"]") {
            return None;
        }
        let [Argument::ThisExpression(_), arguments @ ..] = call.arguments.as_slice() else {
            return None;
        };
        let arguments: Vec<&str> = arguments.iter().map(|argument| text(self.source, argument.span())).collect();
        // if (this) { a = …possibleConstructorReturn…; body; return a; } else { throw … }
        let result_name = result.name.as_str();
        if !matches!(initialised.test.get_inner_expression(), Expression::ThisExpression(_))
            || !initialised.alternate.as_ref().is_some_and(|alternate| text(self.source, alternate.span()).contains("super() hasn't been called"))
        {
            return None;
        }
        let Statement::BlockStatement(block) = &initialised.consequent else {
            return None;
        };
        let [returned, body @ .., last] = block.body.as_slice() else {
            return None;
        };
        let expected = format!("{result_name}={result_name}&&(typeof{result_name}===\"object\"||typeof{result_name}===\"function\")?{result_name}:this;");
        if squeeze(text(self.source, returned.span())) != expected || squeeze(text(self.source, last.span())) != format!("return{result_name};") {
            return None;
        }
        // `a` in the body: read directly in the constructor, never written.
        let mut replace: Vec<Span> = Vec::new();
        for reference in self.scoping.get_resolved_references(result.symbol_id()) {
            let span = self.span(reference.node_id());
            if !body.iter().any(|statement| statement.span().contains_inclusive(span)) {
                continue;
            }
            let own = self.nodes.ancestor_ids(reference.node_id()).find_map(|ancestor| match self.nodes.kind(ancestor) {
                AstKind::Function(function) => Some(function.span == constructor.span),
                AstKind::ArrowFunctionExpression(_) => None,
                _ => None,
            });
            if reference.is_write() || own != Some(true) {
                return None;
            }
            replace.push(span);
        }
        replace.sort_by_key(|span| span.start);
        let mut lines: Vec<String> = checks.iter().map(|statement| text(self.source, statement.span()).to_string()).collect();
        lines.push(format!("super({});", arguments.join(", ")));
        for statement in body {
            let span = statement.span();
            let mut out = String::new();
            let mut at = span.start;
            for name in replace.iter().filter(|name| span.contains_inclusive(**name)) {
                out.push_str(&self.source[at as usize..name.start as usize]);
                out.push_str("this");
                at = name.end;
            }
            out.push_str(&self.source[at as usize..span.end as usize]);
            lines.push(dedent(&out));
        }
        Some(BabelConstructor { alias, body: lines })
    }

    /// Babel's `_createClass(C, protoProps, staticProps)`: the variable's
    /// only value is a thread's result, and that thread returns one that
    /// defines `arguments[1]` on `arguments[0].prototype` and `arguments[2]`
    /// on `arguments[0]`, each non-enumerable and configurable.
    pub(super) fn create_class(&self, callee: &Expression<'_>) -> bool {
        let Some(symbol) = self.symbol(callee) else {
            return false;
        };
        if self.scoping.get_resolved_references(symbol).any(|reference| reference.is_write()) {
            return false;
        }
        let AstKind::VariableDeclarator(declarator) = self.nodes.kind(self.scoping.symbol_declaration(symbol)) else {
            return false;
        };
        let Some(Expression::CallExpression(call)) = declarator.init.as_ref().map(|init| init.get_inner_expression()) else {
            return false;
        };
        let Some(factory) = self.symbol(&call.callee) else {
            return false;
        };
        let AstKind::Function(factory) = self.nodes.kind(self.scoping.symbol_declaration(factory)) else {
            return false;
        };
        let Some(Statement::ReturnStatement(returned)) = factory.body.as_ref().and_then(|body| body.statements.last()) else {
            return false;
        };
        let Some(helper) = returned.argument.as_ref().and_then(|argument| self.symbol(argument)) else {
            return false;
        };
        let AstKind::Function(helper) = self.nodes.kind(self.scoping.symbol_declaration(helper)) else {
            return false;
        };
        let body = squeeze(text(self.source, helper.span));
        ["Object[\"defineProperty\"](arguments[0][\"prototype\"],arguments[1][", "Object[\"defineProperty\"](arguments[0],arguments[2][", "[\"configurable\"]=true", "[\"enumerable\"]||false", "\"value\"in", "returnarguments[0];"]
            .iter()
            .all(|needle| body.contains(needle))
    }

    /// The descriptor arrays built before a `_createClass` call:
    /// `let d = { key: "k", value: void 0 }`, `d["value"] = M`, `a = [d]`,
    /// `a[n] = d`, `[d]` inline. Returns the statements they take, the call,
    /// and the members.
    pub(super) fn create_class_block(&self, statements: &[Statement<'a>], holders: &[SymbolId]) -> Option<(Vec<usize>, Vec<Descriptor>)> {
        let mut values: Values = std::collections::HashMap::new();
        let mut taken: Vec<usize> = Vec::new();
        for (at, statement) in statements.iter().enumerate() {
            match statement {
                Statement::VariableDeclaration(declaration) => {
                    let [declarator] = declaration.declarations.as_slice() else { continue };
                    let BindingPattern::BindingIdentifier(id) = &declarator.id else { continue };
                    let Some(Expression::ObjectExpression(object)) = declarator.init.as_ref().map(|init| init.get_inner_expression()) else { continue };
                    if let Some(key) = plain_descriptor(object) {
                        values.insert(id.symbol_id(), Value::Descriptor(key, Vec::new()));
                        taken.push(at);
                    }
                }
                Statement::ExpressionStatement(expression) => match &expression.expression {
                    Expression::AssignmentExpression(assignment) if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign => {
                        self.descriptor_store(assignment, at, &mut values, &mut taken)?;
                    }
                    // _createClass(C, protoProps, staticProps)
                    Expression::CallExpression(call) if self.create_class(&call.callee) => {
                        return self.create_class_call(call, at, statements, holders, &values, taken);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        None
    }

    /// A store building a descriptor or an array of them: `a = [d]`,
    /// `a[n] = d`, `d["value"] = M`. `None` when the block cannot be read.
    fn descriptor_store(&self, assignment: &AssignmentExpression<'a>, at: usize, values: &mut Values, taken: &mut Vec<usize>) -> Option<()> {
        match &assignment.left {
            // a = [d]
            AssignmentTarget::AssignmentTargetIdentifier(target) => {
                let Some(symbol) = self.scoping.get_reference(target.reference_id()).symbol_id() else { return Some(()) };
                if !values.contains_key(&symbol) {
                    return Some(());
                }
                let items = self.descriptor_array(&assignment.right, values)?;
                values.insert(symbol, Value::Array(items));
                taken.push(at);
            }
            target => {
                let Some(member) = target.as_member_expression() else { return Some(()) };
                // a[n] = d
                if let MemberExpression::ComputedMemberExpression(indexed) = member {
                    if let Expression::NumericLiteral(index) = &indexed.expression {
                        let Some(symbol) = self.symbol(&indexed.object) else { return Some(()) };
                        if !values.contains_key(&symbol) {
                            return Some(());
                        }
                        let descriptor = match values.get(&self.symbol(&assignment.right)?)? {
                            Value::Descriptor(key, fields) => Value::Descriptor(key.clone(), fields.clone()),
                            Value::Array(_) => return None,
                        };
                        let Some(Value::Array(items)) = values.get_mut(&symbol) else { return None };
                        if index.value != items.len() as f64 {
                            return None;
                        }
                        items.push(descriptor);
                        taken.push(at);
                        return Some(());
                    }
                }
                let Some((object, key)) = member_key(member) else { return Some(()) };
                let Some(symbol) = self.symbol(object) else { return Some(()) };
                // d["value"] = M
                if let Some(Value::Descriptor(_, fields)) = values.get_mut(&symbol) {
                    let kind = match key.as_str() {
                        "value" => "",
                        "get" => "get ",
                        "set" => "set ",
                        _ => return None,
                    };
                    fields.push((kind, self.symbol(&assignment.right)?));
                    taken.push(at);
                }
            }
        }
        Some(())
    }

    /// `[d, …]` or an array variable's descriptors.
    fn descriptor_array(&self, expression: &Expression<'_>, values: &Values) -> Option<Vec<Value>> {
        match expression.get_inner_expression() {
            Expression::ArrayExpression(array) => array
                .elements
                .iter()
                .map(|element| match values.get(&self.symbol(element.as_expression()?)?)? {
                    Value::Descriptor(key, fields) => Some(Value::Descriptor(key.clone(), fields.clone())),
                    Value::Array(_) => None,
                })
                .collect(),
            other => match values.get(&self.symbol(other)?)? {
                Value::Array(items) => Some(items.clone()),
                Value::Descriptor(..) => None,
            },
        }
    }

    /// `_createClass(C, protoProps, staticProps)` ending the block: its
    /// members, when the descriptors are read nowhere else.
    fn create_class_call(
        &self,
        call: &CallExpression<'a>,
        at: usize,
        statements: &[Statement<'a>],
        holders: &[SymbolId],
        values: &Values,
        mut taken: Vec<usize>,
    ) -> Option<(Vec<usize>, Vec<Descriptor>)> {
        let mut members = Vec::new();
        let [class, rest @ ..] = call.arguments.as_slice() else { return None };
        if !self.symbol(class.as_expression()?).is_some_and(|symbol| holders.contains(&symbol)) || rest.len() > 2 {
            return None;
        }
        for (index, side) in rest.iter().enumerate() {
            let side = side.as_expression()?;
            if is_void_zero(side) {
                continue;
            }
            for item in self.descriptor_array(side, values)? {
                let Value::Descriptor(key, fields) = item else { return None };
                for (kind, function) in fields {
                    members.push((key.clone(), kind, index == 0, function));
                }
            }
        }
        taken.push(at);
        // All or nothing: every descriptor lives only in
        // these statements, and each function stored in
        // one is a member.
        let spans: Vec<Span> = taken.iter().map(|&at| statements[at].span()).collect();
        for &symbol in values.keys() {
            if self.references(symbol).iter().any(|&node| {
                let span = self.span(node);
                !spans.iter().any(|outer| outer.contains_inclusive(span))
            }) {
                return None;
            }
        }
        let stored: usize = taken
            .iter()
            .filter(|&&at| {
                let Statement::ExpressionStatement(expression) = &statements[at] else { return false };
                matches!(&expression.expression, Expression::AssignmentExpression(assignment)
                    if assignment.left.as_member_expression().and_then(member_key).is_some_and(|(_, key)| matches!(key.as_str(), "value" | "get" | "set")))
            })
            .count();
        if stored != members.len() {
            return None;
        }
        Some((taken, members))
    }

    pub(super) fn babel_class(&mut self, statements: &[Statement<'a>], constructor: &Function<'a>, container: Span) -> Option<()> {
        let babel = self.babel_constructor(constructor)?;
        let class = constructor.id.as_ref()?.symbol_id();
        let name = constructor.id.as_ref()?.name.as_str();
        let Inherits { statement: inherits, base, fixed, rest } = self.inherits(statements, constructor, name, babel.alias)?;
        // _createClass and its descriptors.
        let (taken, members) = self.babel_members(rest, class, babel.alias)?;
        // Uses the rewrite removes do not count.
        let mut ignore: Vec<Span> = vec![constructor.span, inherits.test.span(), inherits.consequent.span()];
        ignore.extend(fixed.iter().map(|statement| statement.span()));
        ignore.extend(taken.iter().map(|&at| rest[at].span()));
        let Some(holders) = self.holders_except(class, &ignore) else {
            // Refused: a use a class does not allow.
            return None;
        };
        if !holders.contains(&babel.alias) {
            return None;
        }
        for reference in self.scoping.get_resolved_references(class) {
            let node = reference.node_id();
            let span = self.span(node);
            if ignore.iter().any(|outer| outer.contains_inclusive(span)) {
                continue;
            }
            if span.start < container.start || span.end > container.end || !self.runs_after(node, constructor.span.end, 3) {
                // Refused: used before the class or outside its list.
                return None;
            }
        }

        // The class at the constructor's place.
        let line_start = self.source[..constructor.span.start as usize].rfind('\n').map_or(0, |at| at + 1);
        let indent = &self.source[line_start..constructor.span.start as usize];
        if !indent.trim().is_empty() {
            return None;
        }
        let out = self.babel_class_text(indent, name, base, &babel, &members, rest)?;
        self.edits.push(analysis::Edit::replace(constructor.span, out));
        // The inherits `if` gives way to what its else block keeps.
        let replacement = self.inherits_remainder(rest, &members, &taken, &holders, indent);
        if replacement.is_empty() {
            self.edits.push(analysis::Edit::delete(whole_line(self.source, inherits.span)));
        } else {
            self.edits.push(analysis::Edit::replace(inherits.span, replacement));
        }
        self.count += 1;
        Some(())
    }

    /// `K = C;` then Babel's inherits block, matched statement by statement.
    fn inherits<'t>(&self, statements: &'t [Statement<'a>], constructor: &Function<'a>, name: &str, alias: SymbolId) -> Option<Inherits<'t, 'a>>
    where
        's: 't,
    {
        let alias = self.scoping.symbol_name(alias);
        // K = C;
        let link_at = statements.iter().position(|statement| squeeze(text(self.source, statement.span())) == format!("{alias}={name};"))?;
        // if (typeof S !== "function" && S !== null) { throw … } else { inherits; rest }
        let Statement::IfStatement(inherits) = statements.get(link_at + 1)? else {
            return None;
        };
        let Expression::LogicalExpression(test) = inherits.test.get_inner_expression() else {
            return None;
        };
        let Expression::BinaryExpression(not_null) = test.right.get_inner_expression() else {
            return None;
        };
        let base = text(self.source, not_null.left.span());
        if squeeze(text(self.source, inherits.test.span())) != squeeze(&format!("typeof {base} !== \"function\" && {base} !== null"))
            || !text(self.source, inherits.consequent.span()).contains("Super expression must either be null or a function")
        {
            return None;
        }
        let pure_base = matches!(not_null.left.get_inner_expression(), Expression::Identifier(_))
            || matches!(not_null.left.get_inner_expression(), Expression::ComputedMemberExpression(member) if matches!(member.object.get_inner_expression(), Expression::Identifier(arguments) if arguments.name == "arguments") && matches!(member.expression, Expression::NumericLiteral(_)));
        if !pure_base {
            return None;
        }
        let Some(Statement::BlockStatement(block)) = &inherits.alternate else {
            return None;
        };
        let [descriptor, prototype, constructor_store, create, set_prototype, rest @ ..] = block.body.as_slice() else {
            return None;
        };
        let expected = [
            format!("let{{}}={{[\"value\"]:{name},[\"enumerable\"]:false,[\"writable\"]:true,[\"configurable\"]:true}};"),
            "let{}={};".to_string(),
            "{}[\"constructor\"]={};".to_string(),
            format!("{name}[\"prototype\"]=Object[\"create\"]({}&&{}[\"prototype\"],{{}});", squeeze(base), squeeze(base)),
            format!("if({}){{if(Object[\"setPrototypeOf\"]){{Object[\"setPrototypeOf\"]({name},{});}}else{{{name}[\"__proto__\"]={};}}}}", squeeze(base), squeeze(base), squeeze(base)),
        ];
        // Compare with the generated names blanked out.
        let blank = |statement: &Statement<'_>| {
            let mut out = squeeze(text(self.source, statement.span()));
            for from in [descriptor, prototype] {
                if let Statement::VariableDeclaration(declaration) = from {
                    if let Some(id) = declaration.declarations.first().and_then(|d| d.id.get_binding_identifier()) {
                        out = out.replace(id.name.as_str(), "{}");
                    }
                }
            }
            out
        };
        let fixed = [descriptor, prototype, constructor_store, create, set_prototype];
        for (statement, expected) in fixed.into_iter().zip(&expected) {
            if &blank(statement) != expected {
                // Refused: the inherits block differs.
                return None;
            }
        }
        Some(Inherits { statement: inherits, base, fixed, rest })
    }

    /// The `_createClass` block in the inherits block's rest: the statements
    /// it takes, and each member with the index of its method thread.
    fn babel_members(&self, rest: &[Statement<'a>], class: SymbolId, alias: SymbolId) -> Option<(Vec<usize>, Vec<(String, &'static str, bool, usize)>)> {
        let holders_guess: Vec<SymbolId> = {
            let mut holders = vec![class, alias];
            for statement in rest {
                if let Some(value) = self.copy(statement) {
                    if holders.contains(&value) {
                        if let Statement::VariableDeclaration(declaration) = statement {
                            if let Some(id) = declaration.declarations.first().and_then(|d| d.id.get_binding_identifier()) {
                                holders.push(id.symbol_id());
                            }
                        }
                    }
                }
            }
            holders
        };
        let (taken, found) = self.create_class_block(rest, &holders_guess)?;
        let mut members = Vec::new();
        for (key, kind, instance, function) in found {
            let at = rest.iter().position(|statement| thread(statement).is_some_and(|f| f.id.as_ref().is_some_and(|id| id.symbol_id() == function)))?;
            let method = thread(&rest[at])?;
            if self.scoping.get_resolved_references(function).count() != 1 || !self.strict_safe(method) || matches!(key.as_str(), "constructor" | "prototype") {
                return None;
            }
            members.push((key, kind, instance, at));
        }
        Some((taken, members))
    }

    /// `class C extends S { constructor() { … } methods }` at `indent`.
    fn babel_class_text(
        &self,
        indent: &str,
        name: &str,
        base: &str,
        babel: &BabelConstructor,
        members: &[(String, &'static str, bool, usize)],
        rest: &[Statement<'a>],
    ) -> Option<String> {
        let mut out = format!("class {name} extends {base} {{\n{indent}\tconstructor() {{\n");
        for statement in &babel.body {
            // The constructor body sat one level in; the class's sits two.
            let statement = statement.replace('\n', "\n\t");
            out.push_str(&format!("{indent}\t\t{statement}\n"));
        }
        out.push_str(&format!("{indent}\t}}\n"));
        for (key, kind, instance, at) in members {
            let function = thread(&rest[*at])?;
            let key = if analysis::is_identifier_name(key) { key.clone() } else { format!("[{key:?}]") };
            let head = format!("{}{kind}{key}", if *instance { "" } else { "static " });
            // The methods sat inside the else block, as deep as a class body.
            out.push_str(&self.element_lines(indent, &head, function, &[], "")?);
        }
        out.push_str(indent);
        out.push('}');
        Some(out)
    }

    /// The inherits block's statements that stay, dedented and joined at
    /// `indent`: all but the moved methods, the `_createClass` block, and
    /// copies of the class only that block read.
    fn inherits_remainder(
        &self,
        rest: &[Statement<'a>],
        members: &[(String, &'static str, bool, usize)],
        taken: &[usize],
        holders: &[SymbolId],
        indent: &str,
    ) -> String {
        let mut moved: HashSet<usize> = members.iter().map(|(_, _, _, at)| *at).chain(taken.iter().copied()).collect();
        // A copy of the class only the `_createClass` call read goes too.
        let taken_spans: Vec<Span> = taken.iter().map(|&at| rest[at].span()).collect();
        for (at, statement) in rest.iter().enumerate() {
            let Statement::VariableDeclaration(declaration) = statement else { continue };
            let [declarator] = declaration.declarations.as_slice() else { continue };
            let Some(id) = declarator.id.get_binding_identifier() else { continue };
            if !declarator.init.as_ref().and_then(|init| self.symbol(init)).is_some_and(|value| holders.contains(&value)) {
                continue;
            }
            if self.references(id.symbol_id()).iter().all(|&node| taken_spans.iter().any(|outer| outer.contains_inclusive(self.span(node)))) {
                moved.insert(at);
            }
        }
        let kept: Vec<String> = rest
            .iter()
            .enumerate()
            .filter(|(at, _)| !moved.contains(at))
            .map(|(_, statement)| dedent(text(self.source, statement.span())))
            .collect();
        kept.join(&format!("\n{indent}"))
    }
}

/// `this` in the code itself, not in nested functions.
#[derive(Default)]
pub(super) struct ThisUses {
    pub(super) found: bool,
}

impl<'a> Visit<'a> for ThisUses {
    fn visit_this_expression(&mut self, _: &ThisExpression) {
        self.found = true;
    }
    fn visit_function(&mut self, _: &Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
}
