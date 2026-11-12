#[test]
fn normal_continuations_keep_types_without_exporting_them_to_abrupt_finalizers() {
    for source in [
        "function f(input){let value={};try{value=+input;}catch(e){value=4;}let $SSA_0=value+1;return [$SSA_0,$SSA_0];}",
        "function f(input){let value={};try{value=+input;}finally{observe();}let $SSA_0=value+1;return [$SSA_0,$SSA_0];}",
    ] {
        let (code,count)=super::inline_fresh_values(source,&Default::default(),false).unwrap();
        assert!(count>0&&!code.contains("$SSA_0"),"{code}");
    }
    for source in [
        "function f(input){let value={};try{value=+input;}finally{let $SSA_0=value+1;observe($SSA_0,$SSA_0);}}",
        "function f(input){let value={};try{value=+input;}catch(e){}let $SSA_0=value+1;return [$SSA_0,$SSA_0];}",
    ] {
        let (code,_)=super::inline_fresh_values(source,&Default::default(),false).unwrap();
        assert!(code.contains("let $SSA_0"),"{code}");
    }
}
