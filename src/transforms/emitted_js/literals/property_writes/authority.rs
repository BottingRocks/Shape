//! Which callee is the native `Object.defineProperty`: the authority a rewrite needs.

use super::*;

pub(super) struct NativeAuthority {
    pub(super) declined: Option<(PropertyWriteSkipReason, String)>,
}

impl NativeAuthority {
    pub(super) fn evaluate(scoping: &Scoping, nodes: &AstNodes<'_>, scan: &ScanCollector<'_>) -> Self {
        if scan.saw_with || scan.saw_eval_reference {
            return Self {
                declined: Some((
                    PropertyWriteSkipReason::DynamicScope,
                    "with/eval can observe bindings and run dynamic code; the input was returned unchanged"
                        .into(),
                )),
            };
        }
        if scan.saw_this {
            return Self {
                declined: Some((
                    PropertyWriteSkipReason::GlobalObjectEscape,
                    "`this` can be the global object and reach the native Object binding; the input was returned unchanged"
                        .into(),
                )),
            };
        }
        if scan.saw_constructor_access || scan.saw_unknown_computed_call {
            return Self {
                declined: Some((
                    PropertyWriteSkipReason::DynamicCode,
                    "a `constructor` access can reach the Function constructor and run dynamic code"
                        .into(),
                )),
            };
        }

        let mut allowed_object_starts = BTreeSet::new();
        for (object_span, member_span) in &scan.define_property_members {
            if !scan
                .write_spans
                .iter()
                .any(|write| span_contains(*write, *member_span))
            {
                allowed_object_starts.insert(object_span.start);
            }
        }

        for (name, references) in scoping.root_unresolved_references().iter() {
            let name = name.as_str();
            if matches!(
                name,
                "globalThis" | "window" | "self" | "global" | "top" | "parent" | "frames"
            ) {
                return Self {
                    declined: Some((
                        PropertyWriteSkipReason::GlobalObjectEscape,
                        format!(
                            "the global object can be reached through `{name}`, so native Object.defineProperty cannot be proven"
                        ),
                    )),
                };
            }
            if name == "Function" {
                return Self {
                    declined: Some((
                        PropertyWriteSkipReason::DynamicCode,
                        "the Function constructor can run dynamic code that patches native Object.defineProperty"
                            .into(),
                    )),
                };
            }
            if name != "Object" {
                continue;
            }
            for reference_id in references.iter().copied() {
                let reference = scoping.get_reference(reference_id);
                let span = nodes.get_node(reference.node_id()).kind().span();
                if !allowed_object_starts.contains(&span.start) {
                    return Self {
                        declined: Some((
                            PropertyWriteSkipReason::NativeObjectEscaped,
                            format!(
                                "the bare global Object is written, aliased, or escaped at bytes {}..{}; native Object.defineProperty cannot be proven",
                                span.start, span.end
                            ),
                        )),
                    };
                }
            }
        }

        Self { declined: None }
    }
}

pub(super) fn span_contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}
