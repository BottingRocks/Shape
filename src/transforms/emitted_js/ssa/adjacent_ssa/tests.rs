use super::inline_adjacent_ssa;

#[test]
fn lexical_callee_reads_do_not_need_a_comma_wrapper() {
    let result = inline_adjacent_ssa(
        "function f(fn) { let $SSA_1 = fn; let $SSA_2 = $SSA_1(); return $SSA_2; }",
    )
    .unwrap();
    assert!(result.code.contains("(fn)()"), "{}", result.code);
    assert!(!result.code.contains("(0, fn)"));
}

#[test]
fn inlines_a_callee_without_turning_a_value_call_into_a_method_call() {
    let source =
        "function f(obj) { let $SSA_1 = obj.method; let $SSA_2 = $SSA_1(3); return $SSA_2; }";
    let result = inline_adjacent_ssa(source).unwrap();
    assert_eq!(result.inlined_bindings, 1);
    assert!(
        result.code.contains("(0, obj.method)(3)"),
        "{}",
        result.code
    );
}

#[test]
fn inlines_first_helper_argument_after_immutable_function_lookup() {
    let source = "function f() { function $VMHELPER_1(x, y) { return x + y; } let $SSA_1 = effect(); let $SSA_2 = $VMHELPER_1($SSA_1, 2); return $SSA_2; }";
    let result = inline_adjacent_ssa(source).unwrap();
    assert!(
        result.code.contains("$VMHELPER_1((effect()), 2)"),
        "{}",
        result.code
    );
}

#[test]
fn retains_first_argument_when_callee_binding_can_change() {
    let source = "function f() { function $VMHELPER_1(x) { return x; } function swap() { $VMHELPER_1 = other; return 1; } let $SSA_1 = swap(); let $SSA_2 = $VMHELPER_1($SSA_1); return $SSA_2; }";
    let result = inline_adjacent_ssa(source).unwrap();
    assert!(
        result.code.contains("let $SSA_1 = swap();"),
        "{}",
        result.code
    );
}

#[test]
fn inlines_plain_read_in_later_helper_argument() {
    let source = "function f($PARAMA) { function $VMHELPER_1(a, b) { return a + b; } let $SSA_1 = $PARAMA; let $SSA_2 = $VMHELPER_1(3, $SSA_1); return $SSA_2; }";
    let result = inline_adjacent_ssa(source).unwrap();
    assert_eq!(result.inlined_bindings, 1, "{}", result.code);
    assert!(result.code.contains("$VMHELPER_1(3, ($PARAMA))"));
}

#[test]
fn inlines_after_literal_arguments_but_retains_getters_and_tdz() {
    let effect = "function f() { function $VMHELPER_1(a, b) { return a + b; } let $SSA_1 = effect(); let $SSA_2 = $VMHELPER_1(3, $SSA_1); return $SSA_2; }";
    assert!(!inline_adjacent_ssa(effect)
        .unwrap()
        .code
        .contains("let $SSA_1 = effect()"));
    let getter = "function f($PARAMA, obj) { function $VMHELPER_1(a, b) { return a + b; } let $SSA_1 = $PARAMA; let $SSA_2 = $VMHELPER_1(obj.value, $SSA_1); return $SSA_2; }";
    assert!(inline_adjacent_ssa(getter)
        .unwrap()
        .code
        .contains("let $SSA_1 = $PARAMA"));
    let lexical = "function f($PARAMA, choice) { function $VMHELPER_1(a, b) { return a + b; } switch (choice) { case 0: let previous = 3; case 1: let $SSA_1 = $PARAMA; let $SSA_2 = $VMHELPER_1(previous, $SSA_1); } }";
    assert!(inline_adjacent_ssa(lexical)
        .unwrap()
        .code
        .contains("let $SSA_1 = $PARAMA"));
}

#[test]
fn inlines_adjacent_return_throw_and_slot_handoff() {
    let source = "function a() { let $SSA_1 = f(); return $SSA_1; }\nfunction b() { let $SSA_2 = g(); throw $SSA_2; }\nfunction c() { let $KEY_0; let $SSA_3 = h(); $KEY_0 = $SSA_3; return $KEY_0; }\n";
    let result = inline_adjacent_ssa(source).unwrap();
    assert_eq!(result.inlined_bindings, 3);
    assert!(result.code.contains("return (f());"));
    assert!(result.code.contains("throw (g());"));
    assert!(result.code.contains("$KEY_0 = (h());"));
}

#[test]
fn preserves_intervening_effect_and_captured_binding() {
    let source = "function a() { let $SSA_1 = f(); g(); return $SSA_1; }\nfunction b() { let $SSA_2 = () => $SSA_2; return $SSA_2; }\n";
    let result = inline_adjacent_ssa(source).unwrap();
    assert_eq!(result.inlined_bindings, 0);
    assert_eq!(result.code, source);
}

#[test]
fn direct_eval_disables_rewrite() {
    let source = "function a() { let $SSA_1 = eval('typeof $SSA_1'); return $SSA_1; }\n";
    let result = inline_adjacent_ssa(source).unwrap();
    assert_eq!(result.inlined_bindings, 0);
    assert_eq!(result.code, source);
}

#[test]
fn retains_handoff_to_unresolved_global_target() {
    let source = "function f() { let $SSA_1 = effect(); globalTarget = $SSA_1; }\n";
    let result = inline_adjacent_ssa(source).unwrap();
    assert_eq!(result.inlined_bindings, 0);
    assert_eq!(result.code, source);
}

#[test]
fn inlines_effectful_value_at_first_evaluation_of_next_initializer() {
    let source = "function f() { let $SSA_1 = call(); let $SSA_2 = $SSA_1[0]; return $SSA_2; }";
    let result = inline_adjacent_ssa(source).unwrap();
    assert_eq!(result.inlined_bindings, 1);
    assert!(
        result.code.contains("let $SSA_2 = (call())[0]"),
        "{}",
        result.code
    );
}

#[test]
fn retains_effectful_value_when_next_initializer_evaluates_something_first() {
    let source =
        "function f() { let $SSA_1 = call(); let $SSA_2 = other() + $SSA_1; return $SSA_2; }";
    let result = inline_adjacent_ssa(source).unwrap();
    assert_eq!(result.inlined_bindings, 1);
    assert!(result.code.contains("let $SSA_1 = call()"));
}

#[test]
fn inlines_next_door_branch_test_and_stable_return_operand() {
    let source = "function f() { const $PARAMA = 2; let $SSA_1 = effect(); if ($SSA_1) { use(); } let $SSA_2 = read(); return ($PARAMA | $SSA_2); }";
    let result = inline_adjacent_ssa(source).unwrap();
    assert_eq!(result.inlined_bindings, 2, "{}", result.code);
    assert!(result.code.contains("if ((effect()))"), "{}", result.code);
    assert!(
        result.code.contains("return ($PARAMA | (read()))"),
        "{}",
        result.code
    );
}

#[test]
fn preserves_extracted_method_call_receiver_and_short_circuit() {
    let source = "function f(obj) { let $SSA_1 = obj.method; return $SSA_1(); } function g(flag) { let $SSA_2 = effect(); if (flag && $SSA_2) { use(); } }";
    let result = inline_adjacent_ssa(source).unwrap();
    assert_eq!(result.inlined_bindings, 1);
    assert!(
        result.code.contains("return (0, obj.method)();"),
        "{}",
        result.code
    );
    assert!(
        result.code.contains("let $SSA_2 = effect();"),
        "{}",
        result.code
    );
}

#[test]
fn inlines_value_handed_to_branch_carry() {
    let source = "function f(flag, value) { let $CARRY_1; if (flag) { let $SSA_1 = value; $CARRY_1 = $SSA_1; } else { $CARRY_1 = 0; } if ($CARRY_1) return 1; }";
    let result = inline_adjacent_ssa(source).unwrap();
    assert_eq!(result.inlined_bindings, 1, "{}", result.code);
    assert!(
        result.code.contains("$CARRY_1 = (value)"),
        "{}",
        result.code
    );
}
#[test]
fn comma_stores_can_cross_separate_private_inert_stores() {
    let source = "function f(){let a=0;let $SSA_0;return ($SSA_0=make(),a=2,$SSA_0+a);}";
    let result = super::inline_thread_adjacent_ssa(source, &Default::default()).unwrap();
    assert_eq!(result.inlined_bindings, 1, "{}", result.code);
    assert!(!result.code.contains("$SSA_0=make()"));
}
