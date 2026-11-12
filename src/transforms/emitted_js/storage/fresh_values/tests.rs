#[test]
fn var_inputs_include_initializer_writes_and_annex_b_names() {
    for source in [
        "function f(){var n=read()|0;let result=n+1;effect();return [n,result];}",
        "function f(){var n=read();let result=[n];effect();consume(result);n=4;}",
    ] {
        let (code, count) =
            super::inline_fresh_values(source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{code}");
    }
    for source in [
        "function f(){let result=[n];var n=4;return result;}",
        "function f(){var n=3;let result=[n];var n=4;return result;}",
        "function f(){var n=3;let result=[n];{function n(){}}return result;}",
        "function f(){var n=3;let result=[n];function change(){n=4}change();return result;}",
    ] {
        assert_eq!(
            super::inline_fresh_values(source, &Default::default(), false)
                .unwrap()
                .1,
            0,
            "{source}"
        );
    }
}

#[test]
fn readonly_argument_computations_reuse_the_existing_non_escape_proof() {
    for source in [
        "function f(){let result=arguments[0]==null;effect();return [result,result];}",
        "function f(){let result=arguments[0]===void 0?[]:arguments[0];effect();return result;}",
    ] {
        let (code,count)=super::inline_fresh_values(source,&Default::default(),false).unwrap();
        assert_eq!(count,1,"{code}");
    }
    for source in [
        "function f(){let result=arguments[0]==null;arguments[0]=4;return result;}",
        "function f(n){let result=arguments[0]==null;n=4;return result;}",
        "function f(){let result=arguments[0]==null;expose(arguments);return result;}",
        "function f(){let result=arguments[0]+1;effect();return [result,result];}",
        "function f(){let arguments=[3];let result=arguments[0]==null;effect();return result;}",
    ] {
        let (code, _) = super::inline_fresh_values(source, &Default::default(), false).unwrap();
        assert!(code.contains("let result="), "{code}");
    }
}

#[test]
fn bounded_identity_free_values_can_have_multiple_dynamic_reads() {
    for source in [
        "function f(){const n=read()|0;let result=n+1;effect();return [result,result];}",
        "function f(){const n=read();let result=n==null;effect();return [result,result];}",
    ] {
        let (code, count) =
            super::inline_fresh_values(source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(!code.contains("let result"), "{code}");
    }
    for source in [
        "function f(){let result=[];effect();return [result,result];}",
        "function f(){const n=read();let result=n+1;effect();return [result,result];}",
        "function f(){let result=/x/;effect();return [result,result];}",
    ] {
        assert_eq!(
            super::inline_fresh_values(source, &Default::default(), false)
                .unwrap()
                .1,
            0,
            "{source}"
        );
    }
}

#[test]
fn mutable_lexical_inputs_require_the_same_version_at_every_read() {
    for source in [
        "function f(){let n=read()|0; n=3;let result=n+1;effect();consume(result);n=4;}",
        "function f(){let n=read();let result=[n];effect();consume(result);n=4;}",
    ] {
        let (code, count) =
            super::inline_fresh_values(source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(!code.contains("let result"), "{code}");
    }
    for source in [
        "function f(){let n=read()|0;let result=n+1;consume(result);n=4;consume(result);}",
        "function f(){let n=read()|0;let result=n+1;function effect(){n=4;}effect();consume(result);}",
        "function f(){var n=3;let result=[n];var n=4;return result;}",
    ] {
        assert_eq!(super::inline_fresh_values(source, &Default::default(), false).unwrap().1, 0, "{source}");
    }
}

#[test]
fn immutable_primitive_computations_cross_effects_but_keep_coercion_barriers() {
    for source in [
        "function f(){const value=read()|0;let result=value+1;effect();return result;}",
        "function f(){const value=read();let result=!value;effect();return result;}",
        "function f(){const value=read();let result=value==null;effect();return result;}",
        "function f(){const value=read();let result=value?true:false;effect();return result;}",
    ] {
        let (code, count) =
            super::inline_fresh_values(source, &Default::default(), false).unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(!code.contains("let result"), "{code}");
    }
    for source in [
        "function f(){const value=read();let result=value+1;effect();return result;}",
        "function f(){const value=1n;let result=value+1;effect();return result;}",
        "function f(){let value=1;let result=value+1;value=3;return result;}",
        "function f(){const value=read();let result=value instanceof Other;effect();return result;}",
        "function f(){const value=read()|0;let result=value+1;effect();return {result};}",
    ] {
        assert_eq!(super::inline_fresh_values(source,&Default::default(),false).unwrap().1,0,"{source}");
    }
}
#[test]
fn checked_slot_captures_release_only_generated_private_temporaries() {
    let source = "function f(){let $SSA_0=[];function $THREAD_1_2(){}effect();return $SSA_0;}";
    assert_eq!(
        super::inline_fresh_values(source, &Default::default(), false)
            .unwrap()
            .1,
        0
    );
    let (code, count) = super::inline_fresh_values(source, &Default::default(), true).unwrap();
    assert_eq!(count, 1);
    assert!(!code.contains("$SSA_0"), "{code}");
    let ordinary = source.replace("$SSA_0", "seen");
    assert_eq!(
        super::inline_fresh_values(&ordinary, &Default::default(), true)
            .unwrap()
            .1,
        0
    );
    let shared = "function f(){const $KEY_7=3;let $SSA_0=[$KEY_7];function $THREAD_1_2(){}effect();return $SSA_0;}";
    let protected = ["$KEY_7".into()].into_iter().collect();
    assert_eq!(
        super::inline_fresh_values(shared, &protected, true)
            .unwrap()
            .1,
        0
    );
}
#[test]
fn single_use_allocations_cross_effects_but_not_loop_instances() {
    let source = "function f(){const value=3;let data={value};effect();return data;}";
    let (code, count) = super::inline_fresh_values(source, &Default::default(), false).unwrap();
    assert_eq!(count, 1);
    assert!(!code.contains("let data"), "{code}");
    assert!(code.contains("return ({value})"), "{code}");
    for source in [
        "function f(){let data=[];for(let i=0;i<2;i++){consume(data);}}",
        "function f(){let data=[];return ()=>data;}",
        "function f(){let data=[];effect();return delete data;}",
        "function f(){let value=1;let data=[value];value=2;return data;}",
        "function f(){const value=1;let data=[value];{const value=2;return data;}}",
        "function f(){let data={read(){return value;}};effect();return data;}",
        "function f(){let data=[];function $THREAD_1_2(){}effect();return data;}",
        "function f(){let data=[];eval('data.push(1)');return data;}",
    ] {
        assert_eq!(
            super::inline_fresh_values(source, &Default::default(), false).unwrap(),
            (source.into(), 0)
        );
    }
}

#[test]
fn shared_inputs_and_output_cells_keep_their_storage() {
    for (source, name) in [
        ("function f(){let data=[];effect();return data;}", "data"),
        (
            "function f(){const value=3;let data=[value];effect();return data;}",
            "value",
        ),
    ] {
        let protected = [name.into()].into_iter().collect();
        assert_eq!(
            super::inline_fresh_values(source, &protected, false)
                .unwrap()
                .1,
            0
        );
    }
}
#[test]
fn mutually_dependent_numeric_stores_keep_their_primitive_category() {
    let source = "function f(n){let a=0;let b=0;while(n--){a=b;b=a+1;}let $SSA_0=a+1;observe();return [$SSA_0,$SSA_0];}";
    let (code, count) = super::inline_fresh_values(source, &Default::default(), false).unwrap();
    assert!(count > 0 && !code.contains("$SSA_0"), "{code}");
    // An unproved seed or store invalidates every dependent cell.
    for source in [
        "function f(n,input){let a=input;let b=0;while(n--){a=b;b=a+1;}let $SSA_0=a+1;observe();return [$SSA_0,$SSA_0];}",
        "function f(n){let a=0;let b=0;while(n--){a=b;b=a+1;}function change(){a={valueOf(){return 3;}};}let $SSA_0=a+1;change();return [$SSA_0,$SSA_0];}",
        "function f(n){let a=0n;let b=0n;while(n--){a=b;b=a+1n;}let $SSA_0=a+1n;observe();return [$SSA_0,$SSA_0];}",
    ] {
        let (code,_) = super::inline_fresh_values(source,&Default::default(),false).unwrap();
        assert!(code.contains("let $SSA_0"),"{code}");
    }
}
#[test]
fn string_accumulators_rematerialize_only_with_unchanged_private_inputs() {
    let source="function f(n){let a='';while(n--){a=a+'x';}let $SSA_0=a+'!';observe();return [$SSA_0,$SSA_0];}";
    let (code, _) = super::inline_fresh_values(source, &Default::default(), false).unwrap();
    assert!(!code.contains("$SSA_0"), "{code}");
    for source in [
        "function f(n){let a='';while(n--){a=a+'x';}let $SSA_0=a+'!';a='changed';return [$SSA_0,$SSA_0];}",
        "function f(input){let a='';a=input;let $SSA_0=a+'!';observe();return [$SSA_0,$SSA_0];}",
        "function f(){let a='';let $SSA_0=a+'!';function change(){a='changed';}change();return [$SSA_0,$SSA_0];}",
    ] {
        let (code,_) = super::inline_fresh_values(source,&Default::default(),false).unwrap();
        assert!(code.contains("let $SSA_0"),"{code}");
    }
}
#[test]
fn rematerialization_uses_output_cost_instead_of_a_reference_count_limit() {
    let name = "$SSA_a_deliberately_long_snapshot_name";
    let values = std::iter::repeat_n(name, 24).collect::<Vec<_>>().join(",");
    let source = format!(
        "function f(input){{let n=Number(input);let {name}=n+1;observe();return [{values}];}}"
    );
    let (code, count) =
        super::inline_fresh_values(&source, &Default::default(), false).unwrap();
    assert_eq!(count, 1, "{code}");
    assert!(!code.contains(name), "{code}");
    assert!(code.len() < source.len());
    // Identity and the output growth budget still apply at high fanout.
    let allocation = source.replace("n+1", "[]");
    assert_eq!(
        super::inline_fresh_values(&allocation, &Default::default(), false)
            .unwrap()
            .1,
        0
    );
    let expensive = source.replace(
        "n+1",
        &std::iter::repeat_n("n", 80).collect::<Vec<_>>().join("+"),
    );
    assert_eq!(
        super::inline_fresh_values(&expensive, &Default::default(), false)
            .unwrap()
            .1,
        0
    );
}
#[test]
fn while_test_aliases_and_computations_use_one_reaching_version() {
    for source in [
        "function f(){let n=0;let $SSA_0;while($SSA_0=n,$SSA_0<3){observe($SSA_0);n=$SSA_0+1;}}",
        "function f(){let n=3;let $SSA_0;while($SSA_0=n-1,$SSA_0>=0){observe($SSA_0);n=$SSA_0;}}",
    ] {
        let (code,count)=super::inline_fresh_values(source,&Default::default(),false).unwrap();
        assert!(count>0,"{code}");assert!(!code.contains("$SSA_0="),"{code}");
    }
    for source in [
        "function f(){let n=3;let $SSA_0;while($SSA_0=n-1,$SSA_0>=0){n=0;observe($SSA_0);}}",
        "function f(){let n=3;let $SSA_0;while($SSA_0=n-1,$SSA_0>=0){n=$SSA_0;}return $SSA_0;}",
        "function f(){let n=3;let $SSA_0;do{observe($SSA_0);}while($SSA_0=n-1,$SSA_0>=0);}",
        "function f(){let n=3;let $SSA_0;while(n--,($SSA_0=n-1)){observe($SSA_0);}}",
    ] {
        assert_eq!(
            super::inline_fresh_values(source, &Default::default(), false)
                .unwrap()
                .1,
            0,
            "{source}"
        );
    }
}
#[test]
fn invariant_scalar_and_argument_expressions_cross_nested_iterations() {
    for source in [
        "function f(v){let base=Number(v);let $SSA_0=base+1;for(let i=0;i<3;i++){consume($SSA_0);}}",
        "function f(){let $SSA_0=arguments[0]===null;for(let i=0;i<3;i++){consume($SSA_0);}}",
    ] {
        let (code,count)=super::inline_fresh_values(source,&Default::default(),false).unwrap();
        assert_eq!(count,1,"{code}");assert!(!code.contains("$SSA_0"),"{code}");
    }
    for source in [
        "function f(){let base=1;let $SSA_0=base+1;for(let i=0;i<3;i++){consume($SSA_0);base++;}}",
        "function f(){let base=1;let $SSA_0=base+1;for(let i=0;i<3;i++){for(let j=0;j<2;j++){consume($SSA_0);}base++;}}",
        "function f(){let base=1;let $SSA_0=base+1;for(;base<3;base++){consume($SSA_0);}}",
        "function f(){let $SSA_0=[];for(let i=0;i<3;i++){consume($SSA_0);}}",
    ] {
        let (code,_)=super::inline_fresh_values(source,&Default::default(),false).unwrap();
        assert!(code.contains("let $SSA_0"),"{code}");
    }
}
