#[test]
fn first_use_control_and_declaration_consumers_are_unconditional() {
    for consumer in [
        "switch($SSA_0){case 1:return 3;default:return 4;}",
        "for(let value=$SSA_0;value<3;value++)consume(value);",
        "for(let key in $SSA_0)consume(key);",
        "for(let value of $SSA_0)consume(value);",
        "let [value]=$SSA_0;return value;",
        "let value=$SSA_0,other=next();return [value,other];",
    ] {
        let source = format!("function f(){{let $SSA_0=produce();{consumer}}}");
        let result = super::inline_thread_adjacent_ssa(&source, &Default::default()).unwrap();
        assert_eq!(result.inlined_bindings, 1, "{}", result.code);
    }
}
#[test]
fn loop_binding_shadowing_and_repeated_inputs_retain_the_snapshot() {
    for consumer in [
        "for(let source=$SSA_0;source<4;source++)consume(source);",
        "for(let source in $SSA_0)consume(source);",
        "for(let source of $SSA_0)consume(source);",
        "while($SSA_0)work();",
        "for(;$SSA_0;)work();",
        "do work();while($SSA_0);",
        "let other=effect(),value=$SSA_0;return value;",
    ] {
        let source =
            format!("function f(){{let source=produce();let $SSA_0=source;{consumer}}}");
        let result = super::inline_thread_adjacent_ssa(&source, &Default::default()).unwrap();
        // Another independent producer may be removed; this snapshot is
        // specifically protected from the new shadow/repeat environment.
        assert!(result.code.contains("$SSA_0"), "{}", result.code);
    }
}
#[test]
fn comma_snapshots_keep_getvalue_for_calls_tags_and_typeof() {
    for consumer in ["$SSA_0()", "$SSA_0`text`", "typeof $SSA_0"] {
        let source =
            format!("function f(){{let $SSA_0;return ($SSA_0=object.method,{consumer});}}");
        let result = super::inline_thread_adjacent_ssa(&source, &Default::default()).unwrap();
        assert_eq!(result.inlined_bindings, 1, "{}", result.code);
        assert!(
            result.code.contains("(0, (object.method))"),
            "{}",
            result.code
        );
    }
}
