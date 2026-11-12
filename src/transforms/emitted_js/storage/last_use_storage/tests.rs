#[test]
fn finished_header_storage_crosses_ten_new_parent_activation_forms() {
    for outer in [
        "for(let outer=0;outer<2;outer++){INNER}",
        "while(flag--){INNER}",
        "do{INNER}while(flag--);",
        "for(const outer of values){INNER}",
        "for(const outer in values){INNER}",
        "for(let outer=0;outer<2;outer++){if(flag){INNER}}",
        "for(let outer=0;outer<2;outer++){if(flag)continue;INNER}",
        "for(let outer=0;outer<2;outer++){INNER if(flag)break;}",
        "try{for(let outer=0;outer<2;outer++){INNER}}catch(e){report(e);}",
        "for(let outer=0;outer<2;outer++){for(let middle=0;middle<2;middle++){INNER}}",
    ] {
        let nested = outer.replace(
            "INNER",
            "for(let $SSA_next=0;$SSA_next<3;$SSA_next++){use($SSA_next);}",
        );
        let source =
            format!("function f(flag){{let $CARRY_old=read();use($CARRY_old);{nested}}}");
        let (code, count) =
            super::recycle_ended_cells(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{source}\n{code}");
        assert!(!code.contains("$SSA_next"), "{code}");
    }
}

#[test]
fn header_storage_keeps_parent_observations_and_per_iteration_identities() {
    for outer in [
        "for(let outer=0;outer<2;outer++){for(let $SSA_next=$CARRY_old;$SSA_next>0;$SSA_next--)use($SSA_next);}",
        "for(let outer=0;outer<2;outer++){use($CARRY_old);INNER}",
        "for(let outer=0;outer<$CARRY_old;outer++){INNER}",
        "while($CARRY_old--){INNER}",
        "do{INNER}while($CARRY_old--);",
        "for(const outer of values){INNER use($CARRY_old);}",
        "for(const outer in values){INNER use($CARRY_old);}",
        "for(let outer=0;outer<2;outer++){try{INNER}finally{use($CARRY_old);}}",
        "for(let outer=0;outer<2;outer++){for(let $SSA_next=0;$SSA_next<2;$SSA_next++)save(()=> $SSA_next);}",
        "for(let outer=0;outer<2;outer++){for(let $SSA_next=0;$SSA_next<2;$SSA_next++){let $CARRY_old=3;use($CARRY_old,$SSA_next);}}",
    ] {
        let nested=outer.replace("INNER","for(let $SSA_next=0;$SSA_next<3;$SSA_next++){use($SSA_next);}");
        let source=format!("function f(){{let $CARRY_old=read();use($CARRY_old);{nested}}}");
        let (code,count)=super::recycle_ended_cells(&source,&Default::default(),false).unwrap();
        assert_eq!(count,0,"{source}\n{code}");
    }
}

#[test]
fn for_headers_borrow_ended_storage_in_ten_control_flow_forms() {
    for loop_body in [
        "for(let $SSA_next=0;$SSA_next<3;$SSA_next++){use($SSA_next);}",
        "for(let $SSA_next=3;$SSA_next>0;$SSA_next--){use($SSA_next);}",
        "for(let $SSA_next=0;$SSA_next<6;$SSA_next+=2){use($SSA_next);}",
        "for(let $SSA_next=start();test($SSA_next);$SSA_next=step($SSA_next)){use($SSA_next);}",
        "for(let $SSA_next=0;$SSA_next<array.length;$SSA_next++){use(array[$SSA_next]);}",
        "for(let $SSA_next=0;$SSA_next<3;$SSA_next++){if(flag)continue;use($SSA_next);}",
        "for(let $SSA_next=0;$SSA_next<3;$SSA_next++){if(flag)break;use($SSA_next);}",
        "for(let $SSA_next=0;$SSA_next<3;$SSA_next++){if(flag)use($SSA_next);else report($SSA_next);}",
        "for(let $SSA_next=0;$SSA_next<3;$SSA_next++){for(let j=0;j<2;j++){use($SSA_next,j);}}",
        "for(let $SSA_next=0;$SSA_next<3;$SSA_next++){try{use($SSA_next);}catch(e){report($SSA_next);}}",
    ] {
        let source=format!("function f(flag){{let $CARRY_old=read();use($CARRY_old);{loop_body}}}");
        let (code,count)=super::recycle_ended_cells(&source,&Default::default(),false).unwrap();
        assert_eq!(count,1,"{source}\n{code}");
        assert!(!code.contains("$SSA_next"),"{code}");
        let allocator=oxc_allocator::Allocator::default();
        crate::analysis::parse_program(&allocator,&code).unwrap();
    }
    for source in [
        "function f(){for(let outer=0;outer<2;outer++){let $CARRY_old=read();use($CARRY_old);for(let $SSA_next=0;$SSA_next<2;$SSA_next++)use($SSA_next);}}",
        "function f(){let $CARRY_old=read();for(let $SSA_next=$CARRY_old;$SSA_next>0;$SSA_next--)use($SSA_next);}",
        "function f(){const $CARRY_old=read();use($CARRY_old);for(const $SSA_next=read();test($SSA_next);)use($SSA_next);}",
    ] {
        let (code,count)=super::recycle_ended_cells(source,&Default::default(),false).unwrap();
        assert_eq!(count,1,"{source}\n{code}");
    }
}

#[test]
fn for_header_storage_retains_activation_scope_captures_and_store_errors() {
    for source in [
        "function f(){let $CARRY_old=read();use($CARRY_old);for(let $SSA_next=0;$SSA_next<2;$SSA_next++)save(()=> $SSA_next);}",
        "function f(){let $CARRY_old=read();for(let outer=0;outer<2;outer++){for(let $SSA_next=$CARRY_old;$SSA_next>0;$SSA_next--)use($SSA_next);}}",
        "function f(){let $CARRY_old=read();use($CARRY_old);for(let $SSA_next=0;$SSA_next<2;$SSA_next++)use($SSA_next);return $CARRY_old;}",
        "function f(){let $CARRY_old=read();for(let $SSA_next=0;$SSA_next<2;$SSA_next++)use($SSA_next,$CARRY_old);}",
        "function f(){const $CARRY_old=read();$CARRY_old=3;for(let $SSA_next=0;$SSA_next<2;$SSA_next++)use($SSA_next);}",
        "function f(){let $CARRY_old=read();use($CARRY_old);for(const $SSA_next=0;$SSA_next<2;$SSA_next++)use($SSA_next);}",
        "function f(){let $CARRY_old=read();use($CARRY_old);for(let $SSA_next=$SSA_next;test($SSA_next);)use($SSA_next);}",
        "function f(){let $CARRY_old=read();use($CARRY_old);for(let $SSA_next=0;$SSA_next<2;$SSA_next++){let $CARRY_old=3;use($CARRY_old,$SSA_next);}}",
        "function f(){let $CARRY_old=read();use($CARRY_old);for(let $SSA_next=0,j=0;$SSA_next<2;$SSA_next++)use($SSA_next,j);}",
        "function f(){let $CARRY_old=read();use($CARRY_old);try{for(let $SSA_next=0;$SSA_next<2;$SSA_next++)use($SSA_next);}finally{use($CARRY_old);}}",
        "function f(){let $CARRY_old=read();use($CARRY_old);for(let $SSA_next=function(){};test($SSA_next);)use($SSA_next.name);}",
        "function f(){let $CARRY_old=read();use($CARRY_old);for(let $SSA_next=0;$SSA_next<2;$SSA_next++){function $CARRY_old(){}use($SSA_next);}}",
        "function f(){let $CARRY_old=read();use($CARRY_old);for(let $SSA_next /*slot*/=0;$SSA_next<2;$SSA_next++)use($SSA_next);}",
    ] {
        let (code,count)=super::recycle_ended_cells(source,&Default::default(),false).unwrap();
        assert_eq!(count,0,"{source}\n{code}");
    }
}

#[test]
fn continuation_conditions_keep_copybacks_on_their_actual_truth_edges() {
    for condition in [
        "$SSA_next>0&&($CARRY_old=$SSA_next,true)",
        "!($SSA_next<=0||($CARRY_old=$SSA_next,false))",
        "$SSA_next>0?($CARRY_old=$SSA_next,true):false",
        "$SSA_next<=0?false:($CARRY_old=$SSA_next,true)",
        "(effect(),$SSA_next>0&&($CARRY_old=$SSA_next,true))",
        "$SSA_next>0&&($CARRY_old=$SSA_next,flag||!flag)",
        "!!($SSA_next>0&&($CARRY_old=$SSA_next,true))",
        "$SSA_next>0&&(flag?($CARRY_old=$SSA_next,true):($CARRY_old=$SSA_next,true))",
        "($SSA_next>0||false)&&($CARRY_old=$SSA_next,true)",
        "false||($SSA_next>0&&($CARRY_old=$SSA_next,true))",
    ] {
        let source=format!("function f(flag){{let $CARRY_old=read();let $SSA_next;do{{$SSA_next=$CARRY_old-1;use($SSA_next);}}while({condition});return $SSA_next;}}");
        let (code, count) =
            super::recycle_last_inputs(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{source}\n{code}");
    }
    for loop_body in [
        "while(($SSA_next=$CARRY_old-1,$SSA_next>0)&&($CARRY_old=$SSA_next,true)){use($SSA_next,$CARRY_old);}",
        "for(;($SSA_next=$CARRY_old-1,$SSA_next>0)&&($CARRY_old=$SSA_next,true);effect()){use($SSA_next,$CARRY_old);}",
    ] {
        let source=format!("function f(){{let $CARRY_old=read();let $SSA_next;{loop_body}return $SSA_next;}}");
        let (code,count)=super::recycle_last_inputs(&source,&Default::default(),false).unwrap();
        assert_eq!(count,1,"{source}\n{code}");
    }
}

#[test]
fn truth_partitioned_branches_prove_only_the_edge_that_restored_equality() {
    for tail in [
        "if(flag&&($CARRY_old=$SSA_next,true))use($SSA_next,$CARRY_old);",
        "if(flag||($CARRY_old=$SSA_next,false)){}else use($SSA_next,$CARRY_old);",
        "if(!(flag||($CARRY_old=$SSA_next,false)))use($SSA_next,$CARRY_old);",
        "if(flag?($CARRY_old=$SSA_next,true):false)use($SSA_next,$CARRY_old);",
        "if(flag?true:($CARRY_old=$SSA_next,false)){}else use($SSA_next,$CARRY_old);",
        "(flag&&($CARRY_old=$SSA_next,true))?use($SSA_next,$CARRY_old):effect();",
        "(flag||($CARRY_old=$SSA_next,false))?effect():use($SSA_next,$CARRY_old);",
        "(flag&&($CARRY_old=$SSA_next,true))&&use($SSA_next,$CARRY_old);",
        "(flag||($CARRY_old=$SSA_next,false))||use($SSA_next,$CARRY_old);",
        "if((effect(),flag&&($CARRY_old=$SSA_next,true)))use($SSA_next,$CARRY_old);",
    ] {
        let source=format!("function f(flag){{let $CARRY_old=read();let $SSA_next;$SSA_next=$CARRY_old-1;{tail}return $SSA_next;}}");
        let (code, count) =
            super::recycle_last_inputs(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{source}\n{code}");
    }
}

#[test]
fn continuation_conditions_keep_short_circuit_false_exits_and_stale_backedges() {
    for tail in [
        "do{$SSA_next=$CARRY_old-1;}while($SSA_next>0&&($CARRY_old=$SSA_next,true));return [$SSA_next,$CARRY_old];",
        "do{$SSA_next=$CARRY_old-1;}while($SSA_next>0||($CARRY_old=$SSA_next,false));return $SSA_next;",
        "do{$SSA_next=$CARRY_old-1;}while($SSA_next>0?true:($CARRY_old=$SSA_next,false));return $SSA_next;",
        "do{$SSA_next=$CARRY_old-1;if(flag)continue;}while($SSA_next>0&&(flag||($CARRY_old=$SSA_next,true)));return $SSA_next;",
        "do{$SSA_next=$CARRY_old-1;}while(($SSA_next>0&&($CARRY_old=$SSA_next,false))||flag);return $SSA_next;",
        "do{$SSA_next=$CARRY_old-1;}while($SSA_next>0&&($CARRY_old=$SSA_next,true));use($CARRY_old);return $SSA_next;",
        "$SSA_next=$CARRY_old-1;if(flag&&($CARRY_old=$SSA_next,true)){}else use($CARRY_old);return $SSA_next;",
        "$SSA_next=$CARRY_old-1;(flag&&($CARRY_old=$SSA_next,true))||use($CARRY_old);return $SSA_next;",
        "$SSA_next=$CARRY_old-1;(flag||($CARRY_old=$SSA_next,false))&&use($CARRY_old);return $SSA_next;",
        "$SSA_next=$CARRY_old-1;if(flag??($CARRY_old=$SSA_next,true))use($CARRY_old);return $SSA_next;",
    ] {
        let source=format!("function f(flag){{let $CARRY_old=read();let $SSA_next;{tail}}}");
        let (code,count)=super::recycle_last_inputs(&source,&Default::default(),false).unwrap();
        assert_eq!(count,0,"{source}\n{code}");
    }
}

#[test]
fn initialized_versions_share_only_proven_equal_or_disjoint_live_ranges() {
    for (init, tail) in [
        ("$CARRY_old", "use($SSA_next,$CARRY_old);return $SSA_next;"),
        ("$CARRY_old-1", "$CARRY_old=$SSA_next;return [$SSA_next,$CARRY_old];"),
        ("$CARRY_old", "while($SSA_next>0){$SSA_next=$CARRY_old-1;$CARRY_old=$SSA_next;}return [$SSA_next,$CARRY_old];"),
        ("$CARRY_old", "try{use($SSA_next,$CARRY_old);}catch(e){report(e);}return $SSA_next;"),
        ("$CARRY_old", "use($SSA_next);$CARRY_old=read();return $CARRY_old;"),
        ("$CARRY_old", "if(flag){$SSA_next++;$CARRY_old=$SSA_next;}else{$SSA_next-=2;$CARRY_old=$SSA_next;}return [$SSA_next,$CARRY_old];"),
        ("$CARRY_old", "$SSA_next||=read();$CARRY_old=$SSA_next;return [$SSA_next,$CARRY_old];"),
        ("$CARRY_old", "use($SSA_next++);$CARRY_old=$SSA_next;return [$SSA_next,$CARRY_old];"),
        ("$CARRY_old", "for(let i=0;i<2;i++){use($SSA_next,$CARRY_old);}return $SSA_next;"),
        ("$CARRY_old", "do{$SSA_next=$CARRY_old-1;$CARRY_old=$SSA_next;}while($SSA_next>0);return $SSA_next;"),
    ] {
        let source=format!("function f(flag){{let $CARRY_old=read();let $SSA_next={init};{tail}}}");
        let (code,count)=super::recycle_last_inputs(&source,&Default::default(),false).unwrap();
        assert_eq!(count,1,"{source}\n{code}");
        assert!(!code.contains("$SSA_next"),"{code}");
    }
    for source in [
        "function f(){const $CARRY_old=read();const $SSA_next=$CARRY_old;return [$CARRY_old,$SSA_next];}",
        "function f(){let $CARRY_old=read();let $SSA_next=($CARRY_old);return [$CARRY_old,$SSA_next];}",
    ] {
        let (code,count)=super::recycle_last_inputs(source,&Default::default(),false).unwrap();
        assert_eq!(count,1,"{code}");
    }
}

#[test]
fn initialized_versions_keep_divergence_tdz_errors_names_and_captures() {
    for source in [
        "function f(){let $CARRY_old=read();let $SSA_next=$CARRY_old-1;return [$SSA_next,$CARRY_old];}",
        "function f(){let $CARRY_old=read();let $SSA_next=$CARRY_old++;return [$SSA_next,$CARRY_old];}",
        "function f(){let $CARRY_old=read();let $SSA_next=$CARRY_old;$SSA_next++;return [$SSA_next,$CARRY_old];}",
        "function f(){let $CARRY_old=read();let $SSA_next=$CARRY_old;$CARRY_old++;return [$SSA_next,$CARRY_old];}",
        "function f(){let $CARRY_old=read();let $SSA_next=$CARRY_old;if(flag)$CARRY_old=read();return [$SSA_next,$CARRY_old];}",
        "function f(){let $CARRY_old=read();let $SSA_next=$CARRY_old;try{$SSA_next=read();}catch(e){}return [$SSA_next,$CARRY_old];}",
        "function f(){let $CARRY_old=read();let $SSA_next=($CARRY_old=$SSA_next);return $SSA_next;}",
        "function f(){let $CARRY_old=read();const $SSA_next=$CARRY_old;$SSA_next=3;return [$SSA_next,$CARRY_old];}",
        "function f(){let $CARRY_old=read();let $SSA_next=$CARRY_old;$SSA_next=function(){};return [$SSA_next.name,$CARRY_old];}",
        "function f(){let $CARRY_old=read();let $SSA_next=$CARRY_old;return ()=>[$SSA_next,$CARRY_old];}",
    ] {
        let (code,count)=super::recycle_last_inputs(source,&Default::default(),false).unwrap();
        assert_eq!(count,0,"{source}\n{code}");
    }
}

#[test]
fn loop_versions_share_storage_in_ten_control_flow_forms() {
    for tail in [
        "while($SSA_next=$CARRY_old-1,$SSA_next>=0){use($SSA_next);$CARRY_old=$SSA_next;}return $SSA_next;",
        "for(;($SSA_next=$CARRY_old-1)>=0;$CARRY_old=$SSA_next){use($SSA_next);}return $SSA_next;",
        "do{$SSA_next=$CARRY_old-1;use($SSA_next);$CARRY_old=$SSA_next;}while($SSA_next>0);return $SSA_next;",
        "while(($SSA_next=$CARRY_old-1)>=0){$CARRY_old=$SSA_next;if(flag)continue;use($SSA_next);}return $SSA_next;",
        "for(;($SSA_next=$CARRY_old-1)>=0;$CARRY_old=$SSA_next){if(flag)continue;use($SSA_next);}return $SSA_next;",
        "while(($SSA_next=$CARRY_old-1)>=0){if(flag)break;use($SSA_next);$CARRY_old=$SSA_next;}return $SSA_next;",
        "while(($SSA_next=$CARRY_old-1)>=0){for(let j=0;j<2;j++){use($SSA_next);if(flag)break;}$CARRY_old=$SSA_next;}return $SSA_next;",
        "while(($SSA_next=$CARRY_old-1)>=0){if(flag)return $SSA_next;use($SSA_next);$CARRY_old=$SSA_next;}return $SSA_next;",
        "while(($SSA_next=$CARRY_old-1)>=0){if(flag)$CARRY_old=$SSA_next;else {$CARRY_old=$SSA_next;use($SSA_next);}}return $SSA_next;",
        "do{$SSA_next=$CARRY_old-1;use($SSA_next);}while(($CARRY_old=$SSA_next)>0);return $SSA_next;",
    ] {
        let source = format!("function f(flag){{let $CARRY_old=read();let $SSA_next;{tail}}}");
        let (code, count) = super::recycle_last_inputs(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{source}\n{code}");
        assert!(!code.contains("$SSA_next"), "{code}");
    }
}

#[test]
fn loop_versions_keep_old_exits_missing_copybacks_and_opaque_abrupt_edges() {
    for tail in [
        // A failed test changes next, but not the final old snapshot.
        "while(($SSA_next=$CARRY_old-1)>=0){$CARRY_old=$SSA_next;}return [$CARRY_old,$SSA_next];",
        "while(($SSA_next=$CARRY_old-1)>=0){if(flag)continue;$CARRY_old=$SSA_next;}return $SSA_next;",
        "while(($SSA_next=$CARRY_old-1)>=0){if(flag)break;$CARRY_old=$SSA_next;}return [$CARRY_old,$SSA_next];",
        "while(($SSA_next=$CARRY_old-1)>=0){use($CARRY_old);$CARRY_old=$SSA_next;}return $SSA_next;",
        "for(;($SSA_next=$CARRY_old-1)>=0;use($SSA_next,$CARRY_old)){$CARRY_old=$SSA_next+1;}return $SSA_next;",
        "do{$SSA_next=$CARRY_old-1;if(flag)continue;$CARRY_old=$SSA_next;}while($SSA_next>0);return $SSA_next;",
        "while(($SSA_next=$CARRY_old-1)>=0){try{if(flag)continue;}catch(e){}$CARRY_old=$SSA_next;}return $SSA_next;",
        "while(($SSA_next=$CARRY_old-1)>=0){try{if(flag)break;}finally{effect();}$CARRY_old=$SSA_next;}return $SSA_next;",
        "outer:while(($SSA_next=$CARRY_old-1)>=0){if(flag)continue outer;$CARRY_old=$SSA_next;}return $SSA_next;",
        "while(($SSA_next=$CARRY_old-1)>=0){for(let j=0;j<2;j++){if(flag)$CARRY_old=$SSA_next+1;}use($SSA_next);$CARRY_old=$SSA_next;}return $SSA_next;",
    ] {
        let source = format!("function f(flag){{let $CARRY_old=read();let $SSA_next;{tail}}}");
        let (code, count) = super::recycle_last_inputs(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 0, "{source}\n{code}");
    }
}

#[test]
fn structured_region_writes_invalidate_the_other_snapshot() {
    for region in [
        "while(flag--){$CARRY_next=read();}",
        "for(let i=0;i<flag;i++)$CARRY_next++;",
        "do{$CARRY_next=read();}while(flag--);",
        "for(const k in object)$CARRY_next=read();",
        "for(const v of values)$CARRY_next=read();",
        "try{$CARRY_next=read();}catch(e){}",
        "try{effect();}finally{$CARRY_next=read();}",
        "switch(flag){case 1:$CARRY_next++;break;}",
        "label:{if(flag)break label;$CARRY_next=read();}",
        "while(flag--){if(flag)continue;$CARRY_next++;}",
    ] {
        let source=format!("function f(flag){{let $SSA_old=read();let $CARRY_next;$CARRY_next=$SSA_old;{region}return [$SSA_old,$CARRY_next];}}");
        let (code, count) =
            super::recycle_last_inputs(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 0, "{code}");
    }
    for region in [
        "while(flag--){use($SSA_old,$CARRY_next);}",
        "try{use($SSA_old,$CARRY_next);}catch(e){}",
        "switch(flag){case 1:use($SSA_old,$CARRY_next);break;}",
        "for(let i=0;i<flag;i++)$SSA_old++;",
    ] {
        let source=format!("function f(flag){{let $SSA_old=read();let $CARRY_next;$CARRY_next=$SSA_old;{region}return $SSA_old;}}");
        let (code, count) =
            super::recycle_last_inputs(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{code}");
    }
}
#[test]
fn cells_with_the_same_undefined_entry_can_have_disjoint_later_lifetimes() {
    for declaration in [
        "let $SSA_old;let $CARRY_next;",
        "let $SSA_old,$CARRY_next;",
        "let $CARRY_next,$SSA_old;",
        "let other,$SSA_old,$CARRY_next;",
    ] {
        for tail in [
            "$CARRY_next=read();use($CARRY_next);$SSA_old=read();return $SSA_old;",
            "use($CARRY_next);$SSA_old=read();return $SSA_old;",
            "if(flag)$CARRY_next=read();use($CARRY_next);$SSA_old=read();return $SSA_old;",
        ] {
            let source = format!("function f(flag){{{declaration}{tail}}}");
            let (code, count) =
                super::recycle_ended_cells(&source, &Default::default(), false).unwrap();
            assert_eq!(count, 1, "{code}");
        }
    }
    for source in [
        "function f(){let $SSA_old;let $CARRY_next;$SSA_old=read();$CARRY_next=read();return [$SSA_old,$CARRY_next];}",
        "function f(){let $SSA_old;$SSA_old=7;let $CARRY_next;return [$SSA_old,$CARRY_next];}",
    ] {
        let(code,count)=super::recycle_ended_cells(source,&Default::default(),false).unwrap();
        assert_eq!(count,0,"{code}");
    }
}
#[test]
fn ended_cells_preserve_undefined_initialization_in_ten_control_forms() {
    for tail in [
        "if(flag)$CARRY_next=read();return $CARRY_next;",
        "flag&&($CARRY_next=read());return $CARRY_next;",
        "flag?.call($CARRY_next=read());return $CARRY_next;",
        "while(flag--)$CARRY_next=read();return $CARRY_next;",
        "for(let i=0;i<flag;i++)$CARRY_next=read();return $CARRY_next;",
        "do{$CARRY_next=read();}while(flag--);return $CARRY_next;",
        "try{if(flag)$CARRY_next=read();}catch(e){}return $CARRY_next;",
        "try{effect();}finally{if(flag)$CARRY_next=read();}return $CARRY_next;",
        "switch(flag){case 1:$CARRY_next=read();break;}return $CARRY_next;",
        "label:{if(flag)break label;$CARRY_next=read();}return $CARRY_next;",
    ] {
        let source = format!(
            "function f(flag){{let $SSA_old=read();consume($SSA_old);let $CARRY_next;{tail}}}"
        );
        let (code, count) =
            super::recycle_ended_cells(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(!code.contains("$CARRY_next"), "{code}");
        assert!(code.contains("$SSA_old = void 0"), "{code}");
    }
    for declaration in [
        "let $CARRY_next,before;",
        "let before,$CARRY_next;",
        "let before,$CARRY_next,after;",
    ] {
        let source=format!("function f(flag){{let $SSA_old=read();consume($SSA_old);{declaration}if(flag)$CARRY_next=read();return $CARRY_next;}}");
        let (code, count) =
            super::recycle_ended_cells(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(code.contains("before"), "{code}");
        assert!(!code.contains("$CARRY_next"), "{code}");
        let allocator = oxc_allocator::Allocator::default();
        crate::analysis::parse_program(&allocator, &code).unwrap();
    }
}
#[test]
fn ended_private_live_ranges_cover_ten_independent_producer_forms() {
    for value in [
        "read()+read()",
        "read()",
        "object.x",
        "new Wrapper(read())",
        "`value:${read()}`",
        "{value:read()}",
        "[read()]",
        "flag?read():other()",
        "flag&&read()",
        "typeof missing",
    ] {
        let source=format!("function f(flag){{const $SSA_old=read();consume($SSA_old);let $SSA_next={value};return [$SSA_next,$SSA_next];}}");
        let (code, count) =
            super::recycle_ended_cells(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(!code.contains("$SSA_next"), "{code}");
        assert!(code.contains("let $SSA_old"), "{code}");
    }
    for source in [
        "function f(){let $SSA_old=read();consume($SSA_old);let $SSA_next=read();return [$SSA_next,$SSA_old];}",
        "function f(){let $SSA_old=read();consume($SSA_old);let $SSA_next=read();return [$SSA_next,()=> $SSA_old];}",
        "function f(){let $SSA_old=read();consume($SSA_old);let $SSA_next=read();return [$SSA_next,()=> $SSA_next];}",
        "function f(){const $SSA_old=read();consume($SSA_old);let $SSA_next=read();$SSA_old=3;return $SSA_next;}",
        "function f(){let $SSA_old=read();consume($SSA_old);while(test()){let $SSA_next=read();consume($SSA_next,$SSA_next);}}",
        "function f(){{let $SSA_old=read();consume($SSA_old);}let $SSA_next=read();return [$SSA_next,$SSA_next];}",
        "function f(){let $SSA_old=read();consume($SSA_old);let $SSA_next=()=>{};return $SSA_next.name;}",
    ] {
        let(code,count)=super::recycle_ended_cells(source,&Default::default(),false).unwrap();
        assert_eq!(count,0,"{code}");
    }
}
#[test]
fn grouped_branch_channels_keep_their_other_declarators_and_lifetimes() {
    for declaration in [
        "let $CARRY_next, untouched;",
        "let untouched, $CARRY_next;",
        "let before, $CARRY_next, after;",
    ] {
        let source=format!("function f(flag){{let $SSA_old=read();{declaration}if(flag)$CARRY_next=$SSA_old+1;else $CARRY_next=encode($SSA_old);return $CARRY_next;}}");
        let (code, count) =
            super::recycle_last_inputs(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(!code.contains("$CARRY_next"), "{code}");
        assert!(
            code.contains(if declaration.contains("untouched") {
                "untouched"
            } else {
                "after"
            }),
            "{code}"
        );
        let allocator = oxc_allocator::Allocator::default();
        crate::analysis::parse_program(&allocator, &code).unwrap();
    }
    let source="function f(flag){let $SSA_old=read();let unrelated=effect(),$CARRY_next;if(flag)$CARRY_next=$SSA_old+1;else $CARRY_next=3;return $CARRY_next;}";
    assert_eq!(
        super::recycle_last_inputs(source, &Default::default(), false)
            .unwrap()
            .1,
        0
    );
}
#[test]
fn private_copy_back_stores_restore_only_the_value_they_actually_copy() {
    for tail in [
        "$CARRY_next=$SSA_old;return [$CARRY_next,$SSA_old];",
        "$CARRY_next=$SSA_old+1;$SSA_old=$CARRY_next;return [$CARRY_next,$SSA_old];",
        "if(flag){$CARRY_next=$SSA_old+1;$SSA_old=$CARRY_next;}else{$CARRY_next=$SSA_old;}return [$CARRY_next,$SSA_old];",
        "$CARRY_next=$SSA_old+1;$SSA_old=$CARRY_next;use($CARRY_next);$SSA_old++;return $SSA_old;",
        "$SSA_old++;$CARRY_next=$SSA_old+1;$SSA_old=$CARRY_next;return [$CARRY_next,$SSA_old];",
        "$CARRY_next=$SSA_old+1;$SSA_old=$CARRY_next;$SSA_old+=read();$CARRY_next=$SSA_old;return [$CARRY_next,$SSA_old];",
        "$CARRY_next=$SSA_old;$SSA_old||=read();$CARRY_next=$SSA_old;return [$CARRY_next,$SSA_old];",
        "$CARRY_next=$SSA_old;$SSA_old&&=read();$CARRY_next=$SSA_old;return [$CARRY_next,$SSA_old];",
        "$CARRY_next=$SSA_old;$SSA_old??=read();$CARRY_next=$SSA_old;return [$CARRY_next,$SSA_old];",
        "$CARRY_next=$SSA_old;$SSA_old=($SSA_old);return [$CARRY_next,$SSA_old];",
    ] {
        let source=format!("function f(flag){{let $SSA_old=read();let $CARRY_next;{tail}}}");
        let(code,count)=super::recycle_last_inputs(&source,&Default::default(),false).unwrap();
        assert_eq!(count,1,"{code}");
    }
    for tail in [
        "$CARRY_next=$SSA_old+1;use($SSA_old);return $CARRY_next;",
        "$CARRY_next=$SSA_old;$SSA_old++;return $CARRY_next;",
        "$CARRY_next=$SSA_old;$CARRY_next++;return $SSA_old;",
        "$CARRY_next=$SSA_old;$SSA_old||=read();return $CARRY_next;",
        "$CARRY_next=$SSA_old;$SSA_old&&=read();return $CARRY_next;",
        "$CARRY_next=$SSA_old;$SSA_old??=read();return $CARRY_next;",
        "if(flag){$CARRY_next=$SSA_old+1;$SSA_old=$CARRY_next;}else{$CARRY_next=3;}return [$CARRY_next,$SSA_old];",
    ] {
        let source=format!("function f(flag){{let $SSA_old=read();let $CARRY_next;{tail}}}");
        let(code,count)=super::recycle_last_inputs(&source,&Default::default(),false).unwrap();
        assert_eq!(count,0,"{code}");
    }
}
#[test]
fn initialized_successor_storage_is_invariant_through_structured_regions() {
    for middle in [
        "for(let i=0;i<3;i++)$CARRY_next++;",
        "while(flag--)$CARRY_next++;",
        "do{$CARRY_next++;}while(flag--);",
        "for(const k in obj)$CARRY_next+=1;",
        "for(const v of values)$CARRY_next+=v;",
        "try{$CARRY_next=read();}catch(e){$CARRY_next=7;}",
        "try{effect();}finally{$CARRY_next++;}",
        "switch(flag){case 1:$CARRY_next++;break;default:$CARRY_next+=2;}",
        "label:{if(flag)break label;$CARRY_next++;}",
        "for(let i=0;i<3;i++){if(i)continue;$CARRY_next++;}",
    ] {
        let source=format!("function f(flag){{let $SSA_old=read();let $CARRY_next;$CARRY_next=$SSA_old+1;{middle}return $CARRY_next;}}");
        let (code, count) =
            super::recycle_last_inputs(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{code}");
    }
    for middle in [
        "while(flag--){$CARRY_next=$SSA_old+1;}",
        "try{$CARRY_next=read($SSA_old);}catch(e){}",
        "switch(flag){case 1:$CARRY_next=$SSA_old+1;}",
        "label:{if(flag)break label;$CARRY_next=$SSA_old+1;}",
    ] {
        let source=format!("function f(flag){{let $SSA_old=read();let $CARRY_next;{middle}return $CARRY_next;}}");
        let (code, count) =
            super::recycle_last_inputs(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 0, "{code}");
    }
}
#[test]
fn branch_successors_recycle_only_after_each_paths_last_old_value_read() {
    for tail in [
        "if(flag)$CARRY_next=$SSA_old+1;else $CARRY_next=encode($SSA_old);return $CARRY_next;",
        "if(flag){$CARRY_next=$SSA_old+1;out[$CARRY_next]=read();}else $CARRY_next=encode($SSA_old);return $CARRY_next;",
        "flag?($CARRY_next=$SSA_old+1):($CARRY_next=encode($SSA_old));return $CARRY_next;",
        "$CARRY_next=flag?$SSA_old+1:encode($SSA_old);return $CARRY_next;",
        "if(flag){$CARRY_next=$SSA_old+1;}else{return $SSA_old;}return $CARRY_next;",
        "if(flag){throw $SSA_old;}else{$CARRY_next=encode($SSA_old);}return $CARRY_next;",
        "if(flag)$CARRY_next=$SSA_old;else{$CARRY_next=read();$CARRY_next=$CARRY_next.x;}return $CARRY_next;",
        "$CARRY_next=encode($SSA_old);$CARRY_next++;return $CARRY_next;",
        "if(flag){effect($SSA_old);$CARRY_next=3;}else{$CARRY_next=$SSA_old+1;}return { $CARRY_next };",
        "$CARRY_next=(effect(),encode($SSA_old));if(flag)$CARRY_next+=2;return $CARRY_next;",
    ] {
        let source=format!("function f(flag){{let $SSA_old=read();let $CARRY_next;{tail}}}");
        let(code,count)=super::recycle_last_inputs(&source,&Default::default(),false).unwrap();
        assert_eq!(count,1,"{code}");
        assert!(!code.contains("let $CARRY_next"),"{code}");
    }
    for tail in [
        "if(flag)$CARRY_next=$SSA_old+1;return $CARRY_next;",
        "$CARRY_next=$SSA_old+1;return [$CARRY_next,$SSA_old];",
        "if(flag){$CARRY_next=3;use($SSA_old);}else $CARRY_next=$SSA_old;return $CARRY_next;",
        "$CARRY_next=encode($SSA_old);$SSA_old=3;return $CARRY_next;",
        "flag?.call($CARRY_next=$SSA_old);return $CARRY_next;",
        "flag&&($CARRY_next=$SSA_old);return $CARRY_next;",
        "if(flag)$CARRY_next=$SSA_old;else use($CARRY_next);return $CARRY_next;",
        "$CARRY_next=()=> $SSA_old;return $CARRY_next;",
        "try{$CARRY_next=encode($SSA_old);}catch(e){return $CARRY_next;}return $CARRY_next;",
        "while(flag--){$CARRY_next=$SSA_old+1;}return $CARRY_next;",
    ] {
        let source = format!("function f(flag){{let $SSA_old=read();let $CARRY_next;{tail}}}");
        let (code, count) =
            super::recycle_last_inputs(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 0, "{code}");
    }
}
#[test]
fn nested_and_mutable_successors_reuse_dead_inputs_in_ten_contexts() {
    for tail in [
        "{let $SSA_next=$SSA_old+1;use($SSA_next,$SSA_next);}",
        "if(flag){let $SSA_next=$SSA_old+1;use($SSA_next,$SSA_next);}",
        "if(flag){}else{let $SSA_next=$SSA_old+1;use($SSA_next,$SSA_next);}",
        "try{let $SSA_next=$SSA_old+1;use($SSA_next,$SSA_next);}catch(e){report(e);}",
        "try{effect();}catch(e){let $SSA_next=$SSA_old+1;use($SSA_next,$SSA_next);}",
        "try{effect();}finally{let $SSA_next=$SSA_old+1;use($SSA_next,$SSA_next);}",
        "let $SSA_next=$SSA_old+1;$SSA_next++;use($SSA_next,$SSA_next);",
        "let $SSA_next=$SSA_old+1;++$SSA_next;use($SSA_next,$SSA_next);",
        "let $SSA_next=$SSA_old+1;$SSA_next+=read();use($SSA_next,$SSA_next);",
        "let $SSA_next=$SSA_old+1;if(flag)$SSA_next=read();use($SSA_next,$SSA_next);",
    ] {
        let source = format!("function f(flag){{let $SSA_old=read();{tail}}}");
        let (code, count) =
            super::recycle_last_inputs(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(!code.contains("$SSA_next"), "{code}");
    }
}
#[test]
fn header_updates_inferred_names_and_const_successor_errors_remain_observable() {
    for source in [
        "function f(){for(let $SSA_old=read();test();$SSA_old++){let $SSA_next=$SSA_old+1;use($SSA_next,$SSA_next);}}",
        "function f(){let $SSA_old=read();let $SSA_next=$SSA_old+1;$SSA_next=function(){};return $SSA_next.name;}",
        "function f(){let $SSA_old=read();let $SSA_next=$SSA_old+1;$SSA_next=()=>{};return $SSA_next.name;}",
        "function f(){let $SSA_old=read();let $SSA_next=$SSA_old+1;$SSA_next=class{};return $SSA_next.name;}",
        "function f(){let $SSA_old=read();const $SSA_next=$SSA_old+1;$SSA_next=3;return $SSA_next;}",
        "function f(){let $SSA_old=read();let $SSA_next=$SSA_old+1;[$SSA_next]=read();return $SSA_next;}",
        "function f(){let $SSA_old=read();if(flag){let $SSA_next=$SSA_old+1;use($SSA_next);}return $SSA_old;}",
        "function f(){let $SSA_old=read();if(flag){let $SSA_old=3;use($SSA_old);}let $SSA_next=$SSA_old+1;return ()=> $SSA_next;}",
    ] {
        let (code,count)=super::recycle_last_inputs(source,&Default::default(),false).unwrap();
        assert_eq!(count,0,"{code}");
    }
}
#[test]
fn one_storage_rule_covers_ten_computed_successor_contexts() {
    for value in [
        "$SSA_old|read()",
        "-$SSA_old",
        "flag?$SSA_old:read()",
        "$SSA_old&&read()",
        "read($SSA_old)",
        "new Wrapper($SSA_old)",
        "$SSA_old['x']",
        "(target=$SSA_old)",
        "[$SSA_old]",
        "(effect(),$SSA_old+1)",
    ] {
        let source=format!("function f(flag){{let $SSA_old=read();let target;let $SSA_next={value};return [$SSA_next,$SSA_next];}}");
        let (code, count) =
            super::recycle_last_inputs(&source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(!code.contains("$SSA_next"), "{code}");
    }
    for source in [
        "function f(){const $CARRY_old=read();let $SSA_next=$CARRY_old+1;return [$SSA_next,$SSA_next];}",
        "function f(){let $CARRY_old; if(flag)$CARRY_old=read();else $CARRY_old=3;let $SSA_next=$CARRY_old+1;return $SSA_next;}",
        "function f(){let $CARRY_old,$CARRY_other;$CARRY_old=read();let $SSA_next=$CARRY_old+1;return [$SSA_next,$CARRY_other];}",
    ] {
        let (code,count)=super::recycle_last_inputs(source,&Default::default(),false).unwrap();assert_eq!(count,1,"{code}");
    }
}
#[test]
fn live_old_values_const_errors_captures_and_new_loop_entries_stay_separate() {
    for source in [
        "function f(){let $SSA_old=read();let $SSA_next=$SSA_old+1;return [$SSA_next,$SSA_old];}",
        "function f(){let $SSA_old=read();let $SSA_next=$SSA_old+1;function observe(){return $SSA_old;}return [$SSA_next,observe()];}",
        "function f(){let $SSA_old=read();let $SSA_next=$SSA_old+1;return [$SSA_next,()=> $SSA_next];}",
        "function f(){const $SSA_old=3;let $SSA_next=($SSA_old=4);return $SSA_next;}",
        "function f(){const $SSA_old=3;let $SSA_next=$SSA_old+1;$SSA_old=4;return $SSA_next;}",
        "function f(){const $SSA_old=3;let out=[];for(let i=0;i<2;i++){let $SSA_next=$SSA_old+1;out.push($SSA_next);}return out;}",
        "function f(){let $SSA_old=3;try{let $SSA_next=$SSA_old+1;use($SSA_next);}catch(e){return $SSA_old;}}",
        "function f(){let $SSA_old=3;try{let $SSA_next=$SSA_old+1;return $SSA_next;}finally{return $SSA_old;}}",
        "function f(){let $SSA_old=3;let $SSA_next=function(){};use($SSA_old);return $SSA_next.name;}",
    ] {
        let (code,count)=super::recycle_last_inputs(source,&Default::default(),false).unwrap();assert_eq!(count,0,"{code}");
    }
}
