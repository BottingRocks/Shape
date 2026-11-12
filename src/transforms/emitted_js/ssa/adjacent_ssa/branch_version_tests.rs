#[test]
fn a_branch_last_use_consumes_only_its_current_stored_version() {
    for consumer in [
        "consume($CARRY_value);",
        "return $CARRY_value;",
        "throw $CARRY_value;",
        "if($CARRY_value)consume(1);",
        "let result=$CARRY_value;consume(result);",
        "target=$CARRY_value;",
        "out[index]=$CARRY_value;",
        "out.value=$CARRY_value;",
        "out[0]=$CARRY_value;",
        "consume([$CARRY_value]);",
    ] {
        let source=format!("function f(flag){{function consume(value){{}}let $CARRY_value;let out={{}},index=0,target;if(flag){{$CARRY_value=produce();{consumer}}}else{{$CARRY_value=other();consume($CARRY_value);}}}}");
        let result = super::inline_thread_adjacent_ssa(&source, &Default::default()).unwrap();
        assert!(
            !result.code.contains("$CARRY_value=produce()"),
            "{}",
            result.code
        );
    }
}
#[test]
fn later_versions_do_not_hide_live_reads_or_exception_observers() {
    for tail in [
        "consume($CARRY_value);",
        "if(other())consume($CARRY_value);",
        "while(other())consume($CARRY_value);",
        "for(let i=0;i<2;i++)consume($CARRY_value);",
        "do{consume($CARRY_value);}while(other());",
        "switch(other()){case 1:consume($CARRY_value);}",
        "label:{consume($CARRY_value);}",
        "let see=()=> $CARRY_value;consume(see);",
        "try{consume($CARRY_value);}finally{}",
        "try{throw 1;}catch(e){consume($CARRY_value);}",
    ] {
        let source=format!("function f(flag){{let $CARRY_value;if(flag){{$CARRY_value=produce();consume($CARRY_value);}}else{{$CARRY_value=other();}}{tail}}}");
        let result = super::inline_thread_adjacent_ssa(&source, &Default::default()).unwrap();
        assert!(
            result.code.contains("$CARRY_value=produce()"),
            "{}",
            result.code
        );
    }
}
#[test]
fn readonly_argument_prefixes_allow_producers_but_getters_stay_ordered() {
    let source="function f(){let out=arguments[1],index=0;let $SSA_value=produce();arguments[1][index]=$SSA_value;return out;}";
    let result = super::inline_thread_adjacent_ssa(source, &Default::default()).unwrap();
    assert_eq!(result.inlined_bindings, 1, "{}", result.code);
    for prefix in [
        "arguments[1].nested[index]",
        "object.value[index]",
        "arguments[1][coerce]",
        "arguments[1][read()]",
    ] {
        let source = format!("function f(){{let $SSA_value=produce();{prefix}=$SSA_value;}}");
        let result = super::inline_thread_adjacent_ssa(&source, &Default::default()).unwrap();
        // A computed key's coercion follows the RHS in both programs, but
        // evaluating an unknown key read/call may itself be observable.
        assert_eq!(result.inlined_bindings, 0, "{}", result.code);
    }
}

#[test]
fn keeps_a_declaration_with_a_comment_outside_its_value() {
    let result = super::inline_adjacent_ssa("function f() { let $SSA_1 = /* keep */ 3; return $SSA_1; }").unwrap();
    assert!(result.code.contains("/* keep */"), "{}", result.code);
    // A comment inside the value moves with it.
    let result = super::inline_adjacent_ssa("function f(g) { let $SSA_1 = g(/* keep */ 3); return $SSA_1; }").unwrap();
    assert_eq!(result.inlined_bindings, 1, "{}", result.code);
    assert!(result.code.contains("/* keep */"), "{}", result.code);
}
