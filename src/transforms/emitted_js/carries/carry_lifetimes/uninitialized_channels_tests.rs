#[test]
fn private_multistore_channels_share_only_a_noninterfering_destination() {
    for source in [
        "function f(flag){let state=9;let $CARRY_0;if(flag)$CARRY_0=state+1;else $CARRY_0=state+2;state=$CARRY_0+1;return state;}",
        "function f(flag){let state=9;let $CARRY_0;if(flag){$CARRY_0=left();}else{$CARRY_0=right();}consume($CARRY_0);state=$CARRY_0+1;return state;}",
        "function f(flag){let state=9;let $CARRY_0;if(flag)$CARRY_0=3;state=$CARRY_0+1;return state;}",
        "function f(flag){let state=9;let $CARRY_0,$CARRY_1;if(flag){$CARRY_0=3;$CARRY_1=7;}else{$CARRY_0=2;$CARRY_1=9;}state=$CARRY_0+$CARRY_1;return state;}",
    ] {
        let (code,count)=super::finish_private_channels(source,&Default::default()).unwrap();
        assert_eq!(count,1,"{code}");
        assert!(!code.contains("$CARRY_0"),"{code}");
        assert_eq!(code.contains("state = void 0;"),source.contains("if(flag)$CARRY_0=3;state"),"{code}");
    }
    for source in [
        "function f(flag){let state=9;let $CARRY_0;if(flag)$CARRY_0=state+1;state=$CARRY_0+1;return state;}",
        "function f(flag){let state=9;let $CARRY_0;if(flag)$CARRY_0=function(){};state=$CARRY_0;return state.name;}",
        "function f(flag){let state=9;let $CARRY_0;if(flag)$CARRY_0=3;observe(state);state=$CARRY_0+1;return state;}",
        "function f(flag){let state=9;let $CARRY_0;try{if(flag)$CARRY_0=left();state=$CARRY_0+1;}catch(e){return state;}return state;}",
        "function f(flag){let state=9;var $CARRY_0;if(flag)$CARRY_0=3;state=$CARRY_0+1;return state;}",
    ] {
        let (code,count)=super::finish_private_channels(source,&Default::default()).unwrap();
        assert_eq!(count,0,"{code}");
    }
}
