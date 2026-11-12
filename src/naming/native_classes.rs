//! Explicit native binding route. A class label requires a whole owned family
//! contract; nearby method/property bags are not class identity evidence.
use crate::signals::analysis::roles::FinderClaim;
use super::*;

pub(super) fn plan(
    program: &Program,
    model: &Model<'_, '_>,
    claims: &[FinderClaim],
    plans: &mut Vec<ProgramBindingRename>,
    rename: bool,
) -> Vec<ThreadClassName> {
    let mut outcomes = Vec::new();
    for node in model.nodes().iter() {
        let AstKind::Class(class) = node.kind() else {
            continue;
        };
        // Expressions/anonymous holders are inventoried but explicitly refused:
        // their inner self and outer holder are distinct lexical identities.
        let symbol = class
            .id
            .as_ref()
            .filter(|id| id.name.starts_with("$THREAD_"))
            .and_then(|id| id.symbol_id.get())
            .or_else(|| {
                let AstKind::VariableDeclarator(decl) = model.nodes().parent_kind(node.id()) else {
                    return None;
                };
                let BindingPattern::BindingIdentifier(id) = &decl.id else {
                    return None;
                };
                id.symbol_id.get()
            });
        let Some(symbol) = symbol else { continue };
        let binding = model.binding_ref(symbol);
        if !binding.name.starts_with("$THREAD_") {
            continue;
        }
        let selected = class_claim(claims, class.span);
        let mut outcome = ThreadClassName {
            class_id: format!("class@{}..{}", class.span.start, class.span.end),
            class_span: class.span.into(),
            binding,
            proposed_name: None,
            evaluator: None,
            confidence: Confidence::Unresolved,
            evidence: vec![class.span.into()],
            explanation: Vec::new(),
            applied: false,
            reason: Some("pending family proof: no complete owned native class contract".into()),
            reference_edits: 0,
            shorthand_edits: 0,
        };
        match selected {
            Ok(Some((name, evaluator, confidence, evidence))) => {
                outcome.proposed_name = Some(name.into());
                outcome.evaluator = Some(evaluator.into());
                outcome.confidence = confidence;
                outcome.explanation = evidence;
                outcome.reason = None;
            }
            Err(evidence) => {
                outcome.reason = Some("conflicting canonical class names for the same class span".into());
                outcome.explanation = evidence;
            }
            Ok(None) => {}
        }
        if !class.is_declaration() {
            outcome.reason = Some("class expression inner/outer binding identity is not supported by an owned family proof".into());
        } else if thread_id(&outcome.binding.name).is_none() {
            outcome.reason = Some("unsupported generated class name grammar".into());
        } else if model.binding_writes(symbol) != 0
            || !model.scoping.symbol_redeclarations(symbol).is_empty()
        {
            outcome.reason = Some("class binding is reassigned or redeclared".into());
        }
        // Oxc versions may represent class self and outer names separately.
        // Refuse that shape until a paired-symbol edit contract is available.
        if model.scoping.symbol_ids().any(|other| {
            other != symbol && model.scoping.symbol_span(other) == model.scoping.symbol_span(symbol)
        }) {
            outcome.reason =
                Some("distinct class self binding requires a paired-symbol proof".into());
        }
        outcomes.push(outcome);
    }
    outcomes.sort_by_key(|o| (o.class_span.start, o.binding.identifier.start));
    for outcome in &mut outcomes {
        if outcome.reason.is_some() {
            continue;
        }
        let name = outcome.proposed_name.as_deref().unwrap();
        let start = outcome.binding.identifier.start;
        let mut starts: Vec<_> = plans
            .iter()
            .filter(|p| p.canonical_name == name)
            .map(|p| p.declaration_start)
            .collect();
        starts.push(start);
        let facts = planned_binding_facts_for(program, model.scoping, model.nodes(), &starts, name);
        let Some(candidate) = facts.last().and_then(Option::as_ref) else {
            outcome.reason = Some("class declaration has no scoped rename facts".into());
            continue;
        };
        if facts[..facts.len() - 1]
            .iter()
            .flatten()
            .any(|prior| planned_bindings_collide(model.scoping, model.nodes(), prior, candidate))
        {
            outcome.reason =
                Some("class proposal conflicts with a reserved binding proposal".into());
        } else if rename {
            plans.push(ProgramBindingRename::new(start, name));
        } else {
            outcome.reason = Some("report only; rename disabled".into());
        }
    }
    outcomes
}

// Group complete family proofs at the exact class span, never first-wins.
fn class_claim(claims: &[FinderClaim], span: oxc_span::Span)
    -> Result<Option<(&'static str, &'static str, Confidence, Vec<String>)>, Vec<String>>
{
    let mut grouped = BTreeMap::<&str, (&str, Confidence, BTreeSet<String>)>::new();
    for claim in claims {
        let FinderClaim::ClassBinding { role, class_span, confidence, evidence } = claim else { continue };
        if *class_span != span { continue; }
        let name = match *role {
            "owned_observable_class" => "Observable",
            "owned_value_encoder_class" => "ValueEncoder",
            "owned_ring_buffer_class" => "RingBuffer",
            "owned_lazy_hook_registry_class" => "LazyHookRegistry",
            "owned_pre_register_hook_forwarder_class" => "PreRegisterHookForwarder",
            "owned_batch_scheduler_class" => "BatchScheduler",
            "owned_callback_notifier_reverse_class" => "CallbackNotifier",
            "owned_callback_notifier_forward_class" => "LiveCallbackNotifier",
            "subscription_class" => "Subscription",
            "subscription_observer_class" => "SubscriptionObserver",
            _ => continue,
        };
        let entry = grouped.entry(name).or_insert((*role, *confidence, BTreeSet::new()));
        entry.2.extend(evidence.iter().cloned());
    }
    // A definition description never competes with an accepted042 family
    // claim (or repairs an existing conflict). It is a naming fallback only.
    if grouped.is_empty() {
        for claim in claims {
            let FinderClaim::ClassBinding { role, class_span, confidence, evidence } = claim else { continue };
            if *class_span == span && *role == "owned_reverse_snapshot_notifier_definition" {
                let entry = grouped.entry("ReverseSnapshotNotifier")
                    .or_insert((*role, *confidence, BTreeSet::new()));
                entry.2.extend(evidence.iter().cloned());
            }
        }
    }
    if grouped.len() > 1 {
        return Err(grouped.into_iter().flat_map(|(name, (_, _, evidence))|
            std::iter::once(format!("conflicting complete proof: {name}")).chain(evidence)).collect());
    }
    Ok(grouped.into_iter().next().map(|(name, (role, confidence, evidence))|
        (name, role, confidence, evidence.into_iter().collect())))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_claims_deduplicate_agreement_and_refuse_conflict_in_either_order() {
        let span = oxc_span::Span::new(10, 20);
        let proof = |role, text: &str| FinderClaim::ClassBinding { role, class_span: span,
            confidence: Confidence::High, evidence: vec![text.into()] };
        let a = proof("owned_observable_class", "owned");
        let b = proof("owned_observable_class", "owned");
        assert_eq!(class_claim(&[a.clone(), b], span).unwrap().unwrap().3, vec!["owned"]);
        let encoder = proof("owned_value_encoder_class", "returned encoder");
        assert_eq!(class_claim(&[encoder.clone()], span).unwrap().unwrap().0, "ValueEncoder");
        assert!(class_claim(&[a.clone(), encoder], span).is_err());
        let c = proof("subscription_class", "lifecycle");
        assert!(class_claim(&[a.clone(), c.clone()], span).is_err());
        assert!(class_claim(&[c, a], span).is_err());
        assert!(class_claim(&[proof("weak_finder_class", "bag")], span).unwrap().is_none());
        assert!(class_claim(&[proof("subscription_class", "other span")], oxc_span::Span::new(21, 30)).unwrap().is_none());
    }
    #[test]
    fn definition_name_is_only_a_fallback_and_never_repairs_old_conflicts() {
        let span = oxc_span::Span::new(10, 20);
        let make = |role, text: &str| FinderClaim::ClassBinding {
            role, class_span: span, confidence: Confidence::High, evidence: vec![text.into()],
        };
        let definition = make("owned_reverse_snapshot_notifier_definition", "definition only");
        assert_eq!(class_claim(&[definition.clone()], span).unwrap().unwrap().0, "ReverseSnapshotNotifier");
        for role in ["owned_callback_notifier_reverse_class", "owned_callback_notifier_forward_class", "owned_observable_class"] {
            let old = make(role, "accepted042 evidence");
            let before = class_claim(&[old.clone()], span).unwrap();
            assert_eq!(class_claim(&[old.clone(), definition.clone()], span).unwrap(), before);
            assert_eq!(class_claim(&[definition.clone(), old], span).unwrap(), before);
        }
        assert!(class_claim(&[make("owned_observable_class", "old"), definition, make("owned_value_encoder_class", "old")], span).is_err());
    }

}
