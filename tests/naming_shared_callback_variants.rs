use shape::naming::name_threads;
fn check(body: &str, parameters: &str, expected: Option<&str>) {
    let src = format!("function bootstrap(){{let carrier=0;function $THREAD_99_1({parameters}){{{body}}}return [carrier,$THREAD_99_1];}}bootstrap();");
    let result = name_threads(&src, true).unwrap();
    let o = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_99_1")
        .unwrap();
    if let Some(role) = expected {
        assert!(o.applied, "{o:?}");
        assert_eq!(o.role_name.as_deref(), Some(role));
    } else {
        assert!(
            !o.evaluator
                .as_deref()
                .unwrap_or("")
                .starts_with("shared_callback_"),
            "{o:?}"
        );
    }
}
#[test]
fn scheduler_positive_variants() {
    for (p, b) in [
        ("callback", "setTimeout(callback,0);"),
        ("$AU", "setTimeout($AU, 0);"),
        ("work", "\"use strict\"; (setTimeout)((work), (0));"),
    ] {
        check(b, p, Some("scheduleCallbackAtZeroDelay"));
    }
}
#[test]
fn scheduler_near_misses() {
    for b in [
        "setTimeout(other,0);",
        "setTimeout(work,1);",
        "return setTimeout(work,0);",
        "setTimeout(work,0,...other);",
        "let setTimeout=other;setTimeout(work,0);",
        "work=other;setTimeout(work,0);",
        "function nested(){setTimeout(work,0);} return nested;",
        "setTimeout(()=>work(),0);",
        "setTimeout(work,0);extra();",
    ] {
        check(b, "work", None);
    }
    check("setTimeout(work,0);", "work,other", None);
}
#[test]
fn scheduler_shadowed_global() {
    let src="function bootstrap(setTimeout){let carrier=0;function $THREAD_99_1(work){setTimeout(work,0);}return [carrier,$THREAD_99_1];}bootstrap();";
    let r = name_threads(src, true).unwrap();
    assert!(r
        .report
        .outcomes
        .iter()
        .all(|o| o.evaluator.as_deref() != Some("shared_callback_schedule")));
}

const REMOVE: &str = "let i=0;for(;i<this.callbacks.length;i++){if(this.callbacks[i]===item){this.callbacks[i]=void 0;}}return this;";
const CHAIN: &str = "let i=0;let result=null;for(;i<this.callbacks.length;i++){if(this.callbacks[i]!=null){try{result=this.callbacks[i].call(void 0,item,result);}catch(e){}}}return result;";
#[test]
fn callback_list_positive_variants() {
    for (p, b, r) in [
        ("", "this.callbacks=[];", "initializeCallbackList"),
        ("", "this.callbacks=[];return this;", "clearCallbackList"),
        (
            "item",
            "this.callbacks.push(item);return this;",
            "appendCallback",
        ),
        ("item", REMOVE, "removeMatchingCallbacks"),
        ("item", CHAIN, "notifyCallbackChain"),
    ] {
        check(b, p, Some(r));
        check(
            &format!(
                "\"use strict\";{}",
                b.replace("callbacks", "__callbacks")
                    .replace("item", "$AU")
                    .replace("result", "$AV")
                    .replace("let i=", "let cursor=")
                    .replace(";i<", ";cursor<")
                    .replace(";i++", ";cursor++")
                    .replace("[i]", "[cursor]")
            ),
            &p.replace("item", "$AU"),
            Some(r),
        );
    }
}
#[test]
fn callback_list_near_misses() {
    for (p, b) in [
        ("item", "other.callbacks.push(item);return this;"),
        ("item", "this.callbacks.push(other);return this;"),
        ("item", "this.callbacks.push(item);extra();return this;"),
        ("", "this.callbacks=[other];return this;"),
        ("", "function nested(){this.callbacks=[];}return nested;"),
        ("item", "this.callbacks.push(...item);return this;"),
        ("item", "this.callbacks?.push(item);return this;"),
    ] {
        check(b, p, None);
    }
    for (a, b) in [
        ("this.callbacks[i]===item", "this.callbacks[j]===item"),
        ("this.callbacks[i]=void 0", "this.other[i]=void 0"),
        ("this.callbacks[i]=void 0", "this.callbacks[i]=null"),
        ("i++", "i--"),
        ("i<this.callbacks.length", "i<=this.callbacks.length"),
        ("this.callbacks[i]===item", "this.callbacks[i]===other"),
    ] {
        check(&REMOVE.replace(a, b), "item", None);
    }
    for (a, b) in [
        ("item,result", "other,result"),
        ("item,result", "item,other"),
        ("result=this.callbacks[i]", "other=this.callbacks[i]"),
        ("this.callbacks[i].call", "this.callbacks[j].call"),
        ("return result", "return other"),
        ("catch(e){}", "catch(e){throw e;}"),
        ("catch(e){}", "catch({value=effect()}){}"),
        ("let i=0", "const i=0"),
        ("let result=null", "const result=null"),
        ("void 0,item", "this,item"),
        ("this.callbacks[i]!=null", "this.other[i]!=null"),
    ] {
        check(&CHAIN.replace(a, b), "item", None);
    }
}
#[test]
fn callback_list_shadowed_and_cross_callable_evidence_refuses() {
    check("let i=0;for(;i<this.callbacks.length;i++){if(this.callbacks[i]===item){function donor(){this.callbacks[i]=void 0;}donor();}}return this;", "item", None);
    check("let i=0;for(;i<this.callbacks.length;i++){if(this.callbacks[i]===item){{let i=1;this.callbacks[i]=void 0;}}}return this;", "item", None);
    check(
        &CHAIN.replace(
            "let result=null;",
            "let result=null;function donor(){return result;}",
        ),
        "item",
        None,
    );
}
