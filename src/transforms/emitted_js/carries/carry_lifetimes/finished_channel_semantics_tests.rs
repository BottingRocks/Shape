#[test]
fn path_assignment_proof_preserves_undefined_old_reads_and_short_circuiting() {
    let mut fixtures = Vec::new();
    for branch in [
        "if(flag)$CARRY_0=state+1;else $CARRY_0=state+2;",
        "if(flag){$CARRY_0=3;}else{$CARRY_0=4;}",
        "if(flag)$CARRY_0=3;",
        "$CARRY_0=flag?state+1:state+2;",
        "flag?($CARRY_0=3):($CARRY_0=4);",
        "flag&&($CARRY_0=3);",
        "flag||($CARRY_0=3);",
        "if(flag){$CARRY_0=3;state;}else{$CARRY_0=4;}",
        "if(flag){state;$CARRY_0=3;}else{state;$CARRY_0=4;}",
        "let observed=$CARRY_0;if(flag)$CARRY_0=3;else $CARRY_0=4;",
        "$CARRY_0=state+1;$CARRY_0=$CARRY_0+2;",
        "if(flag){$CARRY_0=3;if(flag)$CARRY_0=4;}else{$CARRY_0=5;}",
    ] {
        for rhs in ["$CARRY_0+1", "[$CARRY_0,state]", "[$CARRY_0,$CARRY_0]"] {
            let source=format!("function f(flag){{let state=9;let $CARRY_0;{branch}state={rhs};return [state,state===state];}}");
            let (code, _) =
                super::finish_private_channels(&source, &Default::default()).unwrap();
            fixtures.push((source, code));
        }
    }
    let cases = serde_json::to_string(&fixtures).unwrap();
    let script=format!("for(const [before,after] of {cases}){{const a=eval('('+before+')'),b=eval('('+after+')');for(const flag of [false,true]){{const x=JSON.stringify(a(flag)),y=JSON.stringify(b(flag));if(x!==y)throw Error(before+'\\n'+after+'\\n'+x+' != '+y);}}}}");
    let result = std::process::Command::new("node")
        .arg("-e")
        .arg(script)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
