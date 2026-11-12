#[test]
fn point_types_cover_nine_rematerialization_patterns() {
    let cases = [
    // Mixed-type cells cannot use the all-stores numeric proof. A call
    // cannot replace this private cell, even when it observes other state.
    "function f(input){let log=[];function touch(){log.push('call');}let value={};value=+input;touch();let $SSA_0=value|0;return [$SSA_0,$SSA_0,log];}",
    // The current successful store, rather than the declaration's type.
    "function f(input){let value={};value=+input;let $SSA_0=value+1;return [$SSA_0,$SSA_0];}",
    "function f(input){let value={};value=input;if(typeof value==='number'){let $SSA_0=value+1;return [$SSA_0,$SSA_0];}return 'other';}",
    "function f(input){let value={};value=input;let $SSA_0=typeof value==='number'&&value+1;return [$SSA_0,$SSA_0];}",
    "function f(input){let value={};value=+input;value++;let $SSA_0=value+1;return [$SSA_0,$SSA_0];}",
    "function f(input){let value={};value=+input;value^=3;let $SSA_0=value+1;return [$SSA_0,$SSA_0];}",
    "function f(input){let value={};value=+input;let out=[];for(let i=0;i<2;i++){let $SSA_0=value+1;out.push($SSA_0,$SSA_0);}return out;}",
    "function f(input){let value={};value=+input;let $SSA_0=`v${value}`;return [$SSA_0,$SSA_0];}",
    "function f(input){let value={};value=+input;let $SSA_0=(value|0,value+1);return [$SSA_0,$SSA_0];}",
];

    for source in cases {
        let (code, count) =
            super::inline_fresh_values(source, &Default::default(), false).unwrap();
        assert!(count > 0 && !code.contains("$SSA_0"), "{code}");
    }
}
