#[test]
fn aliases_cross_normal_and_abrupt_control_only_with_reaching_equality() {
    for source in [
        "function f(flag){let current=read();let $CARRY_0;try{$CARRY_0=current;use();}catch(e){throw e;}return $CARRY_0;}",
        "function f(flag){let current=read();let $CARRY_0=current;try{use();}catch(e){use($CARRY_0);}return $CARRY_0;}",
        "function f(flag){let current=0;let $CARRY_0;try{current=read();}finally{if(flag)return 9;else $CARRY_0=current;}return $CARRY_0;}",
        "function f(flag){let current=read();let $CARRY_0=current;object?.method($CARRY_0);return $CARRY_0;}",
        "function f(flag){let current=read();let $CARRY_0=current;switch(flag){case 0:use($CARRY_0);break;default:use($CARRY_0);}return $CARRY_0;}",
        "function f(flag){let current=read();let $CARRY_0=current;block:{if(flag)break block;use($CARRY_0);}return $CARRY_0;}",
        "function f(flag){let current=0;let $CARRY_0=current;do{use($CARRY_0);current++;}while($CARRY_0=current,current<2);return $CARRY_0;}",
        "function f(flag){let current=read();let $CARRY_0;switch(flag){case 0:$CARRY_0=current;break;default:$CARRY_0=current;break;}return $CARRY_0;}",
    ] {
        let (code,a,b)=super::eliminate_stable_channels(source,&Default::default(),false).unwrap();
        assert!(a+b>0 && !code.contains("$CARRY_0"),"{code}");
    }
}
#[test]
fn partial_exception_entries_and_finally_overrides_keep_snapshots() {
    for source in [
        "function f(flag){let current=0;let $CARRY_0;try{use();$CARRY_0=current;}catch(e){}return $CARRY_0;}",
        "function f(flag){let current=0;let $CARRY_0=current;try{current=read();use();}catch(e){return $CARRY_0;}return $CARRY_0;}",
        "function f(flag){let current=0;let $CARRY_0;try{use();$CARRY_0=current;}finally{use($CARRY_0);}return $CARRY_0;}",
        "function f(flag){let current=0;let $CARRY_0=current;object?.method(current=1);return $CARRY_0;}",
        "function f(flag){let current=0;let $CARRY_0=current;switch(flag){case 0:current=1;break;}return $CARRY_0;}",
        "function f(flag){let current=0;let $CARRY_0=current;block:{current=1;if(flag)break block;}return $CARRY_0;}",
        "function f(flag){let current=0;let $CARRY_0=current;do{current++;use($CARRY_0);}while($CARRY_0=current,current<2);return $CARRY_0;}",
        "function f(flag){let current=0;let $CARRY_0;switch(flag){case 0:$CARRY_0=current;break;}return $CARRY_0;}",
        "function f(flag){let current=0;let $CARRY_0=current;try{throw {x:1};}catch({[current=9]:v}){return $CARRY_0;}return $CARRY_0;}",
    ] {
        let (code,a,b)=super::eliminate_stable_channels(source,&Default::default(),false).unwrap();
        assert_eq!(a+b,0,"{code}");
    }
}
