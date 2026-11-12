//! Expand the compiler's one-line helpers back into the expressions they hide.
//!
//! The VM routes ordinary operations through tiny shared helpers —
//! `$VMHELPER_64(x)` is `x`, `$VMHELPER_158(x)` is `x["push"]`,
//! `$VMHELPER_96(x)` is `x["length"]`, `$VMHELPER_466(a, b, c, d)` is
//! `a[b] = c ^ d()`. A qualifying helper is a never-reassigned
//! `$VMHELPER_`/`vm_sub_` declaration with plain unwritten parameters and no
//! `this`, `arguments`, nested function or class, `await`, `yield`, or
//! comment, whose body is either one `return E;` (expanded at any call), or
//! up to three expression statements with an optional final `return` of a
//! parameter (expanded where the call is a statement of its own). Every
//! other name the body reads must resolve the same at the call.
//!
//! Arguments are evaluated once, in order, before the body, so a parameter
//! is replaced by its argument text only when that keeps the order: a
//! literal or a never-written local that is not a slot may appear anywhere;
//! any other argument must be used exactly once, the uses must follow the
//! argument order, and nothing in the body may run code before such a use.
//! An argument whose parameter is unused must be pure. Nested expandable
//! calls are composed inside one parse. A helper whose every reference was
//! expanded is deleted.
//!
//! A call that is a statement of its own, a declaration's initializer, an
//! assignment's right side, or a `return` value may also expand into
//! statements when its arguments would not keep their order inline: every
//! non-stable argument is first bound, in order, to a fresh `$SSA_x…`
//! temporary; then come the helper's statements; then the original
//! statement with the helper's value (a returned parameter or empty literal,
//! else `void 0`). This is how `$VMHELPER_380({}, "k", v)` (`o[k] = v;
//! return o`) becomes `let t = {}; t["k"] = v; … = t;`, which the literal
//! fill and forwarding stages then fold.

use std::collections::BTreeMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{BinaryOperator, UnaryOperator};
use oxc_syntax::scope::ScopeFlags;

use super::super::inline_tuple_helpers::FreshNames;
use super::super::purity::expression_is_pure;
use crate::analysis;
use super::super::support::whole_line;

mod hoist;
mod found;
mod loops;
mod sites;
#[cfg(test)]
mod tests;

use found::*;
use sites::*;

const MAX_STATEMENTS: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InlinedSmallHelpers {
    pub code: String,
    pub expanded: usize,
}

pub(crate) fn inline_small_helpers(source: &str) -> Result<InlinedSmallHelpers, String> {
    expand_helpers(source, false)
}

/// The last expansion round: every helper call in statement position
/// expands exactly as the call runs — each computed argument bound in order
/// to a temporary, the body, then the returned value — whatever its size,
/// its return, or how it reads its parameters; helpers nothing calls go.
/// The forwarding that follows folds the temporaries it can.
pub(crate) fn inline_remaining_helpers(source: &str) -> Result<InlinedSmallHelpers, String> {
    expand_helpers(source, true)
}

fn expand_helpers(source: &str, exact: bool) -> Result<InlinedSmallHelpers, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let comments: Vec<Span> = program.comments.iter().map(|c| c.span).collect();
    let mut found = Found {
        source,
        scoping: &scoping,
        comments: &comments,
        helpers: BTreeMap::new(),
        dynamic: false,
        exact,
        unused: Vec::new(),
    };
    found.visit_program(&program);
    if found.dynamic {
        return Ok(unchanged(source));
    }
    let unused = std::mem::take(&mut found.unused);
    if found.helpers.is_empty() && unused.is_empty() {
        return Ok(unchanged(source));
    }
    let helpers = found.helpers;
    let mut sites = Sites {
        names: FreshNames::new(source),
        scoping: &scoping,
        nodes: &nodes,
        helpers: &helpers,
        planned: Vec::new(),
        exact,
        declined: Vec::new(),
    };
    sites.visit_program(&program);
    if sites.planned.is_empty() && unused.is_empty() {
        return Ok(unchanged(source));
    }
    let mut planned = sites.planned;
    planned.sort_by_key(|site| (site.span.start, std::cmp::Reverse(site.span.end)));
    // Only outermost sites become edits; nested ones render inside them.
    let mut edits = Vec::new();
    let mut counts: BTreeMap<SymbolId, usize> = BTreeMap::new();
    let mut end = 0;
    for index in 0..planned.len() {
        let site = &planned[index];
        if site.span.start < end {
            continue;
        }
        end = site.span.end;
        edits.push(analysis::Edit::replace(site.span, render_site(source, &helpers, &planned, index)));
    }
    for site in &planned {
        *counts.entry(site.helper).or_default() += 1;
    }
    let mut expanded = unused.len();
    for span in unused {
        edits.push(analysis::Edit::delete(whole_line(source, span)));
    }
    for (helper, count) in &counts {
        expanded += count;
        if scoping.get_resolved_reference_ids(*helper).len() == *count {
            edits.push(analysis::Edit::delete(whole_line(source, helpers[helper].declaration)));
        }
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("small helper expansion edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("small helper expansion output: {error}"))?;
    Ok(InlinedSmallHelpers {
        code: applied.output,
        expanded,
    })
}

/// A statement-shaped helper called inside an expression
/// (`g(a, H(x), b)`) expands only once it is a statement of its own. When
/// nothing the statement evaluates before the call can run code or read
/// through a name the call is handed, the call moves into a declaration
/// just before it (`let t = H(x); g(a, t, b);`), which the expansion and
/// forwarding then fold.
pub(crate) fn hoist_helper_calls(source: &str) -> Result<InlinedSmallHelpers, String> {
    hoist_calls(source, false)
}

/// [`hoist_helper_calls`] for the helpers [`inline_remaining_helpers`] takes.
/// It also hoists one-`return` helpers whose arguments need temporaries
/// (an inner call the body would otherwise run late), innermost first, and
/// binds unconditional code the statement runs before the call to
/// temporaries ahead of it, in order, when no property or changeable name
/// read comes before that code: `let r = k(1)("s", H(a, H(b)))` becomes
/// `let t = k(1); let u = H(b); let r = t("s", H(a, u))`, then the outer
/// call next round.
pub(crate) fn hoist_remaining_helper_calls(source: &str) -> Result<InlinedSmallHelpers, String> {
    hoist_calls(source, true)
}

fn hoist_calls(source: &str, exact: bool) -> Result<InlinedSmallHelpers, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let comments: Vec<Span> = program.comments.iter().map(|c| c.span).collect();
    let mut found = Found {
        source,
        scoping: &scoping,
        comments: &comments,
        helpers: BTreeMap::new(),
        dynamic: false,
        exact,
        unused: Vec::new(),
    };
    found.visit_program(&program);
    if found.dynamic || found.helpers.is_empty() {
        return Ok(unchanged(source));
    }
    let helpers = found.helpers;
    // The exact round also hoists expression helpers whose arguments need
    // temporaries, which only a statement of their own can hold.
    let declined = if exact {
        let mut sites = Sites {
            names: FreshNames::new(source),
            scoping: &scoping,
            nodes: &nodes,
            helpers: &helpers,
            planned: Vec::new(),
            exact,
            declined: Vec::new(),
        };
        sites.visit_program(&program);
        sites.declined
    } else {
        Vec::new()
    };
    let mut hoists = Hoists {
        source,
        scoping: &scoping,
        helpers: &helpers,
        declined: &declined,
        exact,
        names: FreshNames::new(source),
        edits: Vec::new(),
        done: 0,
        nodes: &nodes,
        labels: Vec::new(),
    };
    super::super::support::for_each_list(&program, |list| hoists.list(list));
    if hoists.done == 0 {
        return Ok(unchanged(source));
    }
    let done = hoists.done;
    let applied = analysis::apply_edits(source, hoists.edits);
    if !applied.rejected.is_empty() {
        return Err("helper call hoists overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output).map_err(|error| format!("helper call hoist output: {error}"))?;
    Ok(InlinedSmallHelpers {
        code: applied.output,
        expanded: done,
    })
}

struct Hoists<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    helpers: &'s BTreeMap<SymbolId, Helper>,
    declined: &'s [Span],
    exact: bool,
    names: FreshNames,
    edits: Vec<analysis::Edit>,
    done: usize,
    nodes: &'s AstNodes<'a>,
    /// Labels this round gave rewritten loops.
    labels: Vec<String>,
}

fn unchanged(source: &str) -> InlinedSmallHelpers {
    InlinedSmallHelpers {
        code: source.into(),
        expanded: 0,
    }
}

