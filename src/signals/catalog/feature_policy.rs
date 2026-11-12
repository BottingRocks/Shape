//! `FeaturePolicy`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, LeafView, SignalLayout, SlotView, ValueSlice};
use crate::signals::analysis::{shape_text::*, leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField, CardMessage};


/// `FeaturePolicy`. Card: [`CARD`].
pub(crate) fn recognize_feature_policy(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = leaves_of(view);
    // `policy.features()` on a host-rooted local alias of
    // `document.featurePolicy` is either a transform of the policy read or a
    // host call path of its own (`<alias>.features()`).
    let calls = |leaf: &LeafView, method: &str| {
        has_transform(leaf, &format!(".{method}()")) || has_api(leaf, &format!("{method}()"))
    };
    let lists = leaves.iter().any(|leaf| calls(leaf, "features"))
        && leaves.iter().any(|leaf| calls(leaf, "allowedFeatures"));
    // One count of each list on its own (companion slots that share the
    // registers carry both lists at once).
    let count_of = |list: &str, other: &str| {
        leaves.iter().any(|leaf| {
            leaf.op == LeafOp::Number
                && (has_api(leaf, "document.featurePolicy") || has_api(leaf, &format!("{list}()")))
                && has_transform(leaf, ".length")
                && calls(leaf, list)
                && !calls(leaf, other)
        })
    };
    // A register reused for the cursor can carry the other list into the
    // count's provenance; the write site itself still reads `.length` of a
    // list defined from one call only.
    let sliced_count = |list: &str, other: &str| {
        leaves.iter().any(|leaf| {
            leaf.op == LeafOp::Number
                && has_api(leaf, "document.featurePolicy")
                && ctx.value_slice(leaf, 1).map(|slice| with_sibling_arms(ctx, slice)).is_some_and(|slice| {
                    slice.members.get("length") == Some(&0)
                        && slice.members.get(list) == Some(&1)
                        && slice.members.get(other).is_none_or(|depth| *depth > 1)
                })
        })
    };
    let counts = (count_of("features", "allowedFeatures") || sliced_count("features", "allowedFeatures"))
        && (count_of("allowedFeatures", "features") || sliced_count("allowedFeatures", "features"));
    let digest = leaves.iter().any(|leaf| {
        leaf.op == LeafOp::Number && has_transform(leaf, ".join()") && calls(leaf, "features") && calls(leaf, "allowedFeatures")
    });
    // Family A has four fields, family B five (sampling seed). Companion
    // slots sharing registers can pick up featurePolicy provenance, so the
    // field count is part of the evidence.
    let shapes = view.shapes();
    let fields = (top_count_is(&shapes, 4) || top_count_is(&shapes, 5)) && max_top_count(&shapes) <= 5;
    if lists && digest && counts && fields {
        return claim(&["4-5 fields", "counts and digest of document.featurePolicy features()/allowedFeatures()"]);
    }
    None
}

pub(crate) fn bind_feature_policy(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // Family A (4 fields) and family B (+ sampling seed) share the names.
    // Leaf provenance is shared with the seed's registers, so the numbers are
    // told apart at their write sites: the digest calls `join` in its own
    // expression, a count reads `.length` of a list defined from
    // `features()` / `allowedFeatures()`, the seed does neither.
    // The digest joins in its own expression or in the strings it hashes
    // (`hash(a + b)` with `a = list.join("|") + "||"`).
    let joins = |slice: &ValueSlice| slice.calls.iter().any(|((method, _, _), depth)| method == "join" && *depth <= 1);
    let length = |slice: &ValueSlice| slice.members.get("length") == Some(&0);
    let hash = |f: &Field, slice: &ValueSlice| f.is_plain(LeafKind::Number) && joins(slice);
    let count = |list: &'static str, other: &'static str| {
        move |f: &Field, slice: &ValueSlice| {
            let slice = &with_sibling_arms(ctx, slice.clone());
            f.is_plain(LeafKind::Number)
                && !joins(slice)
                && length(slice)
                && slice.members.get(list).is_some_and(|depth| *depth == 1)
                && slice.members.get(other).is_none_or(|depth| *depth > 1)
        }
    };
    let allowed = count("allowedFeatures", "features");
    let features = count("features", "allowedFeatures");
    let entry = |f: &Field, _: &ValueSlice| f.writes(LeafKind::String);
    let seed = |f: &Field, slice: &ValueSlice| f.is_plain(LeafKind::Number) && !joins(slice) && !length(slice);
    solve_by_slices(
        view,
        layout,
        ctx,
        1,
        &[
            ("featureListHash", &hash),
            ("allowedFeatureCount", &allowed),
            ("featureCount", &features),
            ("allowlistEntry", &entry),
            ("samplingSeed", &seed),
        ],
    )
}

/// `slice` plus the definitions in the other arm of an `if` whose arm holds
/// a followed definition (`let l; if (c) l = p.features(); else l = [];`):
/// the nearest-definition walk keeps only the later arm. The other arm's
/// facts take the followed definition's depth.
fn with_sibling_arms(ctx: &FinderCtx, mut slice: ValueSlice) -> ValueSlice {
    use oxc_ast::ast::AssignmentTarget;
    use oxc_ast::AstKind;
    use oxc_span::GetSpan;
    let nodes = ctx.parse.nodes;
    let mut extra = ValueSlice::default();
    for (symbol, definition, depth) in slice.definitions.clone() {
        let mut child = definition;
        let mut other = None;
        for _ in 0..4 {
            let parent = nodes.parent_id(child);
            if parent == child {
                break;
            }
            if let AstKind::IfStatement(statement) = nodes.kind(parent) {
                let child_span = nodes.kind(child).span();
                other = match &statement.alternate {
                    Some(alternate) if statement.consequent.span() == child_span => Some(alternate.span()),
                    Some(alternate) if alternate.span() == child_span => Some(statement.consequent.span()),
                    _ => None,
                };
                break;
            }
            if matches!(nodes.kind(parent), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)) {
                break;
            }
            child = parent;
        }
        let Some(other) = other else { continue };
        for id in ctx.nodes_within(other) {
            let AstKind::AssignmentExpression(assignment) = nodes.kind(id) else { continue };
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else { continue };
            if !assignment.operator.is_assign() || ctx.identifier_symbol(target) != Some(symbol) {
                continue;
            }
            let Some(right) = ctx.node_with_span(assignment.right.span()) else { continue };
            let arm = ctx.slice_of(right, 0);
            for (name, inner) in arm.members {
                let entry = extra.members.entry(name).or_insert(depth + inner);
                *entry = (*entry).min(depth + inner);
            }
            for (call, inner) in arm.calls {
                let entry = extra.calls.entry(call).or_insert(depth + inner);
                *entry = (*entry).min(depth + inner);
            }
        }
    }
    slice.absorb(&extra);
    slice
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `FeaturePolicy`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "FeaturePolicy",
    summary: "Reads document.featurePolicy (Chromium only). It sends the number of features the browser knows, the number allowed on this page, a hash of both feature lists, and the first allowlist entry (an origin such as '*' or 'https://...') of one randomly chosen allowed feature. If there is no featurePolicy object, nothing is sent.",
    why: "The feature lists differ by browser, version and embedding context, so they help verify the claimed browser and detect engines or headless builds that fake being Chrome (likely).",
    sample_payload: Some(r#"{
  "allowedFeatureCount": 76,
  "allowlistEntry": "https://www.target.com",
  "featureCount": 77,
  "featureListHash": -667729372,
  "samplingSeed": 0.5477980646993165
}"#),
    by_name: false,
    body: CardBody::Messages(&[
        CardMessage {
            when: "family A (puma builds, including the main build): the feature is picked with Math.random.",
            fields: &[
                CardField::number(
                    "allowedFeatureCount",
                    "Number of features allowedFeatures() returns (0 if the method is missing).",
                ),
                CardField::number(
                    "featureListHash",
                    "Signed 32-bit hash of features() joined with '|', then '||', then allowedFeatures() joined with '|'.",
                ),
                CardField::number(
                    "featureCount",
                    "Number of features features() returns (0 if the method is missing).",
                ),
                CardField::string(
                    "allowlistEntry",
                    "First allowlist entry (origin string) for one randomly picked allowed feature, or a null marker if there are no allowed features or no entry.",
                ).nullable(),
            ],
        },
        CardMessage {
            when: "family B (all other vendors): the feature is picked from a shared random pool, and the pool value used is also sent.",
            fields: &[
                CardField::number(
                    "allowedFeatureCount",
                    "Number of features allowedFeatures() returns (0 if the method is missing).",
                ),
                CardField::number(
                    "featureListHash",
                    "Signed 32-bit hash of features() joined with '|', then '||', then allowedFeatures() joined with '|'.",
                ),
                CardField::number(
                    "featureCount",
                    "Number of features features() returns (0 if the method is missing).",
                ),
                CardField::string(
                    "allowlistEntry",
                    "First allowlist entry (origin string) for the picked allowed feature, or a null marker if there are no allowed features or no entry.",
                ).nullable(),
                CardField::number(
                    "samplingSeed",
                    "Random fraction between 0 and 1 that chose the feature, so the receiver can tell which feature the entry belongs to (likely).",
                ),
            ],
        },
    ]),
};
