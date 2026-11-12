#[test]
fn argument_channels_cross_branch_and_conditional_store_orders() {
    for source in [
        "function f(){let $CARRY_0;if(arguments[0]){$CARRY_0=arguments[1];left();}else{right();$CARRY_0=arguments[1];}return $CARRY_0;}",
        "function f(){let $CARRY_0;let x=arguments[0]?($CARRY_0=arguments[1],left()):(right(),$CARRY_0=arguments[1],2);return [$CARRY_0,x];}",
        "function f(){let $CARRY_0;if(arguments[0]){if(arguments[2])return 2;$CARRY_0=arguments[1];}else{$CARRY_0=arguments[1];}return $CARRY_0;}",
    ] {
        let (code,a,_)=super::eliminate_stable_channels(source,&Default::default(),false).unwrap();
        assert_eq!(a,1,"{code}");assert!(!code.contains("$CARRY_0"),"{code}");
    }
}
#[test]
fn parallel_channel_copies_track_source_stores_after_rhs() {
    let source="function f(flag){let current;let $CARRY_0;if(flag){current=left();$CARRY_0=current;}else{current=right();$CARRY_0=current;}return [$CARRY_0,current];}";
    let (code, _, a) =
        super::eliminate_stable_channels(source, &Default::default(), false).unwrap();
    assert_eq!(a, 1, "{code}");
    assert!(!code.contains("$CARRY_0"), "{code}");
    for source in [
        "function f(flag){let current=1;let $CARRY_0;if(flag){$CARRY_0=current;}return $CARRY_0;}",
        "function f(flag){let current=1;let $CARRY_0;$CARRY_0=current;current=2;return $CARRY_0;}",
        "function f(flag){let current=1;let $CARRY_0;$CARRY_0=current;[current]=[2];return $CARRY_0;}",
        "function f(flag){let current=1;let $CARRY_0;$CARRY_0=current;var current=2;return $CARRY_0;}",
        "function f(flag){let current=1;let $CARRY_0;$CARRY_0=current;function change(){current=2;}change();return $CARRY_0;}",
        "function f(){let $CARRY_0;if(arguments[0])$CARRY_0=arguments[1];else $CARRY_0=arguments[1];arguments[1]=2;return $CARRY_0;}",
    ] {
        let (_,a,b)=super::eliminate_stable_channels(source,&Default::default(),false).unwrap();
        assert_eq!(a+b,0,"{source}");
    }
}
#[test]
fn test_refreshed_alias_is_current_after_a_failing_loop_test() {
    let source="function f(){let n=0;let $SSA_0;while($SSA_0=n,$SSA_0<3){observe($SSA_0);n=$SSA_0+1;}return $SSA_0;}";
    let (code, _, count) =
        super::eliminate_stable_channels(source, &Default::default(), false).unwrap();
    assert_eq!(count, 1, "{code}");
    assert!(!code.contains("$SSA_0"), "{code}");
    let source="function f(){let carry=0;let index=0;let $SSA_0;while($SSA_0=carry,index<3){let next=($SSA_0<<5)-carry+values[index];index=Number(index)+1;carry=next|0;}return $SSA_0;}";
    let (code, _, count) =
        super::eliminate_stable_channels(source, &Default::default(), false).unwrap();
    assert_eq!(count, 1, "{code}");
}
#[test]
fn initialized_snapshots_keep_their_source_version_through_nested_loops() {
    for source in [
        "function f(){let source=read();const $SSA_0=source;for(let i=0;i<3;i++){consume($SSA_0);}source=other();return source;}",
        "function f(flag){let source=read();let $CARRY_0;if(flag)$CARRY_0=source;else $CARRY_0=source;for(let i=0;i<3;i++){consume($CARRY_0);}source=other();return source;}",
        "function f(){let n=0;let $SSA_0;for(let i=0;$SSA_0=n,i<3;i++){n=$SSA_0+1;continue;}return $SSA_0;}",
    ] {
        let (code,_,count)=super::eliminate_stable_channels(source,&Default::default(),false).unwrap();
        assert_eq!(count,1,"{code}");
    }
    for source in [
        "function f(){let source=read();const $SSA_0=source;for(let i=0;i<3;i++){consume($SSA_0);source=other();}}",
        "function f(){let source=read();const $SSA_0=source;source=other();return $SSA_0;}",
        "function f(){let source=read();const $SSA_0=source;for(let i=0;i<3;i++){consume($SSA_0);}source=other();return $SSA_0;}",
    ] {
        let (_,a,b)=super::eliminate_stable_channels(source,&Default::default(),false).unwrap();
        assert_eq!(a+b,0,"{source}");
    }
}
#[test]
fn constant_alias_stores_retain_their_error() {
    let source = "function f(){let source=1;const $SSA_0=source;$SSA_0=source;return $SSA_0;}";
    let (_, a, b) =
        super::eliminate_stable_channels(source, &Default::default(), false).unwrap();
    assert_eq!(a + b, 0);
}
#[test]
fn shared_and_closure_writable_inputs_use_effect_free_windows() {
    let protected = ["$KEY_0".into()].into_iter().collect();
    for source in [
        "function f(){let $KEY_0=read();const $SSA_0=$KEY_0;return $SSA_0===null||$SSA_0===false;}",
        "function f(){let $KEY_0=read();function change(){$KEY_0=2;}const $SSA_0=$KEY_0;let n=3;return [$SSA_0,n];}",
    ] {
        let (code,_,count)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(count,1,"{code}");
    }
    for source in [
        "function f(){let $KEY_0=1;function change(){$KEY_0=2;}const $SSA_0=$KEY_0;change();return $SSA_0;}",
        "function f(){let $KEY_0=1;const obj={get x(){$KEY_0=2;return 0;}};const $SSA_0=$KEY_0;obj.x;return $SSA_0;}",
        "function f(){let $KEY_0=1;const obj={valueOf(){$KEY_0=2;return 0;}};const $SSA_0=$KEY_0;+obj;return $SSA_0;}",
        "function f(){let $KEY_0=1;const obj={get x(){$KEY_0=2;return 0;}};const $SSA_0=$KEY_0;let {x}=obj;return $SSA_0;}",
    ] {
        let (_,a,b)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(a+b,0,"{source}");
    }
}
#[test]
fn incoming_slot_aliases_preserve_the_first_read_and_effect_barriers() {
    let protected = ["$KEY_0".into(), "$KEY_1".into()].into_iter().collect();
    for source in [
        "function f(){const $SSA_0=$KEY_0;return $SSA_0===null||$SSA_0===false;}",
        "function f(){const $SSA_0=$KEY_0;return typeof $SSA_0;}",
        "function f(){const $SSA_0=$KEY_0;return $SSA_0();}",
        "function f(){const $SSA_0=$KEY_0;return [$SSA_0,$SSA_0];}",
    ] {
        let (code, _, count) =
            super::eliminate_stable_channels(source, &protected, false).unwrap();
        assert_eq!(count, 1, "{code}");
    }
    for source in [
        "function f(){const $SSA_0=$KEY_0;return unknown($SSA_0);}",
        "function f(){const $SSA_0=$KEY_0;return [$KEY_1,$SSA_0];}",
        "function f(){const $SSA_0=$KEY_0;return [$SSA_0,change(),$SSA_0];}",
        "function f(){const $SSA_0=$KEY_0;$KEY_0=2;return $SSA_0;}",
        "function f(){const $SSA_0=$KEY_0;return [$SSA_0,(obj.x),$SSA_0];}",
        "function f(){const $SSA_0=$KEY_0;return [+$SSA_0,$SSA_0];}",
        "function f(){const $SSA_0=$KEY_0;return [$SSA_0,(()=>{let $KEY_0=2;return $SSA_0;})()];}",
    ] {
        let (_,a,b)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(a+b,0,"{source}");
    }
}
#[test]
fn retained_declarators_keep_their_const_contract() {
    let source =
        "function f(){let source=1;const $SSA_0=source, other=2;other=3;return $SSA_0;}";
    let (code, _, count) =
        super::eliminate_stable_channels(source, &Default::default(), false).unwrap();
    assert_eq!(count, 1, "{code}");
    assert!(code.contains("const other=2;"), "{code}");
}
#[test]
fn prior_successful_reads_prove_incoming_initialization_on_every_normal_path() {
    let protected = ["$KEY_0".into(), "$KEY_1".into()].into_iter().collect();
    for source in [
        "function f(){if($KEY_0){const $SSA_0=$KEY_0;$KEY_1=arguments[0];return $SSA_0;}}",
        "function f(flag){if(flag){consume($KEY_0);}else{consume($KEY_0);}const $SSA_0=$KEY_0;$KEY_1=1;return $SSA_0;}",
        "function f(flag){if(flag){return 0;}else{consume($KEY_0);}const $SSA_0=$KEY_0;$KEY_1=1;return $SSA_0;}",
    ] {
        let (code,_,count)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(count,1,"{code}");
    }
    for source in [
        "function f(flag){if(flag)consume($KEY_0);const $SSA_0=$KEY_0;$KEY_1=1;return $SSA_0;}",
        "function f(){flag &&= $KEY_0;const $SSA_0=$KEY_0;$KEY_1=1;return $SSA_0;}",
        "function f(){obj?.[$KEY_0];const $SSA_0=$KEY_0;$KEY_1=1;return $SSA_0;}",
        "function f(){switch(flag){case $KEY_0: break;default:break;}const $SSA_0=$KEY_0;$KEY_1=1;return $SSA_0;}",
        "function f(){typeof $KEY_0;const $SSA_0=$KEY_0;$KEY_1=1;return $SSA_0;}",
        "function f(){try{mightThrow();consume($KEY_0);}catch(e){}const $SSA_0=$KEY_0;$KEY_1=1;return $SSA_0;}",
        "function f(x=$KEY_0){const $SSA_0=$KEY_0;$KEY_1=1;return $SSA_0;}",
        "function f(){let {x=$KEY_0}=obj;const $SSA_0=$KEY_0;$KEY_1=1;return $SSA_0;}",
        "function f(){while(flag)consume($KEY_0);const $SSA_0=$KEY_0;$KEY_1=1;return $SSA_0;}",
    ] {
        let (_,a,b)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(a+b,0,"{source}");
    }
}
#[test]
fn exact_restorations_reestablish_argument_literal_and_immutable_values() {
    let protected = ["$KEY_0".into()].into_iter().collect();
    for source in [
        "function f(){$KEY_0=arguments[0];const $SSA_0=$KEY_0;$KEY_0=other();$KEY_0=arguments[0];return $SSA_0;}",
        "function f(){$KEY_0=7;const $SSA_0=$KEY_0;$KEY_0=other();$KEY_0=7;return $SSA_0;}",
        "function f(){const saved={};$KEY_0=saved;const $SSA_0=$KEY_0;$KEY_0=other();$KEY_0=saved;return $SSA_0;}",
    ] {
        let (code,_,count)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(count,1,"{code}");
    }
    for source in [
        "function f(){$KEY_0=arguments[0];const $SSA_0=$KEY_0;$KEY_0=arguments[1];return $SSA_0;}",
        "function f(){$KEY_0=0;const $SSA_0=$KEY_0;$KEY_0=-0;return $SSA_0;}",
        "function f(){let saved={};$KEY_0=saved;const $SSA_0=$KEY_0;saved={};$KEY_0=saved;return $SSA_0;}",
        "function f(){let $SSA_0;$KEY_0=7;consume($SSA_0);$SSA_0=$KEY_0;return $SSA_0;}",
        "function f(){$KEY_0=arguments[0];const $SSA_0=$KEY_0;arguments[0]=2;$KEY_0=arguments[0];return $SSA_0;}",
        "function f(flag){$KEY_0=1;const $SSA_0=$KEY_0;if(flag)$KEY_0=1;else $KEY_0=2;return $SSA_0;}",
    ] {
        let (_,a,b)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(a+b,0,"{source}");
    }
}
#[test]
fn successful_primitive_stores_allow_repeated_snapshot_reads_without_coercion_hooks() {
    let protected = ["$KEY_0".into()].into_iter().collect();
    for source in [
        "function f(){$KEY_0=+arguments[0];const $SSA_0=$KEY_0;return ($SSA_0<<5)-$SSA_0;}",
        "function f(){if(typeof $KEY_0===\"number\"){const $SSA_0=$KEY_0;return ($SSA_0<<5)-$SSA_0;}return 0;}",
        "function f(){$KEY_0=arguments[0]*2;const $SSA_0=$KEY_0;return ($SSA_0*3)+$SSA_0;}",
        "function f(flag){if(flag)$KEY_0=1;else $KEY_0=2;const $SSA_0=$KEY_0;return ($SSA_0*3)+$SSA_0;}",
    ] {
        let (code,_,count)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(count,1,"{code}");
    }
    for source in [
        "function f(){$KEY_0=arguments[0];const $SSA_0=$KEY_0;return ($SSA_0<<5)-$SSA_0;}",
        "function f(flag){if(flag)$KEY_0=1;else $KEY_0=object;const $SSA_0=$KEY_0;return ($SSA_0*3)+$SSA_0;}",
        "function f(){$KEY_0=1;change();const $SSA_0=$KEY_0;return ($SSA_0*3)+$SSA_0;}",
    ] {
        let (_,a,b)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(a+b,0,"{source}");
    }
}
#[test]
fn existing_slot_copy_outs_can_own_a_producer_without_an_extra_snapshot() {
    let protected = ["$KEY_0".into()].into_iter().collect();
    for source in [
        "function f(){let $SSA_0=compute();$KEY_0=$SSA_0;return $SSA_0;}",
        "function f(){let $SSA_0=Number($KEY_0)-1;$KEY_0=$SSA_0;if($SSA_0<=0)done();}",
        "function f(){let $SSA_0=arguments[0]*2;$KEY_0=$SSA_0;return ($SSA_0<<5)-$SSA_0;}",
        "function f(){let $SSA_0=new Thing();$KEY_0=$SSA_0;return [$SSA_0,$SSA_0];}",
    ] {
        let (code, _, count) =
            super::eliminate_stable_channels(source, &protected, false).unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(!code.contains("$SSA_0"), "{code}");
    }
    for source in [
        "function f(){let $SSA_0=compute();$KEY_0=$SSA_0;change();return $SSA_0;}",
        "function f(){let $SSA_0=compute();$KEY_0=$SSA_0;return [$SSA_0,object.x,$SSA_0];}",
        "function f(){let $SSA_0=compute();$KEY_0=$SSA_0;$KEY_0=9;return $SSA_0;}",
        "function f(){let $SSA_0=()=>1;$KEY_0=$SSA_0;return $SSA_0.name;}",
        "function f(){let $SSA_0=compute();$KEY_0=$SSA_0;function child(){return $SSA_0;}return child();}",
        "function f(){let $SSA_0=compute();$KEY_0=$SSA_0;$SSA_0=9;return $SSA_0;}",
        "function f(){let $SSA_0=compute();$KEY_0=$SSA_0;return ($SSA_0<<5)-$SSA_0;}",
    ] {
        let (_,a,b)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(a+b,0,"{source}");
    }
}
#[test]
fn delayed_inert_producers_keep_the_slot_store_at_its_original_point() {
    let protected = ["$KEY_0".into()].into_iter().collect();
    for source in [
        "function f(){const n=7;let $SSA_0=n+1;work();$KEY_0=$SSA_0;return [$SSA_0,$SSA_0];}",
        "function f(){let $SSA_0=[arguments[0]];work();$KEY_0=$SSA_0;return [$SSA_0===$SSA_0,$SSA_0];}",
        "function f(){let $SSA_0=make();var $KEY_0=$SSA_0;return $SSA_0;}",
    ] {
        let (code,_,count)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(count,1,"{code}");assert!(!code.contains("$SSA_0"),"{code}");
    }
    for source in [
        "function f(){let $SSA_0=make();work();$KEY_0=$SSA_0;return $SSA_0;}",
        "function f(){let n=1;let $SSA_0=n+1;n=2;$KEY_0=$SSA_0;return $SSA_0;}",
        "function f(){let $SSA_0=object.x;work();$KEY_0=$SSA_0;return $SSA_0;}",
        "function f(){let $SSA_0=arguments[0]+1;work();$KEY_0=$SSA_0;return $SSA_0;}",
        "function f(){let $SSA_0=make();var $KEY_0=$SSA_0;change();return $SSA_0;}",
    ] {
        let (_, a, b) = super::eliminate_stable_channels(source, &protected, false).unwrap();
        assert_eq!(a + b, 0, "{source}");
    }
}
#[test]
fn branch_copy_outs_transfer_only_terminal_normal_path_stores() {
    let protected = ["$KEY_0".into()].into_iter().collect();
    for source in [
        "function f(flag){let $CARRY_0;if(flag)$CARRY_0=left();else $CARRY_0=right();$KEY_0=$CARRY_0;return 0;}",
        "function f(flag){let $CARRY_0;if(flag){work();$CARRY_0=left();}else{$CARRY_0=right();}$KEY_0=$CARRY_0;return $CARRY_0;}",
        "function f(a,b){let $CARRY_0;if(a){if(b){$CARRY_0=left();}else{return 3;}}else{$CARRY_0=right();}$KEY_0=$CARRY_0;return $CARRY_0;}",
        "function f(flag){let $CARRY_0;if(flag)$CARRY_0=left();else $CARRY_0=right();var $KEY_0=$CARRY_0;return $CARRY_0;}",
    ] {
        let (code,_,count)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(count,1,"{code}");assert!(!code.contains("$CARRY_0"),"{code}");
    }
    for source in [
        "function f(flag){let $CARRY_0;if(flag){$CARRY_0=left();work();}else{$CARRY_0=right();}$KEY_0=$CARRY_0;return $CARRY_0;}",
        "function f(flag){let $CARRY_0;if(flag){let $KEY_0;$CARRY_0=left();}else{$CARRY_0=right();}$KEY_0=$CARRY_0;return $CARRY_0;}",
        "function f(flag){let $CARRY_0;if(flag)$CARRY_0=left();else $CARRY_0=right();$KEY_0=$CARRY_0;change();return $CARRY_0;}",
        "function f(flag){let $CARRY_0;if(flag)$CARRY_0=()=>1;else $CARRY_0=()=>2;$KEY_0=$CARRY_0;return $CARRY_0.name;}",
        "function f(flag){let $CARRY_0;if(flag)$CARRY_0=$CARRY_0;else $CARRY_0=1;$KEY_0=$CARRY_0;return $CARRY_0;}",
    ] {
        let (_,a,b)=super::eliminate_stable_channels(source,&protected,false).unwrap();
        assert_eq!(a+b,0,"{source}");
    }
}
