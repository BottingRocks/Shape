use shape::naming::name_threads;

const BODY: &str = "let alias=listener;setTimeout(()=>alias(error),0);";
const PREFIX: &str = "error=caught;latched=caught;failed=true;running=false;queue.length=0;";

fn source(body: &str, prefix: &str, registration: &str) -> String {
    format!(
        "function bootstrap(){{let listeners=[],queue=[],latched,failed=false,running=true;
         function drain(){{let error;try{{work();}}catch(caught){{{prefix}
         function $THREAD_99_1(listener){{{body}}}{registration}}}}}
         return [listeners,queue,drain];}}bootstrap();"
    )
}

fn check(src: &str, positive: bool) {
    let result = name_threads(src, true).unwrap();
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_99_1")
        .unwrap();
    if positive {
        assert!(outcome.applied, "{outcome:?}");
        assert_eq!(outcome.role_name.as_deref(), Some("scheduleErrorListener"));
        assert_eq!(
            outcome.evaluator.as_deref(),
            Some("shared_caught_error_listener")
        );
    } else {
        assert_ne!(
            outcome.evaluator.as_deref(),
            Some("shared_caught_error_listener"),
            "{outcome:?}"
        );
    }
}

#[test]
fn caught_error_listener_positive_variants() {
    for prefix in ["error=caught;", PREFIX] {
        for body in [
            BODY,
            "const alias=listener;(setTimeout)(()=> (alias)((error)),(0));",
            "var alias=listener;setTimeout(()=>alias(error),0);",
        ] {
            check(
                &source(body, prefix, "listeners.forEach($THREAD_99_1);"),
                true,
            );
        }
    }
    let renamed = source(BODY, PREFIX, "listeners.forEach($THREAD_99_1);")
        .replace("listener", "subscriber")
        .replace("caught", "failure")
        .replace("error", "heldFailure")
        .replace("alias", "retained");
    check(&renamed, true);
}

#[test]
fn caught_error_listener_body_near_misses() {
    for body in [
        "let alias=listener;setTimeout(()=>alias(error),1);",
        "let alias=listener;return setTimeout(()=>alias(error),0);",
        "let alias=listener;setTimeout(()=>alias(other),0);",
        "let alias=other;setTimeout(()=>alias(error),0);",
        "let alias=listener;alias=other;setTimeout(()=>alias(error),0);",
        "listener=other;let alias=listener;setTimeout(()=>alias(error),0);",
        "let alias=listener;setTimeout(async()=>alias(error),0);",
        "let alias=listener;setTimeout((error)=>alias(error),0);",
        "let alias=listener;setTimeout(()=>alias(error,other),0);",
        "let alias=listener;setTimeout(()=>alias.call(null,error),0);",
        "let alias=listener;setTimeout(()=>alias?.(error),0);",
        "let alias=listener;setTimeout?.(()=>alias(error),0);",
        "let alias=listener;setTimeout(()=>alias(error),0,...extra);",
        "let alias=listener;setTimeout(()=>alias(...error),0);",
        "let alias=listener;setTimeout(()=>{alias(error);extra();},0);",
        "let alias=listener;function donor(){setTimeout(()=>alias(error),0);}",
        "let alias=listener;setTimeout(()=>alias(error),0);extra();",
        "let alias=listener;setTimeout(()=>{let error=other;alias(error);},0);",
    ] {
        check(
            &source(body, PREFIX, "listeners.forEach($THREAD_99_1);"),
            false,
        );
    }
    let original = source(BODY, PREFIX, "listeners.forEach($THREAD_99_1);");
    for (from, to) in [
        ("bootstrap()", "bootstrap(setTimeout)"),
        ("function $THREAD_99_1", "async function $THREAD_99_1"),
        ("function $THREAD_99_1", "function* $THREAD_99_1"),
        ("(listener)", "(listener=other)"),
        ("(listener)", "(listener,...extra)"),
        ("(listener)", "(listener,other)"),
        (
            "let alias=listener;",
            "\"custom directive\";let alias=listener;",
        ),
    ] {
        check(&original.replacen(from, to, 1), false);
    }
}

#[test]
fn caught_error_listener_rejects_wrong_origin_and_registration() {
    for prefix in [
        "error=other;",
        "latched=caught;",
        "if(flag){error=caught;}",
        "function donor(){error=caught;}",
        "caught=other;error=caught;",
        "error=caught;error=other;",
        "error=caught;extra();",
        "error=caught;latched=other;failed=true;running=false;queue.length=0;",
        "error=caught;latched=caught;failed=false;running=false;queue.length=0;",
        "error=caught;latched=caught;failed=true;running=true;queue.length=0;",
        "error=caught;latched=caught;failed=true;running=false;queue.length=1;",
        "error=caught;latched=caught;failed=true;failed=false;queue.length=0;",
    ] {
        check(
            &source(BODY, prefix, "listeners.forEach($THREAD_99_1);"),
            false,
        );
    }
    for registration in [
        "listeners.forEach(other);",
        "listeners.map($THREAD_99_1);",
        "listeners.forEach($THREAD_99_1,other);",
        "listeners.forEach(...$THREAD_99_1);",
        "listeners?.forEach($THREAD_99_1);",
        "if(flag){listeners.forEach($THREAD_99_1);}",
        "function donor(){listeners.forEach($THREAD_99_1);}",
        "extra();listeners.forEach($THREAD_99_1);",
        "listeners.forEach($THREAD_99_1);$THREAD_99_1(other);",
        "listeners.forEach($THREAD_99_1);error=other;",
        "$THREAD_99_1=other;listeners.forEach($THREAD_99_1);",
    ] {
        check(&source(BODY, PREFIX, registration), false);
    }
    let original = source(BODY, PREFIX, "listeners.forEach($THREAD_99_1);");
    for (from, to) in [
        ("let error;", "let error=other;"),
        ("let error;", ""),
        ("function drain(){let error;", "let error;function drain(){"),
        ("catch(caught)", "catch({caught})"),
        ("catch(caught){", "catch(caught){function donor(){"),
    ] {
        // The nested donor case needs a balancing brace before the catch ends.
        let mutated = if to.ends_with("donor(){") {
            original.replace(from, to).replace(
                "listeners.forEach($THREAD_99_1);",
                "listeners.forEach($THREAD_99_1);}",
            )
        } else {
            original.replace(from, to)
        };
        check(&mutated, false);
    }
}

#[test]
fn caught_error_listener_rejects_sibling_and_nested_donors() {
    check(
        "function bootstrap(){let listeners=[];
         function drain(){let error;try{work();}catch(caught){error=caught;}
         function $THREAD_99_1(listener){let alias=listener;setTimeout(()=>alias(error),0);}
         listeners.forEach($THREAD_99_1);}return [listeners,drain];}bootstrap();",
        false,
    );
    check(
        "function bootstrap(){let listeners=[];
         function drain(){let error;try{work();}catch(caught){error=caught;
         function $THREAD_99_1(listener){let alias=listener;setTimeout(()=>alias(error),0);}
         function donor(){listeners.forEach($THREAD_99_1);}}}return [listeners,drain];}bootstrap();",
        false,
    );
}

#[test]
fn existing_direct_scheduler_role_is_preserved() {
    let src = source(
        "setTimeout(listener,0);",
        PREFIX,
        "listeners.forEach($THREAD_99_1);",
    );
    let result = name_threads(&src, true).unwrap();
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_99_1")
        .unwrap();
    assert!(outcome.applied, "{outcome:?}");
    assert_eq!(
        outcome.role_name.as_deref(),
        Some("scheduleCallbackAtZeroDelay")
    );
    assert_eq!(
        outcome.evaluator.as_deref(),
        Some("shared_callback_schedule")
    );
}
