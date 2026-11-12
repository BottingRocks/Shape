use super::*;

pub fn fingerprint(
    result: &PrefixTraceReport,
    parameters: &[String],
    emitting: &Emitting<'_>,
) -> (Value, Option<String>) {
    let complete = result.stop == PrefixStopReason::Completed;
    let positions: BTreeSet<String> = result
        .records
        .iter()
        .map(|record| format!("{}/{}", record.dispatch_pc, record.state_before))
        .collect();
    let arena = &result.trace.arena;
    let kind = |id| arena.nodes.get(id as usize).map(|node| &node.kind);
    let host_key = |object, key| match (kind(object), kind(key)) {
        (Some(TraceNodeKind::Global { name }), Some(TraceNodeKind::String(key)))
            if name == "host" =>
        {
            Some(key.clone())
        }
        _ => None,
    };
    let mut host: BTreeSet<String> = arena
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            TraceNodeKind::PropertyRead { object, key } => {
                host_key(object, key).map(|key| format!("read {key}"))
            }
            _ => None,
        })
        .collect();
    host.extend(
        arena
            .statements
            .iter()
            .filter_map(|statement| match statement.kind {
                TraceStatementKind::PropertyWrite { object, key, .. } => {
                    host_key(object, key).map(|key| format!("write {key}"))
                }
                _ => None,
            }),
    );
    let created: Vec<String> = created_functions(result)
        .into_iter()
        .map(Function::key)
        .collect();
    let (mut real, mut by_test) = (0, 0);
    let mut by_context = Vec::new();
    let mut by_test_decisions = BTreeSet::new();
    let mut forks = BTreeSet::new();
    let describer = result.trace.arena.describer();
    let mut possible = PossibleValues::new(arena);
    let mut branch_list = Vec::new();
    for record in &result.records {
        if let PrefixStepEffect::Handler {
            branch: Some(branch),
            ..
        } = &record.effect
        {
            let mut test = describer.describe(branch.test);
            if test.len() > 300 {
                let end = (0..=300)
                    .rev()
                    .find(|&end| test.is_char_boundary(end))
                    .unwrap_or(0);
                test.truncate(end);
                test.push('…');
            }
            let kept = |taken: bool| if taken { "jump" } else { "fall-through" };
            branch_list.push(match &branch.kind {
                PrefixBranchKind::Real => json!({
                    "at": format!("{}/{}", record.dispatch_pc, record.state_before),
                    "kind": "real",
                    "values": possible.truth(branch.test),
                    "test": test,
                }),
                PrefixBranchKind::FakeByTest { taken } => json!({
                    "at": format!("{}/{}", record.dispatch_pc, record.state_before),
                    "kind": "fake by test",
                    "kept": kept(*taken),
                    "audit": result.fake_audits.get(&format!("{}/{}", record.dispatch_pc, record.state_before)),
                    "defs": possible.definitions(branch.test, &describer),
                    "values": possible.truth(branch.test),
                    "test": test,
                }),
                PrefixBranchKind::FakeByContext { taken, reason } => json!({
                    "at": format!("{}/{}", record.dispatch_pc, record.state_before),
                    "kind": "fake by context",
                    "kept": kept(*taken),
                    "values": possible.truth(branch.test),
                    "reason": reason,
                    "test": test,
                }),
            });
            match &branch.kind {
                PrefixBranchKind::Real => {
                    real += 1;
                    forks.insert(format!("{}/{}", record.dispatch_pc, record.state_before));
                }
                PrefixBranchKind::FakeByTest { taken } => {
                    by_test += 1;
                    by_test_decisions.insert(format!(
                        "{}/{} kept {}",
                        record.dispatch_pc,
                        record.state_before,
                        if *taken { "jump" } else { "fall-through" }
                    ));
                }
                PrefixBranchKind::FakeByContext { taken, reason } => by_context.push(format!(
                    "{}/{} kept {}: {reason}",
                    record.dispatch_pc,
                    record.state_before,
                    if *taken { "jump" } else { "fall-through" }
                )),
            }
        }
    }
    let (undeclared, emission, code) = if complete {
        check_emission(result, parameters, &created_functions(result), emitting)
    } else {
        (Vec::new(), Vec::new(), None)
    };
    let print = json!({
        "complete": complete,
        "stop": result.stop.to_string(),
        "positions": positions,
        "creates": created,
        "host": host,
        "branches": { "real": real, "fake_by_test": by_test },
        "forks": forks,
        "fake_by_context": by_context,
        "fake_by_test": by_test_decisions,
        "branch_list": branch_list,
        "undeclared": undeclared,
        "emission": emission,
        "self_check": result.self_check,
        "steps": result.records.len(),
        "handlers": result.trace.handlers_traced,
        "retraces": result.retraces.total(),
        "cell_restarts": result.retraces.cells,
        "wasted": result.wasted_steps,
    });
    (print, code)
}

pub fn strings(value: &Value) -> BTreeSet<String> {
    value.as_array().into_iter().flatten().map(text).collect()
}

/// Differences between a walk and the same walk without outlining.
pub fn compare_inline(outlined: &Value, inline: &Value) -> Vec<String> {
    let mut differences = Vec::new();
    if outlined["complete"] != inline["complete"] {
        differences.push(format!(
            "without outlining the walk {} ({})",
            if inline["complete"] == json!(true) {
                "completes"
            } else {
                "stops"
            },
            text(&inline["stop"])
        ));
        return differences;
    }
    // A walk that stopped covered what it reached before the stop, which
    // depends on the order it walked in: compare how it stopped instead.
    if outlined["complete"] != json!(true) {
        if stop_kind(&text(&outlined["stop"])) != stop_kind(&text(&inline["stop"])) {
            differences.push(format!(
                "stops with outlining at {}, without at {}",
                text(&outlined["stop"]),
                text(&inline["stop"])
            ));
        }
        return differences;
    }
    for field in ["positions", "creates", "host"] {
        let (a, b) = (strings(&outlined[field]), strings(&inline[field]));
        let missing: Vec<_> = b.difference(&a).take(5).cloned().collect();
        let extra: Vec<_> = a.difference(&b).take(5).cloned().collect();
        if !missing.is_empty() || !extra.is_empty() {
            differences.push(format!(
                "{field}: without outlining also {missing:?}, with outlining also {extra:?}"
            ));
        }
    }
    // Where the walk forks: a jump that forks without outlining but never
    // with it lost one of its directions; the other way round, outlining
    // traced a direction the test rules out. (How many times a jump is met,
    // forked or decided, depends on where paths join, which may differ.)
    // A test decided from an earlier test of the same value on the path is
    // decided by that path's history, which joins shape: no direction is
    // lost where one run decides it so.
    let repeated = |print: &Value| -> BTreeSet<String> {
        strings(&print["fake_by_context"])
            .into_iter()
            .filter(|decision| decision.contains("tested the same value"))
            .filter_map(|decision| decision.split(' ').next().map(str::to_string))
            .collect()
    };
    let (with, without) = (strings(&outlined["forks"]), strings(&inline["forks"]));
    let (with_repeated, without_repeated) = (repeated(outlined), repeated(inline));
    let lost: Vec<_> = without
        .difference(&with)
        .filter(|at| !with_repeated.contains(*at))
        .take(5)
        .cloned()
        .collect();
    let extra: Vec<_> = with
        .difference(&without)
        .filter(|at| !without_repeated.contains(*at))
        .take(5)
        .cloned()
        .collect();
    if !lost.is_empty() {
        differences.push(format!("forks only without outlining at {lost:?}"));
    }
    if !extra.is_empty() {
        differences.push(format!("forks only with outlining at {extra:?}"));
    }
    // Sides dropped for reaching a bad state must be the same: where and
    // which side, whichever value of it showed the bad state.
    let bad = |print: &Value| -> BTreeSet<String> {
        strings(&print["fake_by_context"])
            .into_iter()
            .filter(|decision| !decision.contains("tested the same value"))
            .map(|decision| decision.split(": ").next().unwrap_or_default().to_string())
            .collect()
    };
    // A walk that knows more values decides by test what the other only
    // decides by the bad state its side reaches: the same branch, the same
    // side kept, is no difference.
    let by_test = |print: &Value| strings(&print["fake_by_test"]);
    let (with, without): (BTreeSet<String>, BTreeSet<String>) = (
        bad(outlined)
            .into_iter()
            .filter(|decision| !by_test(inline).contains(decision))
            .collect(),
        bad(inline)
            .into_iter()
            .filter(|decision| !by_test(outlined).contains(decision))
            .collect(),
    );
    if with != without {
        differences.push(format!(
            "branches decided by context: {:?} with outlining, {:?} without",
            with.difference(&without).take(3).collect::<Vec<_>>(),
            without.difference(&with).take(3).collect::<Vec<_>>()
        ));
    }
    differences
}

/// Regressions and other changes from `baseline` to `current`.
pub fn compare(baseline: &Value, current: &Value) -> (Vec<String>, Vec<String>) {
    let (mut regressions, mut changes) = (Vec::new(), Vec::new());
    let empty = serde_json::Map::new();
    for (seed, old_functions) in baseline.as_object().unwrap_or(&empty) {
        let new_functions = current[seed].as_object().unwrap_or(&empty);
        if current.get(seed).is_none() {
            changes.push(format!("{seed}: not walked"));
            continue;
        }
        for (key, old) in old_functions.as_object().unwrap_or(&empty) {
            let Some(new) = new_functions.get(key) else {
                regressions.push(format!("{seed} {key}: no longer reached"));
                continue;
            };
            let at = format!("{seed} {key}");
            match (
                old["complete"] == json!(true),
                new["complete"] == json!(true),
            ) {
                (true, false) => regressions.push(format!(
                    "{at}: completed, now stops: {}",
                    text(&new["stop"])
                )),
                (false, true) => changes.push(format!("{at}: now completes")),
                (false, false) if old["stop"] != new["stop"] => {
                    changes.push(format!("{at}: stops differently: {}", text(&new["stop"])))
                }
                _ => {}
            }
            for field in ["positions", "creates", "host"] {
                let (a, b) = (strings(&old[field]), strings(&new[field]));
                let lost: Vec<_> = a.difference(&b).cloned().collect();
                let gained: Vec<_> = b.difference(&a).cloned().collect();
                if !lost.is_empty() && new["complete"] == json!(true) {
                    regressions.push(format!(
                        "{at}: lost {} {field}: {:?}",
                        lost.len(),
                        &lost[..lost.len().min(5)]
                    ));
                }
                if !gained.is_empty() {
                    changes.push(format!(
                        "{at}: gained {} {field}: {:?}",
                        gained.len(),
                        &gained[..gained.len().min(5)]
                    ));
                }
            }
            // A decision is its position and the side kept; the reason names
            // generated variables, which other changes renumber.
            let decided = |print: &Value| -> BTreeSet<String> {
                strings(&print["fake_by_context"])
                    .into_iter()
                    .map(|decision| decision.split(':').next().unwrap_or("").to_string())
                    .collect()
            };
            let new_context: Vec<_> = decided(new).difference(&decided(old)).cloned().collect();
            for decision in new_context {
                regressions.push(format!("{at}: new branch decided by context: {decision}"));
            }
            if old["branches"] != new["branches"] {
                changes.push(format!(
                    "{at}: branches {} -> {}",
                    old["branches"], new["branches"]
                ));
            }
        }
        for key in new_functions.keys() {
            if !old_functions
                .as_object()
                .is_some_and(|old| old.contains_key(key))
            {
                changes.push(format!("{seed} {key}: newly reached"));
            }
        }
    }
    (regressions, changes)
}
