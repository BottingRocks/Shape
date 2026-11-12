#[test]
fn keeps_a_source_a_block_function_overwrites() {
    for source in [
        "function f(){var $SSA_s=read();var slot=$SSA_s;{function $SSA_s(){}}return [$SSA_s,slot];}",
        "function f(){var $SSA_s=read();state=$SSA_s;{function $SSA_s(){}}consume($SSA_s);}",
    ] {
        let (code, count) = super::coalesce_local_handoffs(source, &Default::default()).unwrap();
        assert_eq!(count, 0, "{source}\n{code}");
    }
}

#[test]
fn adjacent_var_copy_in_uses_its_existing_storage() {
    let source =
        "function f(){let $SSA_value=read();var slot=$SSA_value;return [$SSA_value,slot];}";
    let (code, count) = super::coalesce_local_handoffs(source, &Default::default()).unwrap();
    assert_eq!(count, 1, "{code}");
    assert!(!code.contains("$SSA_value"), "{code}");
    assert!(code.contains("var slot = read()"), "{code}");
}
#[test]
fn exception_observers_protect_outer_cells_but_not_private_arm_locals() {
    for source in [
        "function f(){let state;try{let snapshot=call();check();state=snapshot;consume(snapshot);}catch(e){throw e;}return state;}",
        "function f(){try{let state;let snapshot=call();check();state=snapshot;consume(state);}catch(e){}}",
        "function f(){let state;try{throw 1;}catch(e){let snapshot=call();check();state=snapshot;consume(snapshot);}return state;}",
    ] {
        let (code,count) = super::coalesce_local_handoffs(source,&Default::default()).unwrap();
        assert_eq!(count,1,"{code}");
        assert!(!code.contains("snapshot"),"{code}");
    }
    for source in [
        "function f(){let state=4;try{let snapshot=call();check();state=snapshot;}catch(e){}return state;}",
        "function f(){let state=4;try{let snapshot=call();check();state=snapshot;}catch(e){return state;}return state;}",
        "function f(){let state=4;try{let snapshot=call();check();state=snapshot;}finally{return state;}}",
    ] {
        assert_eq!(super::coalesce_local_handoffs(source,&Default::default()).unwrap().1,0,"{source}");
    }
}

#[test]
fn copy_out_shares_later_reads_until_the_next_destination_statement() {
    let source = "function f(){let state;let snapshot=[];state=snapshot;consume(snapshot);state=other;return state;}";
    let (code, count) = super::coalesce_local_handoffs(source, &Default::default()).unwrap();
    assert_eq!(count, 1, "{code}");
    assert!(!code.contains("snapshot"), "{code}");
    assert!(code.contains("consume(state)"), "{code}");
    for source in [
        "function f(){let state;let snapshot=17;state=snapshot;state=(state=9,snapshot);return state;}",
        "function f(){let state;let snapshot=17;state=snapshot;snapshot=9;return [snapshot,state];}",
        "function f(){let snapshot=17;var state=4;state=snapshot;consume(state);}",
    ] {
        assert_eq!(super::coalesce_local_handoffs(source, &Default::default()).unwrap().1, 0, "{source}");
    }
}

#[test]
fn copy_in_reuses_existing_storage_without_moving_the_getter() {
    let source = "function f(){let snapshot=obj.value;let state=snapshot;consume(snapshot);while(state){state--;}}";
    let (code, count) = super::coalesce_local_handoffs(source, &Default::default()).unwrap();
    assert_eq!(count, 1);
    assert!(code.contains("let state = obj.value;"), "{code}");
    assert!(code.contains("consume(state)"), "{code}");
    assert!(!code.contains("snapshot"), "{code}");
}

#[test]
fn copy_in_keeps_lifetimes_that_interfere_or_are_shared() {
    for source in [
        "function f(){let snapshot=obj.value;let state=snapshot;while(state){consume(snapshot);state--;}}",
        "function f(){let snapshot=obj.value;let state=snapshot;state=snapshot-1;}",
        "function f(){let snapshot=obj.value;let state=snapshot;state--;consume(snapshot);}",
        "function f(){let snapshot=obj.value;let state=snapshot;consume(()=>snapshot);state--;}",
        "function f(){let snapshot=obj.value;let state=snapshot;consume(()=>state);state--;}",
    ] {
        let (code, count) = super::coalesce_local_handoffs(source, &Default::default()).unwrap();
        assert_eq!(count, 0, "{source}");
        assert_eq!(code, source);
    }
    let source =
        "function f(){let snapshot=obj.value;let state=snapshot;consume(snapshot);state--;}";
    for name in ["snapshot", "state"] {
        let protected = [name.into()].into_iter().collect();
        assert_eq!(
            super::coalesce_local_handoffs(source, &protected)
                .unwrap()
                .1,
            0
        );
    }
}
#[test]
fn computed_commits_reuse_private_storage_only_until_the_final_rhs() {
    for source in [
        "function f(){let state=0;let $SSA_0=Number(state)+1;array[$SSA_0]=call();state=$SSA_0+1;return state;}",
        "function f(){let state=0;let $SSA_0=read();observe($SSA_0);state=other($SSA_0);return state;}",
    ] {
        let (code,count)=super::coalesce_local_handoffs(source,&Default::default()).unwrap();
        assert_eq!(count,1,"{code}");assert!(!code.contains("$SSA_0"),"{code}");
    }
    for source in [
        "function f(){let state=0;let $SSA_0=Number(state)+1;array[$SSA_0]=call();state=$SSA_0+1;return [$SSA_0,state];}",
        "function f(){let state=0;let $SSA_0=read();observe(state);state=$SSA_0+1;return state;}",
        "function f(){let state=0;try{let $SSA_0=read();call();state=$SSA_0+1;}catch(e){return state;}return state;}",
        "function f(){let state=0;function observe(){return state;}let $SSA_0=read();observe();state=$SSA_0+1;return state;}",
    ] {
        let (_,count)=super::coalesce_local_handoffs(source,&Default::default()).unwrap();
        assert_eq!(count,0,"{source}");
    }
}
