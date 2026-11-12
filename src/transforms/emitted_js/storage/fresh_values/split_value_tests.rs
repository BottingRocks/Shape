#[test]
fn effects_stay_once_while_ten_result_forms_rematerialize() {
    for value in [
        "(effect(),base+1)",
        "flag?(left(),base+1):(right(),offset)",
        "flag&&(effect(),base+1)",
        "(target=(effect(),base+1))",
        "void effect()",
        "!(effect(),base)",
        "(left(),base)+(right(),offset)",
        "`a${(left(),base)}b${(right(),offset)}`",
        "(effect(),7)",
        "(effect(),arguments[0])",
    ] {
        let source=format!("function f(){{let flag=arguments[0];const base=read()|0;const offset=3;let target=0;let $SSA_result={value};return [$SSA_result,$SSA_result,target];}}");
        let (code, count) =
            super::inline_fresh_values(&source, &Default::default(), false).unwrap();
        assert!(count > 0 && !code.contains("$SSA_result"), "{code}");
    }
}
#[test]
fn unstable_inputs_coercions_identity_and_tdz_keep_the_cache() {
    for source in [
        "function f(){let flag=true;const base=3;let $SSA_result=flag?(flag=false,base):base;return [$SSA_result,$SSA_result];}",
        "function f(){let base={};let $SSA_result=+(effect(),base);return [$SSA_result,$SSA_result];}",
        "function f(){let base={};let $SSA_result=`a${(effect(),base)}`;return [$SSA_result,$SSA_result];}",
        "function f(){let $SSA_result=(effect(),[]);return [$SSA_result,$SSA_result];}",
        "function f(){let $SSA_result=(effect(),base);let base=3;return [$SSA_result,$SSA_result];}",
        "function f(){let base=3;let $SSA_result=(effect(),base+1);base=9;return [$SSA_result,$SSA_result];}",
        "function f(){let base=3;const obj={set x(v){base=9;}};let $SSA_result=obj.x=(effect(),base);return [$SSA_result,$SSA_result];}",
        "function f(){let $SSA_result=(effect(),arguments[0]);expose(arguments);return [$SSA_result,$SSA_result];}",
    ] {
        let (code,_)=super::inline_fresh_values(source,&Default::default(),false).unwrap();
        assert!(code.contains("let $SSA_result"),"{code}");
    }
}
