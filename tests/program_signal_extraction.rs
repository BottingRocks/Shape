use oxc_span::Span;
use shape::signals::extract::contracts::GrammarNode;
use shape::signals::extract::layout::{
    extract_signal_layouts, CollectorRequest, EmissionSite, HelperBindings, ObserverWiring,
    SignalLayoutLimits, SignalLayoutReport, SignalLayoutRequest,
};

fn extract(body: &str) -> SignalLayoutReport {
    extract_at(EmissionSite::ObserverNext, body)
}

fn extract_at(site: EmissionSite, body: &str) -> SignalLayoutReport {
    let source = format!("function write(m,v,b,p){{return p;}} function prng(s){{return ()=>s;}} function collect(obs){{let seed=Math.random()*4294967296|0;let m=prng(seed^7);let p=3;{body}}}");
    let writer = source.find("write").unwrap() as u32;
    let prng = source.find("prng").unwrap() as u32;
    let collector = source.find("function collect").unwrap() as u32;
    extract_signal_layouts(&SignalLayoutRequest {
        source: &source,
        helpers: HelperBindings {
            number_writer: vec![Span::new(writer, writer + 5)],
            prng_factory: vec![Span::new(prng, prng + 4)],
            ..Default::default()
        },
        collectors: vec![CollectorRequest {
            function: Span::new(collector, source.len() as u32),
            observers: ObserverWiring::Direct { signal: 0 },
            site,
            forwarded_parameter: None,
        }],
        limits: SignalLayoutLimits::default(),
    })
    .unwrap()
}
const PREFIX: &str = "[seed>>>24,seed>>16&255,seed>>8&255,seed&255]";
fn numbers(grammar: &GrammarNode) -> Vec<String> {
    let mut out = vec![];
    grammar.walk(&mut |node| {
        if let GrammarNode::Number(write) = node {
            out.push(write.value.clone())
        }
    });
    out
}
#[test]
fn worker_reply_literal_names_a_payload_bound_before_it() {
    // `postMessage({a: t, b: p})` with `p = [[], buffer]` bound earlier
    // delivers that buffer like `{a: t, b: [[], buffer]}` or `r.b = p`.
    for post in ["let r=[[],b];postMessage({a:1,b:r});", "postMessage({a:1,b:[[],b]});", "let r=[[],b];let o={a:1};o.b=r;postMessage(o);"] {
        let report = extract_at(EmissionSite::WorkerReply, &format!("let b={PREFIX};p=write(m,111,b,p);{post}"));
        let emissions = &report.signals[0].emissions;
        assert_eq!(emissions.len(), 1, "{post}");
        assert_eq!(numbers(&emissions[0].grammar), ["111"], "{post}");
    }
}
#[test]
fn fresh_array_excludes_previous_buffer_lifetime() {
    let report=extract(&format!("let b={PREFIX};p=write(m,111,b,p);obs.next([[],b]);b={PREFIX};p=write(m,222,b,p);obs.next([[],b]);"));
    let emissions = &report.signals[0].emissions;
    assert_eq!(emissions.len(), 2);
    assert_eq!(numbers(&emissions[0].grammar), ["111"]);
    assert_eq!(numbers(&emissions[1].grammar), ["222"]);
}
#[test]
fn do_while_condition_retains_short_circuit_write() {
    let report = extract(&format!(
        "let b={PREFIX};let i=2;do{{--i;}}while(i>=0&&(p=write(m,i,b,p),true));obs.next([[],b]);"
    ));
    let grammar = &report.signals[0].emissions[0].grammar;
    assert_eq!(numbers(grammar), ["i"]);
    let mut repeats = 0;
    let mut choices = 0;
    grammar.walk(&mut |node| match node {
        GrammarNode::Repeat { .. } => repeats += 1,
        GrammarNode::Choice { .. } => choices += 1,
        _ => {}
    });
    assert_eq!(repeats, 1);
    // The skip route of the test's `&&` is the loop exit: every repeated
    // iteration writes, like the guarded `for (;;) { if (t) { …; continue; } … }`.
    assert_eq!(choices, 0);
}
#[test]
fn branch_local_allocations_drop_prior_buffer_on_both_arms() {
    let report=extract(&format!("let b={PREFIX};p=write(m,999,b,p);if(seed){{b={PREFIX};p=write(m,111,b,p);}}else{{b={PREFIX};p=write(m,222,b,p);}}obs.next([[],b]);"));
    let grammar = &report.signals[0].emissions[0].grammar;
    assert_eq!(numbers(grammar), ["111", "222"]);
    let mut prefixes = 0;
    grammar.walk(&mut |node| {
        if matches!(node, GrammarNode::RawPrefix(_)) {
            prefixes += 1
        }
    });
    assert_eq!(prefixes, 2);
}
#[test]
fn single_branch_reset_keeps_prior_buffer_only_on_other_arm() {
    let report=extract(&format!("let b={PREFIX};p=write(m,999,b,p);if(seed){{b={PREFIX};p=write(m,111,b,p);}}obs.next([[],b]);"));
    let grammar = &report.signals[0].emissions[0].grammar;
    assert_eq!(numbers(grammar), ["111", "999"]);
}
#[test]
fn for_header_writes_follow_javascript_evaluation_order() {
    let report=extract(&format!("let b={PREFIX};for(p=write(m,10,b,p);(p=write(m,20,b,p),seed);p=write(m,40,b,p)){{p=write(m,30,b,p);}}obs.next([[],b]);"));
    assert_eq!(
        numbers(&report.signals[0].emissions[0].grammar),
        ["10", "20", "30", "40", "20"]
    );
}
#[test]
fn emission_keeps_its_own_mask_before_a_later_reassignment() {
    let report=extract(&format!("let b={PREFIX};p=write(m,111,b,p);obs.next([[],b]);m=prng(seed^123);b={PREFIX};p=write(m,222,b,p);obs.next([[],b]);"));
    for (emission, salt) in report.signals[0]
        .emissions
        .iter()
        .zip(["seed^7", "seed^123"])
    {
        let mut masks = vec![];
        emission.grammar.walk(&mut |node| {
            if let GrammarNode::Number(write) = node {
                masks.push(write.mask_source.clone().unwrap().replace(' ', ""))
            }
        });
        assert_eq!(masks, [salt]);
    }
}
#[test]
fn nested_writer_calls_keep_inner_before_outer() {
    let report = extract(&format!(
        "let b={PREFIX};p=write(m,222,b,write(m,111,b,p));obs.next([[],b]);"
    ));
    assert_eq!(
        numbers(&report.signals[0].emissions[0].grammar),
        ["111", "222"]
    );
}
#[test]
fn legacy_wire_does_not_resurrect_old_buffer_fields() {
    use shape::signals::extract::layout::WireOp;
    fn values(wire: &WireOp, out: &mut Vec<String>) {
        match wire {
            WireOp::Sequence(items) | WireOp::Choice(items) => {
                for item in items {
                    values(item, out)
                }
            }
            WireOp::Repeat { body, .. } => values(body, out),
            WireOp::Leaf(leaf) => {
                if let Some(value) = &leaf.value {
                    out.push(value.clone())
                }
            }
            _ => {}
        }
    }
    let report = extract(&format!(
        "let b={PREFIX};p=write(m,999,b,p);b={PREFIX};p=write(m,111,b,p);obs.next([[],b]);"
    ));
    let mut writes = vec![];
    values(&report.signals[0].emissions[0].wire, &mut writes);
    assert!(!writes.iter().any(|v| v == "999"));
    assert!(writes.iter().any(|v| v == "111"));
}

#[test]
fn indexed_seed_header_span_includes_the_first_store() {
    let report = extract("let b=[];b[0]=seed>>>24;b[1]=seed>>16&255;b[2]=seed>>8&255;b[3]=seed&255;p=write(m,111,b,p);obs.next([[],b]);");
    let mut prefixes = vec![];
    report.signals[0].emissions[0].grammar.walk(&mut |node| {
        if let GrammarNode::RawPrefix(prefix) = node {
            prefixes.push(prefix);
        }
    });
    assert_eq!(prefixes.len(), 1);
    let prefix = prefixes[0];
    let source = "function write(m,v,b,p){return p;} function prng(s){return ()=>s;} function collect(obs){let seed=Math.random()*4294967296|0;let m=prng(seed^7);let p=3;let b=[];b[0]=seed>>>24;b[1]=seed>>16&255;b[2]=seed>>8&255;b[3]=seed&255;p=write(m,111,b,p);obs.next([[],b]);}";
    assert!(source[prefix.span.start as usize..prefix.span.end as usize].starts_with("b[0]="));
}

#[test]
fn buffer_alias_in_only_one_branch_keeps_both_payloads() {
    let report = extract(&format!("let b;let c;if(seed){{b={PREFIX};p=write(m,111,b,p);}}else{{c={PREFIX};b=c;p=write(m,222,b,p);}}obs.next([[],b]);"));
    let grammar = &report.signals[0].emissions[0].grammar;
    assert_eq!(numbers(grammar), ["111", "222"]);
    assert_eq!(report.accounting.len(), 2);
}

#[test]
fn alias_branch_can_use_a_buffer_allocated_before_the_choice() {
    let report = extract(&format!("let c={PREFIX};p=write(m,22,c,p);let b;if(seed){{b={PREFIX};p=write(m,111,b,p);}}else{{b=c;p=write(m,222,b,p);}}obs.next([[],b]);"));
    assert_eq!(
        numbers(&report.signals[0].emissions[0].grammar),
        ["111", "22", "222"]
    );
}

#[test]
fn returning_loop_iteration_is_not_replayed_as_an_earlier_iteration() {
    let report = extract(&format!("let b={PREFIX};while(seed){{if(seed>1){{p=write(m,111,b,p);continue;}}p=write(m,222,b,p);obs.next([[],b]);return;}}p=write(m,333,b,p);obs.next([[],b]);"));
    assert_eq!(
        numbers(&report.signals[0].emissions[0].grammar),
        ["111", "222"]
    );
    assert_eq!(
        numbers(&report.signals[0].emissions[1].grammar),
        ["111", "333"]
    );
}

#[test]
fn writes_on_an_already_returned_path_do_not_reach_a_later_emission() {
    let report = extract(&format!("let b={PREFIX};if(seed){{p=write(m,111,b,p);obs.next([[],b]);return;}}p=write(m,222,b,p);obs.next([[],b]);"));
    assert_eq!(numbers(&report.signals[0].emissions[0].grammar), ["111"]);
    assert_eq!(numbers(&report.signals[0].emissions[1].grammar), ["222"]);
}

#[test]
fn normalized_loop_break_does_not_terminate_the_collector() {
    let report = extract(&format!("let b={PREFIX};let i=2;for(;;){{--i;if(!(i>=0))break;p=write(m,i,b,p);}}p=write(m,333,b,p);obs.next([[],b]);"));
    assert_eq!(
        numbers(&report.signals[0].emissions[0].grammar),
        ["i", "333"]
    );
}

#[test]
fn ordinary_branches_keep_a_shared_continuation() {
    let mut body = format!("let b={PREFIX};if(seed>99){{return;}}");
    for value in 0..12 {
        body.push_str(&format!(
            "if(seed){{p=write(m,{value},b,p);}}else{{p=write(m,999,b,p);}}"
        ));
    }
    body.push_str("obs.next([[],b]);");
    let report = extract(&body);
    assert_eq!(numbers(&report.signals[0].emissions[0].grammar).len(), 24);
}

#[test]
fn inherited_prefix_keeps_the_allocation_span() {
    let report = extract(&format!("let b={PREFIX};let callback=()=>{{p=write(m,111,b,p);obs.next([[],b]);}};callback();"));
    assert!(!report.signals[0].emissions.is_empty());
    for emission in &report.signals[0].emissions {
        emission.grammar.walk(&mut |node| {
            if let GrammarNode::RawPrefix(prefix) = node {
                assert_eq!((prefix.span.end - prefix.span.start) as usize, PREFIX.len());
            }
        });
    }
}

#[test]
fn reused_payload_list_drops_the_previous_second_element() {
    let report = extract(&format!("let b={PREFIX};p=write(m,111,b,p);let payload=[[],b];obs.next(payload);let c={PREFIX};p=write(m,222,c,p);payload=[[]];payload[1]=c;obs.next(payload);"));
    assert_eq!(report.signals[0].emissions.len(), 2);
    assert_eq!(numbers(&report.signals[0].emissions[0].grammar), ["111"]);
    assert_eq!(numbers(&report.signals[0].emissions[1].grammar), ["222"]);
}

#[test]
fn throwing_validation_is_not_the_lowered_loop_exit_guard() {
    let report = extract(&format!("let b={PREFIX};let i=2;for(;;){{let j=i-1;if(seed<0)throw new Error();if(j>=0){{p=write(m,j,b,p);i=j;continue;}}else{{p=write(m,999,b,p);break;}}}}obs.next([[],b]);"));
    assert_eq!(numbers(&report.signals[0].emissions[0].grammar), ["j", "999"]);
}

#[test]
fn outer_counter_advanced_on_inner_loop_exit_has_its_own_direction() {
    let report = extract(&format!("let b={PREFIX};let outer=0;while(outer<2){{let inner=2;for(;;){{let next=inner-1;if(seed<0)throw new Error();if(next>=0){{p=write(m,next,b,p);inner=next;continue;}}else{{++outer;break;}}}}}}obs.next([[],b]);"));
    let mut directions = vec![];
    report.signals[0].emissions[0].grammar.walk(&mut |node| {
        if let GrammarNode::Repeat { direction, .. } = node { directions.push(*direction); }
    });
    assert_eq!(directions, [shape::signals::extract::contracts::LoopDirection::Ascending,
                            shape::signals::extract::contracts::LoopDirection::Descending]);
}

#[test]
fn payload_holder_joins_distinct_branch_buffers() {
    let source = format!("let payload;if(seed){{let left={PREFIX};write(m,111,left,3);payload=[[],left];}}else{{let right={PREFIX};write(m,222,right,3);payload=[[],right];}}obs.next(payload);");
    let report = extract(&source);
    let grammar = &report.signals[0].emissions[0].grammar;
    assert_eq!(numbers(grammar), ["111", "222"]);
    let mut prefixes = 0;
    grammar.walk(&mut |node| if matches!(node, GrammarNode::RawPrefix(_)) {prefixes += 1});
    assert_eq!(prefixes, 2);
}

#[test]
fn hoisted_callback_captures_single_write_observer_handle() {
    let report = extract(&format!("let handle;function callback(){{let b={PREFIX};write(m,111,b,3);handle.next([[],b]);}}handle=obs;register(callback);"));
    assert_eq!(report.signals.len(), 1);
    assert_eq!(numbers(&report.signals[0].emissions[0].grammar), ["111"]);
}

#[test]
fn host_listener_cleanup_is_not_a_string_writer() {
    let report=extract(&format!("let listener=()=>{{}};let b={PREFIX};function cleanup(){{host.removeEventListener('move',listener,{{capture:true}});}}p=write(m,111,b,p);obs.next([[],b]);"));
    assert_eq!(numbers(&report.signals[0].emissions[0].grammar),["111"]);
    assert!(report.unresolved.is_empty(),"{:?}",report.unresolved);
}

#[test]
fn conditional_writer_buffer_stays_on_its_selected_arm() {
    let report=extract(&format!("let a={PREFIX};let b={PREFIX};p=write(m,10,flag?a:b,p);obs.next([[],a]);obs.next([[],b]);"));
    assert_eq!(report.signals[0].emissions.len(),2);
    for emission in &report.signals[0].emissions {
        assert_eq!(numbers(&emission.grammar),["10"]);
        let mut choices=0;emission.grammar.walk(&mut|node|if matches!(node,GrammarNode::Choice{..}){choices+=1;});
        assert!(choices>0);
    }
}

#[test]
fn reused_payload_register_takes_the_stream_of_its_own_writes() {
    // `b` first carries another emission's buffer (stream salt 7), then is
    // reused as the holder of a per-arm buffer whose writes draw from salt 55.
    let report = extract(&format!("let b={PREFIX};p=write(m,111,b,p);obs.next([[],b]);if(seed){{let left={PREFIX};let k=prng(seed^55);write(k,1,left,3);b=[[],left];}}else{{let right={PREFIX};let j=prng(seed^55);write(j,2,right,3);b=[[],right];}}obs.next(b);"));
    let emissions = &report.signals[0].emissions;
    assert_eq!(emissions.len(), 2);
    let salt = |index: usize| emissions[index].mask.as_ref().and_then(|mask| mask.xor_source.clone());
    assert_eq!(salt(0).as_deref(), Some("7"));
    assert_eq!(salt(1).as_deref(), Some("55"));
}

fn write_masks(report: &SignalLayoutReport) -> Vec<(String, Option<String>)> {
    let mut out = vec![];
    report.signals[0].emissions[0].grammar.walk(&mut |node| {
        if let GrammarNode::Number(write) = node {
            out.push((format!("{:?}", write.mask), write.mask_source.clone()));
        }
    });
    out
}

#[test]
fn stream_copied_on_every_arm_resolves_after_the_join() {
    let report = extract(&format!("let b={PREFIX};let s;if(seed){{let k=prng(seed^55);s=k;}}else{{let j=prng(seed^55);s=j;}}p=write(s,1,b,p);obs.next([[],b]);"));
    let masks = write_masks(&report);
    assert_eq!(masks.len(), 1);
    assert_eq!(masks[0].0, "Stream");
    assert!(masks[0].1.as_deref().unwrap_or_default().contains("55"), "{masks:?}");
}

#[test]
fn stream_copies_with_different_salts_stay_unresolved() {
    let report = extract(&format!("let b={PREFIX};let s;if(seed){{let k=prng(seed^55);s=k;}}else{{let j=prng(seed^66);s=j;}}p=write(s,1,b,p);obs.next([[],b]);"));
    let masks = write_masks(&report);
    assert_eq!(masks.len(), 1);
    assert_eq!(masks[0].0, "Unresolved", "{masks:?}");
}
