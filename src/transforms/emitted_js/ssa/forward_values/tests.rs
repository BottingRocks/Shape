use super::forward_values;

fn forward(source: &str) -> (String, usize) {
    let result = forward_values(source).unwrap();
    (result.code, result.forwarded)
}

#[test]
fn keeps_a_handoff_slot_that_would_forward_too() {
    let source = "function f(o, g) {\n  let $CARRY_1 = o[\"get\"];\n  let $CARRY_2 = $CARRY_1;\n  if (typeof $CARRY_1 === \"function\" && g($CARRY_2)) {\n    return 1;\n  }\n  return 0;\n}";
    let (code, _) = forward(source);
    let (free, _) = crate::tracing::scope::free_and_bound_names(&code).unwrap();
    assert!(!free.iter().any(|name| name.starts_with("$CARRY_")), "{code}");
}

#[test]
fn moves_past_reads_of_a_local_no_call_can_write() {
    let source = "function f(k, a, n) {\n  let $LOOP_1 = 0;\n  for (let i = 0; i < n; i++) {\n    $LOOP_1 = 0 | ($LOOP_1 << 5) - $LOOP_1 + k(a[i]);\n  }\n  let $JOIN_1;\n  if (n) {\n    let $SSA_1 = k(a[0]);\n    $JOIN_1 = ($LOOP_1 << 5) - $LOOP_1 + ($SSA_1 << 16) | 0;\n  } else {\n    $JOIN_1 = $LOOP_1;\n  }\n  return $JOIN_1;\n}";
    let (code, _) = forward(source);
    assert!(code.contains("$JOIN_1 = ($LOOP_1 << 5) - $LOOP_1 + ((k(a[0])) << 16) | 0;"), "{code}");
    // a closure writes the local: the call could change it
    let (code, _) = forward(&source.replace("  return $JOIN_1;", "  return () => $LOOP_1 = 1;"));
    assert!(code.contains("let $SSA_1"), "{code}");
    // operands are primitives: an untyped local moves the same way
    let (code, _) = forward(&source.replace("let $LOOP_1 = 0;", "let $LOOP_1 = a;"));
    assert!(!code.contains("let $SSA_1"), "{code}");
}

#[test]
fn substitutes_stable_values_at_every_read() {
    let (code, n) = forward(
        "function f(a) {\n  let $SSA_1 = 5;\n  let $SSA_2 = a;\n  let $SSA_3 = $SSA_2;\n  return [$SSA_1, $SSA_1.toFixed, $SSA_3, $SSA_3];\n}",
    );
    assert_eq!(n, 3, "{code}");
    assert!(code.contains("return [5, (5).toFixed, a, a];"), "{code}");
}

#[test]
fn keeps_slots_arguments_aliased_params_and_closure_reads() {
    for source in [
        // a VM slot can change
        "function f() {\n  let $SSA_1 = $KEY_1;\n  g();\n  return [$SSA_1, $SSA_1];\n}",
        // a sloppy parameter aliases `arguments`
        "function f(a) {\n  arguments[0] = 2;\n  let $SSA_1 = a;\n  return [$SSA_1, $SSA_1];\n}",
        // a reassigned local
        "function f(a) {\n  let $SSA_1 = a;\n  a = 3;\n  return [$SSA_1, $SSA_1];\n}",
        // a closure may run before the binding initializes
        "function f() {\n  let $SSA_1 = 1;\n  return () => [$SSA_1, $SSA_1];\n}",
        // a later `var` initializer changes the value
        "function f() {\n  let $SSA_1 = v;\n  var v = 1;\n  return [$SSA_1, $SSA_1];\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 0, "{source} -> {code}");
    }
}

#[test]
fn moves_a_single_use_value_into_the_next_statement() {
    let (code, n) = forward(
        "function h() { sink(); }\nfunction f(o) {\n  let $SSA_1 = o.p;\n  let $SSA_2 = h($SSA_1);\n  var $KEY_1 = 3;\n  return $SSA_2;\n}",
    );
    assert_eq!(n, 2, "{code}");
    assert!(code.contains("var $KEY_1 = 3;\n  return h(o.p);"), "{code}");
}

#[test]
fn keeps_this_for_a_member_callee_and_parenthesizes_operators() {
    let (code, n) = forward(
        "function f(o, x) {\n  let $SSA_1 = o.m;\n  $SSA_1(1);\n  let $SSA_2 = (0, x);\n  return $SSA_2;\n}",
    );
    assert_eq!(n, 2, "{code}");
    assert!(code.contains("(0, o.m)(1);"), "{code}");
    assert!(code.contains("return (0, x);"), "{code}");
    let (code, _) = forward("function f(a, b) {\n  let $SSA_1 = a + b;\n  return $SSA_1 * 2;\n}");
    assert!(code.contains("return (a + b) * 2;"), "{code}");
}

#[test]
fn keeps_moves_past_effects_branches_and_slot_reads() {
    for source in [
        // an earlier call in the read's statement
        "function h() { sink(); }\nfunction f() {\n  let $SSA_1 = x();\n  h(y(), $SSA_1);\n}",
        // a slot read before the value
        "function h() { sink(); }\nfunction f() {\n  let $SSA_1 = x();\n  h($KEY_1, $SSA_1);\n}",
        // the read only runs in one arm
        "function h() { sink(); }\nfunction f(c) {\n  let $SSA_1 = x();\n  if (c) {\n    h($SSA_1);\n  }\n}",
        "function f(c) {\n  let $SSA_1 = x();\n  return c ? $SSA_1 : 0;\n}",
        "function f(c) {\n  let $SSA_1 = x();\n  return c && $SSA_1;\n}",
        // an intervening statement with an effect
        "function h() { sink(); }\nfunction f() {\n  let $SSA_1 = x();\n  y();\n  h($SSA_1);\n}",
        // an intervening store into an input of the value
        "function f(o) {\n  let $SSA_1 = o.p;\n  o = 1;\n  return $SSA_1;\n}",
        // a loop body
        "function f() {\n  let $SSA_1 = x();\n  for (;;) {\n    return $SSA_1;\n  }\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 0, "{source} -> {code}");
    }
}

#[test]
fn treats_fresh_tuple_slots_as_stable_and_member_targets_as_unread() {
    let (code, n) = forward(
        "function $VMHELPER_1(a) {\n  return [a, a.m];\n}\nfunction f(o, k, v) {\n  let $SSA_1 = $VMHELPER_1(o);\n  let $SSA_2 = $SSA_1[0];\n  $SSA_1[1].call($SSA_2, 1);\n  let $SSA_3 = v ^ 1;\n  o[k] = $SSA_3;\n  let $SSA_4 = $SSA_1[1];\n  $SSA_4();\n  return [$SSA_2, Infinity];\n}",
    );
    assert_eq!(n, 3, "{code}");
    assert!(code.contains("$SSA_1[1].call($SSA_1[0], 1);"), "{code}");
    assert!(code.contains("o[k] = v ^ 1;"), "{code}");
    assert!(code.contains("(0, $SSA_1[1])();"), "{code}");
    assert!(code.contains("return [$SSA_1[0], Infinity];"), "{code}");
    // an escaping holder or a compound target stays
    for source in [
        "function $VMHELPER_1(a) {\n  return [a, a];\n}\nfunction f(o) {\n  let $SSA_1 = $VMHELPER_1(o);\n  let $SSA_2 = $SSA_1[0];\n  g($SSA_1);\n  return [$SSA_2, $SSA_2];\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 0, "{source} -> {code}");
    }
}

#[test]
fn reads_duplicate_tuple_slots_through_the_first_name() {
    let (code, n) = forward(
        "function $VMHELPER_1(a) {\n  let $SSA_9 = a.m;\n  return [$SSA_9, $SSA_9];\n}\nfunction f(o) {\n  let [$SSA_1, $SSA_2] = $VMHELPER_1(o);\n  return [$SSA_1.x, $SSA_2.call(o), $SSA_2];\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("let [$SSA_1, ] = $VMHELPER_1(o);"), "{code}");
    assert!(code.contains("return [$SSA_1.x, $SSA_1.call(o), $SSA_1];"), "{code}");
    // different slots are different values
    let (code, n) = forward(
        "function $VMHELPER_1(a, b) {\n  return [a, b];\n}\nfunction f(o) {\n  let [$SSA_1, $SSA_2] = $VMHELPER_1(o.p, o.q);\n  return [$SSA_1, $SSA_2, $SSA_2];\n}",
    );
    assert_eq!(n, 0, "{code}");
}

#[test]
fn reads_pass_through_slots_as_their_arguments() {
    // a pure shuffle of stable arguments disappears
    let (code, n) = forward(
        "function $VMHELPER_1(a, b, c) {\n  return [c, a, b, a];\n}\nfunction g() { sink(); }\nfunction f(x, y) {\n  let [$SSA_1, $SSA_2, $SSA_3, $SSA_4] = $VMHELPER_1(x, 7, y);\n  return g($SSA_1, $SSA_2, $SSA_3, $SSA_4, $SSA_4);\n}",
    );
    assert_eq!(n, 4, "{code}");
    assert!(!code.contains("$VMHELPER_1(x"), "{code}");
    assert!(code.contains("return g(y, x, 7, x, x);"), "{code}");
    // a computing helper keeps its call and its computed slot
    let (code, n) = forward(
        "function $VMHELPER_1(a, b) {\n  let $SSA_9 = a.m;\n  return [$SSA_9, b];\n}\nfunction g() { sink(); }\nfunction f(x, y) {\n  let [$SSA_1, $SSA_2] = $VMHELPER_1(x, y);\n  return g($SSA_1, $SSA_2, $SSA_2);\n}",
    );
    // the helper's own single-use temporary goes too
    assert_eq!(n, 2, "{code}");
    assert!(code.contains("let [$SSA_1, ] = $VMHELPER_1(x, y);"), "{code}");
    assert!(code.contains("return g($SSA_1, y, y);"), "{code}");
    for source in [
        // a computed argument is not copied
        "function $VMHELPER_1(a, b) {\n  return [b, a];\n}\nfunction f(x) {\n  let [$SSA_1, $SSA_2] = $VMHELPER_1(x.p, x.q);\n  return [$SSA_1, $SSA_2, $SSA_2, $SSA_1];\n}",
        // a reassigned parameter is not the argument
        "function $VMHELPER_1(a, b) {\n  a = 2;\n  b = 3;\n  return [b, a];\n}\nfunction f(x, y) {\n  let [$SSA_1, $SSA_2] = $VMHELPER_1(x, y);\n  return [$SSA_1, $SSA_2, $SSA_2, $SSA_1];\n}",
        // `arguments` can write the parameter
        "function $VMHELPER_1(a, b) {\n  arguments[0] = 1;\n  arguments[1] = 2;\n  return [b, a];\n}\nfunction f(x, y) {\n  let [$SSA_1, $SSA_2] = $VMHELPER_1(x, y);\n  return [$SSA_1, $SSA_2, $SSA_2, $SSA_1];\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 0, "{source} -> {code}");
    }
}

#[test]
fn never_copies_a_moved_value() {
    // the holder is read once, so its call may move into the slot read;
    // the slot is then read twice and must not repeat the call
    let (code, _) = forward(
        "function $VMHELPER_1(a) {\n  let $SSA_9 = a.m;\n  return [$SSA_9, a.n];\n}\nfunction f(o) {\n  let $SSA_1 = $VMHELPER_1(o);\n  let $SSA_2 = $SSA_1[0];\n  return [$SSA_2, $SSA_2];\n}",
    );
    assert_eq!(code.matches("$VMHELPER_1(o)").count(), 1, "{code}");
}

#[test]
fn a_double_slash_in_a_string_is_not_a_comment() {
    let (code, n) = forward("function f(a) {\n  let $SSA_1 = a + \"://\";\n  return $SSA_1;\n}");
    assert_eq!(n, 1, "{code}");
    let (code, n) = forward("function f(a) {\n  let $SSA_1 = /* keep */ a;\n  return $SSA_1;\n}");
    assert_eq!(n, 0, "{code}");
}

#[test]
fn moves_across_a_stable_declaration_decided_later() {
    let (code, n) = forward(
        "function h() { sink(); }\nfunction f(o) {\n  let $SSA_1 = o.p;\n  let $SSA_2 = o;\n  return h($SSA_1, $SSA_2, $SSA_2);\n}",
    );
    assert_eq!(n, 2, "{code}");
    assert!(code.contains("return h(o.p, o, o);"), "{code}");
}

#[test]
fn moves_across_a_later_move_into_the_same_statement() {
    let (code, n) = forward(
        "function f(o) {\n  let $SSA_1 = o.f;\n  let $SSA_2 = o.g;\n  return $SSA_1($SSA_2);\n}",
    );
    assert_eq!(n, 2, "{code}");
    assert!(code.contains("return (0, o.f)(o.g);"), "{code}");
    // two property reads commute (no getters)
    let (code, n) = forward(
        "function f(o) {\n  let $SSA_1 = o.f;\n  let $SSA_2 = o.g;\n  return [$SSA_2, $SSA_1];\n}",
    );
    assert_eq!(n, 2, "{code}");
    assert!(code.contains("return [o.g, o.f];"), "{code}");
    // a call between still orders them
    let (code, _) = forward(
        "function f(o, h) {\n  let $SSA_1 = o.f;\n  let $SSA_2 = h(o);\n  return [$SSA_2, $SSA_1];\n}",
    );
    assert!(code.contains("let $SSA_1 = o.f;"), "{code}");
}

#[test]
fn reads_a_slot_snapshot_from_the_slot_while_nothing_writes_it() {
    let (code, n) = forward(
        "function g() { sink(); }\nfunction f() {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  g();\n  g($SSA_1, $SSA_1);\n  $KEY_1 = 2;\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("g($KEY_1, $KEY_1);"), "{code}");
    // an outer slot read with nothing that runs code in between
    let (code, n) = forward(
        "function g() { sink(); }\nvar $KEY_1 = g();\nfunction f() {\n  let $SSA_1 = $KEY_1;\n  return [$SSA_1, $SSA_1];\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("return [$KEY_1, $KEY_1];"), "{code}");
    for source in [
        // a write between the copy and a read
        "function g() { sink(); }\nfunction f() {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  $KEY_1 = 2;\n  g($SSA_1, $SSA_1);\n}",
        // a closure writes the slot and a call runs in between
        "function g() { sink(); }\nfunction f() {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  g(() => { $KEY_1 = 3; });\n  g($SSA_1, $SSA_1);\n}",
        // a `var` redeclaration stores into the slot
        "function g() { sink(); }\nfunction f() {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  var $KEY_1 = 2;\n  g($SSA_1, $SSA_1);\n}",
        // a read in a loop that writes the slot afterwards
        "function g() { sink(); }\nfunction f() {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  for (;;) {\n    g($SSA_1, $SSA_1);\n    $KEY_1 = 2;\n  }\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 0, "{source} -> {code}");
    }
}

#[test]
fn reads_a_shared_slot_while_every_store_owns_its_binding() {
    // a program-level initializer cannot change during the copy's extent
    let (code, n) = forward(
        "function g() { sink(); }\nvar $KEY_1 = g();\nfunction f() {\n  let $SSA_1 = $KEY_1;\n  g();\n  g($SSA_1, $SSA_1);\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("g($KEY_1, $KEY_1);"), "{code}");
    // the slot belongs to the enclosing function that stores it
    let (code, n) = forward(
        "function g() { sink(); }\nfunction outer() {\n  var $KEY_1 = g();\n  function f() {\n    let $SSA_1 = $KEY_1;\n    g();\n    g($SSA_1, $SSA_1);\n  }\n  $KEY_1 = 2;\n  f();\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("g($KEY_1, $KEY_1);"), "{code}");
}

#[test]
fn reads_a_shared_slot_when_nothing_runs_before_the_reads() {
    let (code, n) = forward(
        "function h() { $KEY_1 = 2; }\nvar $KEY_1 = g();\nfunction f() {\n  let $SSA_1 = $KEY_1;\n  return [$SSA_1, $SSA_1];\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("return [$KEY_1, $KEY_1];"), "{code}");
}

#[test]
fn keeps_a_shared_slot_snapshot_when_a_store_can_overwrite_the_binding() {
    for source in [
        // a program-level slot stored by the copy's own function, which
        // a recursive call can enter again
        "function g() { sink(); }\nvar $KEY_1 = g();\nfunction f(n) {\n  let $SSA_1 = $KEY_1;\n  $KEY_1 = $SSA_1 + 1;\n  if (n) f(n - 1);\n  return g($SSA_1, $SSA_1);\n}",
        // an outer-level slot stored by an enclosing function, which a
        // recursive call can re-enter
        "function g() { sink(); }\nvar $KEY_1 = g();\nfunction outer(n) {\n  function f() {\n    let $SSA_1 = $KEY_1;\n    g();\n    return g($SSA_1, $SSA_1);\n  }\n  $KEY_1 = 2;\n  if (n) outer(n - 1);\n  return f();\n}",
        // an `await` between the copy and its reads lets the enclosing
        // frame run its store into the slot
        "function g() { sink(); }\nfunction outer() {\n  var $KEY_1 = g();\n  async function f() {\n    let $SSA_1 = $KEY_1;\n    await 0;\n    return g($SSA_1, $SSA_1);\n  }\n  return f();\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 0, "{source} -> {code}");
    }
}

#[test]
fn reads_a_slot_before_a_store_whose_right_side_consumes_the_copy() {
    // the target is written after the whole right side runs
    let (code, n) = forward(
        "function f() {\n  var $KEY_3 = g();\n  let $SSA_1 = $KEY_3;\n  $KEY_3 = ($SSA_1 << 5) - $SSA_1 | 0;\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("$KEY_3 = ($KEY_3 << 5) - $KEY_3 | 0;"), "{code}");
    for source in [
        // a read after the store sees the new value
        "function f() {\n  var $KEY_3 = g();\n  let $SSA_1 = $KEY_3;\n  $KEY_3 = $SSA_1 + 1;\n  return [$SSA_1, $SSA_1];\n}",
        // the initializer writes after it runs, but a later read does not
        "function f() {\n  var $KEY_3 = g();\n  let $SSA_1 = $KEY_3;\n  var $KEY_3 = $SSA_1 + 1;\n  return [$SSA_1, $SSA_1];\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 0, "{source} -> {code}");
    }
}

#[test]
fn reads_a_snapshot_in_a_loop_that_does_not_store_the_slot() {
    let (code, n) = forward(
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  for (let i = 0; i < c; i++) {\n    g($SSA_1, $SSA_1);\n  }\n  $KEY_1 = 2;\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("g($KEY_1, $KEY_1);"), "{code}");
    // a loop that stores the slot changes it between iterations
    let (code, n) = forward(
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  for (let i = 0; i < c; i++) {\n    g($SSA_1, $SSA_1);\n    $KEY_1 = 2;\n  }\n}",
    );
    assert_eq!(n, 0, "{code}");
}

#[test]
fn reads_a_snapshot_past_a_store_the_block_jumps_away_from() {
    for source in [
        // `return` leaves the activation before the read
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  if (c) {\n    $KEY_1 = 2;\n    return;\n  }\n  return g($SSA_1, $SSA_1);\n}",
        // `throw` does too
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  if (c) {\n    $KEY_1 = 2;\n    throw 0;\n  }\n  return g($SSA_1, $SSA_1);\n}",
        // `continue` starts the iteration that re-runs the copy
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  for (let i = 0; i < c; i++) {\n    let $SSA_1 = $KEY_1;\n    if (i) {\n      $KEY_1 = 2;\n      continue;\n    }\n    g($SSA_1, $SSA_1);\n  }\n}",
        // a labeled `continue` names its target loop
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  vm_loop_0: for (;;) {\n    let $SSA_1 = $KEY_1;\n    if (c) {\n      $KEY_1 = 2;\n      continue vm_loop_0;\n    }\n    g($SSA_1, $SSA_1);\n    break;\n  }\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 1, "{source} -> {code}");
        assert!(code.contains("g($KEY_1, $KEY_1)"), "{source} -> {code}");
    }
}

#[test]
fn keeps_a_snapshot_when_the_store_can_still_reach_a_read() {
    for source in [
        // the read is after the store in the block that continues
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  for (let i = 0; i < c; i++) {\n    let $SSA_1 = $KEY_1;\n    $KEY_1 = 2;\n    g($SSA_1, $SSA_1);\n    continue;\n  }\n}",
        // the read is in the return expression that ends the block
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  if (c) {\n    $KEY_1 = 2;\n    return g($SSA_1, $SSA_1);\n  }\n  return 0;\n}",
        // a labeled read after the store in the continuing block
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  vm_loop_0: for (;;) {\n    let $SSA_1 = $KEY_1;\n    $KEY_1 = 2;\n    g($SSA_1, $SSA_1);\n    continue vm_loop_0;\n  }\n}",
        // `break` finishes the loop, so a read after it sees the store
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  for (;;) {\n    $KEY_1 = 2;\n    break;\n  }\n  return g($SSA_1, $SSA_1);\n}",
        // the read after the loop still runs after `continue`
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  for (let i = 0; i < c; i++) {\n    if (i) {\n      $KEY_1 = 2;\n      continue;\n    }\n  }\n  return g($SSA_1, $SSA_1);\n}",
        // a finalizer runs the read after the jump
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  for (let i = 0; i < c; i++) {\n    let $SSA_1 = $KEY_1;\n    try {\n      $KEY_1 = 2;\n      continue;\n    } finally {\n      g($SSA_1, $SSA_1);\n    }\n  }\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 0, "{source} -> {code}");
    }
}

#[test]
fn moves_past_a_call_method_read() {
    let (code, n) = forward(
        "function f(g, t, o) {\n  let $SSA_1 = o.p;\n  return g.call(t, $SSA_1);\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("return g.call(t, o.p);"), "{code}");
    // any other property read first still blocks
    let (code, n) = forward(
        "function f(g, t, o) {\n  let $SSA_1 = o.p;\n  return g.bind(t, $SSA_1);\n}",
    );
    assert_eq!(n, 0, "{code}");
}

#[test]
fn hands_a_stored_value_over_to_its_slot() {
    let (code, n) = forward(
        "function h() { sink(); }\nfunction f(o) {\n  let $SSA_1 = o.p;\n  var $KEY_1 = $SSA_1;\n  return [$SSA_1, $SSA_1];\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("var $KEY_1 = o.p;"), "{code}");
    assert!(code.contains("return [$KEY_1, $KEY_1];"), "{code}");
    for source in [
        // the slot changes before a later read
        "function f(o) {\n  let $SSA_1 = o.p;\n  var $KEY_1 = $SSA_1;\n  $KEY_1 = 2;\n  return [$SSA_1, $SSA_1];\n}",
        // an outer slot, with a call in between
        "function h() { sink(); }\nvar $KEY_1;\nfunction f(o) {\n  let $SSA_1 = o.p;\n  $KEY_1 = $SSA_1;\n  h();\n  return [$SSA_1, $SSA_1];\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 0, "{source} -> {code}");
    }
}

#[test]
fn writes_a_later_stored_value_to_its_slot_early() {
    let (code, n) = forward(
        "function h() { sink(); }\nfunction f(o) {\n  let $SSA_1 = h(o);\n  h($SSA_1);\n  var $KEY_1 = $SSA_1;\n  return [$SSA_1, $SSA_1];\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("$KEY_1 = h(o);\n  h($KEY_1);\n  var $KEY_1;\n  return [$KEY_1, $KEY_1];"), "{code}");
    for source in [
        // the slot is read before the store
        "function h() { sink(); }\nfunction f(o) {\n  var $KEY_1 = h();\n  let $SSA_1 = h(o);\n  h($KEY_1);\n  $KEY_1 = $SSA_1;\n  return [$SSA_1, $SSA_1];\n}",
        // a handler could see the early write
        "function h() { sink(); }\nfunction f(o) {\n  var $KEY_1 = h();\n  try {\n    let $SSA_1 = h(o);\n    h();\n    $KEY_1 = $SSA_1;\n    return [$SSA_1, $SSA_1];\n  } catch (e) {\n    return $KEY_1;\n  }\n}",
        // a closure sees the slot and code runs in between
        "function h() { sink(); }\nfunction f(o) {\n  var $KEY_1 = h();\n  let g = () => $KEY_1;\n  let $SSA_1 = h(o);\n  h(g);\n  $KEY_1 = $SSA_1;\n  return [$SSA_1, $SSA_1];\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 0, "{source} -> {code}");
    }
}

#[test]
fn leaves_a_read_in_a_deleted_shuffle_to_the_call() {
    // an early slot and the shuffle's deletion both claimed the argument
    let (code, _) = forward(
        "function $VMHELPER_1(a, b) {\n  return [b, a, a];\n}\nfunction h() { sink(); }\nfunction f(x) {\n  let $SSA_1 = h(x);\n  let [$SSA_2, ] = $VMHELPER_1($SSA_1, x);\n  h($SSA_2, $SSA_1);\n  var $KEY_1 = $SSA_1;\n  return [$SSA_1, $SSA_1];\n}",
    );
    assert!(!code.contains("$VMHELPER_1($"), "{code}");
    assert!(code.contains("$KEY_1 = h(x);\n  h(x, $KEY_1);\n  var $KEY_1;\n  return [$KEY_1, $KEY_1];"), "{code}");
    // the deleted call does not take a moved value's effect with it
    let (code, _) = forward(
        "function $VMHELPER_1(a, b) {\n  return [b, a];\n}\nfunction f(x, g) {\n  let $SSA_1 = g();\n  let [$SSA_2, ] = $VMHELPER_1($SSA_1, x);\n  return $SSA_2;\n}",
    );
    assert!(code.contains("let $SSA_1 = g();\n  return x;"), "{code}");
}

#[test]
fn treats_an_internal_slot_like_a_value() {
    let (code, n) = forward(
        "function h() { sink(); }\nfunction f(o) {\n  var $KEY_1 = o.p;\n  return h($KEY_1);\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("return h(o.p);"), "{code}");
    for source in [
        // stored again
        "function h() { sink(); }\nfunction f(o) {\n  var $KEY_1 = o.p;\n  $KEY_1 = h();\n  return h($KEY_1);\n}",
        // exported to a nested thread
        "function h() { sink(); }\nfunction f(o) {\n  var $KEY_1 = o.p;\n  return [h($KEY_1), () => $KEY_1];\n}",
        // in a loop
        "function h() { sink(); }\nfunction f(o) {\n  for (;;) {\n    var $KEY_1 = o.p;\n    h($KEY_1);\n  }\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 0, "{source} -> {code}");
    }
}

#[test]
fn reads_fixed_arguments_as_stable_values() {
    let (code, n) = forward(
        "function h() { sink(); }\nfunction f() {\n  let $SSA_1 = arguments[1];\n  var $KEY_0 = arguments[0];\n  h($SSA_1, $KEY_0);\n  return [$SSA_1, $SSA_1];\n}",
    );
    assert!(n >= 1, "{code}");
    assert!(code.contains("return [arguments[1], arguments[1]];"), "{code}");
    for source in [
        // a parameter aliases `arguments` in sloppy code
        "function f(a) {\n  let $SSA_1 = arguments[0];\n  a = 2;\n  return [$SSA_1, $SSA_1];\n}",
        // `arguments` is written
        "function f() {\n  let $SSA_1 = arguments[0];\n  arguments[0] = 2;\n  return [$SSA_1, $SSA_1];\n}",
        // `arguments` escapes
        "function g() { sink(); }\nfunction f() {\n  let $SSA_1 = arguments[0];\n  g(arguments);\n  return [$SSA_1, $SSA_1];\n}",
    ] {
        let (code, n) = forward(source);
        assert_eq!(n, 0, "{source} -> {code}");
    }
}

#[test]
fn deletes_decoys_and_moves_pure_primitives() {
    let (code, _) = forward(
        "function g() { sink(); }\nfunction f(o) {\n  let $SSA_1 = Number(o) + 2;\n  $SSA_1 + 2;\n  g();\n  o[1] = 3;\n  return $SSA_1 * 2;\n}",
    );
    assert!(!code.contains("$SSA_1 + 2;"), "{code}");
    // Number(o) runs before g(): the call stays, the pure + 2 moves
    assert!(code.contains("return ($SSA_1 * 2)") || code.contains("return $SSA_1 * 2;") || code.contains("Number(o) + 2"), "{code}");
    // operands are primitives: a discarded sum is a decoy too
    let (code, _) = forward("function f(o) {\n  o + 2;\n  return 1;\n}");
    assert!(!code.contains("o + 2;"), "{code}");
}

#[test]
fn writes_a_value_to_the_carry_it_joins_into() {
    let (code, n) = forward(
        "function h() { sink(); }\nfunction f(o, c) {\n  let $CARRY_1;\n  if (c) {\n    let $SSA_1 = h(o);\n    h(null != $SSA_1);\n    $CARRY_1 = $SSA_1;\n    h($SSA_1);\n  } else {\n    $CARRY_1 = 2;\n  }\n  return $CARRY_1;\n}",
    );
    assert_eq!(n, 1, "{code}");
    assert!(code.contains("$CARRY_1 = h(o);\n    h(null != $CARRY_1);\n    h($CARRY_1);"), "{code}");
    // the carry is read between the value and its store
    let (code, n) = forward(
        "function h() { sink(); }\nfunction f(o) {\n  let $CARRY_1 = 0;\n  let $SSA_1 = h(o);\n  h($CARRY_1);\n  $CARRY_1 = $SSA_1;\n  return [$SSA_1, $SSA_1];\n}",
    );
    assert_eq!(n, 0, "{code}");
}

#[test]
fn keeps_a_snapshot_when_a_catch_resumes_after_the_store() {
    for source in [
        // an explicit throw into a catch of the same function
        "function g() { sink(); }\nfunction f(c, x) {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  try {\n    if (c) {\n      $KEY_1 = 2;\n      throw x;\n    }\n  } catch (e) {}\n  return [$SSA_1, $SSA_1];\n}",
        // an implicit throw between the store and a return
        "function g() { sink(); }\nfunction f(c) {\n  var $KEY_1 = g();\n  let $SSA_1 = $KEY_1;\n  try {\n    if (c) {\n      $KEY_1 = 2;\n      g();\n      return 0;\n    }\n  } catch (e) {}\n  return [$SSA_1, $SSA_1];\n}",
    ] {
        let (code, n) = forward(source);
        assert!(code.contains("return [$SSA_1, $SSA_1];"), "{source} -> {code} ({n})");
    }
}

#[test]
fn copies_a_builtin_method_into_its_reads() {
    let (code, _) = forward(
        "function f(g, x) {\n  let $SSA_1 = Object[\"defineProperty\"];\n  g();\n  $SSA_1.call(Object, x, \"a\", {});\n  return $SSA_1;\n}",
    );
    assert!(code.contains("Object[\"defineProperty\"].call(Object, x"), "{code}");
    assert!(code.contains("return Object[\"defineProperty\"];"), "{code}");
    // a local named Object is not the built-in
    let (code, _) = forward("function f(Object, g) {\n  let $SSA_1 = Object.keys;\n  g();\n  return [$SSA_1, $SSA_1];\n}");
    assert!(code.contains("[$SSA_1, $SSA_1]"), "{code}");
}

#[test]
fn moves_a_fresh_array_down_to_its_one_read() {
    let (code, _) = forward("function f(g, o) {\n  let $SSA_1 = [];\n  g();\n  o.x = g();\n  if (o) {\n    o.next([$SSA_1, 2]);\n  }\n}");
    assert!(code.contains("o.next([[], 2]);") && !code.contains("$SSA_1"), "{code}");
    // a loop would allocate it again on every pass
    let (code, _) = forward("function f(g, o) {\n  let $SSA_1 = [];\n  g();\n  while (o.x) {\n    o.next($SSA_1);\n  }\n}");
    assert!(code.contains("o.next($SSA_1);"), "{code}");
}

#[test]
fn a_slot_its_declaring_function_writes_is_settled_inside_closures() {
    let source = "function t() {\n  let $KEY_1, $KEY_2;\n  function h(a, o) {\n    let $SSA_1 = $KEY_1(a);\n    o.x = $KEY_1(a, $SSA_1);\n  }\n  $KEY_1 = h;\n  return h;\n}";
    let (code, _) = forward(source);
    assert!(code.contains("o.x = $KEY_1(a, $KEY_1(a));"), "{code}");
    // more writes, even in a loop, while the declaring frame is suspended
    let (code, _) = forward(&source.replace("$KEY_1 = h;", "$KEY_1 = h;\n  while (t) $KEY_1 = t;"));
    assert!(code.contains("o.x = $KEY_1(a, $KEY_1(a));"), "{code}");
    // a let declared after other code may be read in its TDZ
    let (code, _) = forward(&source.replace("  let $KEY_1, $KEY_2;\n", "  h();\n  let $KEY_1, $KEY_2;\n"));
    assert!(code.contains("let $SSA_1"), "{code}");
    // a closure writes it
    let (code, _) = forward(&source.replace("$KEY_1 = h;", "(() => { $KEY_1 = h; })();"));
    assert!(code.contains("let $SSA_1"), "{code}");
}

#[test]
fn copies_a_property_read_late_only_while_nothing_can_change_it() {
    let source = "function f(o) {\n  let $SSA_1 = o.p;\n  return [$SSA_1, $SSA_1 + 1];\n}";
    assert_eq!(forward(source).1, 0);
    let late = super::forward_values_and_property_reads(source).unwrap();
    assert_eq!(late.forwarded, 1, "{}", late.code);
    assert!(late.code.contains("return [o.p, (o.p) + 1];"), "{}", late.code);
    for kept in [
        "function f(o, g) {\n  let $SSA_1 = o.p;\n  g(o);\n  return [$SSA_1, $SSA_1];\n}",
        "function f(o) {\n  let $SSA_1 = o.p;\n  o.p = 2;\n  return [$SSA_1, $SSA_1];\n}",
        "function f(o, n) {\n  let $SSA_1 = o.p;\n  for (let i = 0; i < n; i++) {\n    use_it($SSA_1, o);\n  }\n}",
        "function f(o, n) {\n  let $SSA_1 = o.length;\n  for (let i = 0; i < $SSA_1; i++) {\n    o[i] = 1;\n  }\n}",
        "function f(o) {\n  let $SSA_1 = o[{}];\n  return [$SSA_1, $SSA_1];\n}",
        "function f(o, h) {\n  let $SSA_1 = o[h()];\n  return [$SSA_1, $SSA_1];\n}",
    ] {
        let late = super::forward_values_and_property_reads(kept).unwrap();
        assert_eq!(late.forwarded, 0, "{}", late.code);
    }
}

#[test]
fn reads_a_length_in_a_loop_test_when_the_loop_stores_elsewhere() {
    let source = "function f(a, out) {\n  let $SSA_1 = a.length;\n  for (let k = 0; k < $SSA_1; k++) {\n    out[a[k]] = k;\n  }\n}";
    let late = super::forward_values_and_property_reads(source).unwrap();
    assert!(late.code.contains("k < (a.length)"), "{}", late.code);
}

#[test]
fn reads_a_property_of_arguments_across_calls_not_handed_arguments() {
    let source = "function f() {\n  let $SSA_1 = arguments[0].location;\n  let $CARRY_2 = 0;\n  let $CARRY_3 = g;\n  for (;;) {\n    if (!($CARRY_2 < $SSA_1.length)) break;\n    if (!!{}.hasOwnProperty.call($CARRY_3, $SSA_1[$CARRY_2])) {\n      $CARRY_3 = $CARRY_3[$SSA_1[$CARRY_2]];\n      ++$CARRY_2;\n      continue;\n    }\n    break;\n  }\n  return $CARRY_3;\n}";
    let late = super::forward_values_and_property_reads(source).unwrap();
    assert!(!late.code.contains("let $SSA_1"), "{}", late.code);
}

#[test]
fn reads_a_property_inside_the_store_it_feeds() {
    let source = "function f(o, p) {\n  let $SSA_1 = o.a;\n  $SSA_1.ts = $SSA_1.ts + p.top;\n}";
    let late = super::forward_values_and_property_reads(source).unwrap();
    assert!(late.code.contains("(o.a).ts = (o.a).ts + p.top;"), "{}", late.code);
}

#[test]
fn never_copies_a_fresh_array_into_several_reads() {
    for source in [
        "function f() {\n  let $SSA_1 = [0];\n  let $SSA_2 = $SSA_1;\n  $SSA_2[1] = 1;\n  $SSA_2[2] = 0;\n  let $SSA_3 = $SSA_2;\n  $SSA_3[3] = 2;\n  return $SSA_3;\n}",
        "function f() {\n  let $SSA_1 = {};\n  let $SSA_2 = $SSA_1;\n  $SSA_2.a = 1;\n  return $SSA_2;\n}",
        "function f(g) {\n  let $SSA_1 = function () {};\n  let $SSA_2 = $SSA_1;\n  return g($SSA_2, $SSA_2);\n}",
    ] {
        let (code, n) = forward(source);
        assert!(!code.contains("[0][") && !code.contains("({}).") && !code.contains("{}.a"), "{source} -> {code} ({n})");
        assert_eq!(code.matches("[0]").count() + code.matches("{}").count(), 1, "{source} -> {code} ({n})");
    }
}

#[test]
fn reads_a_slot_as_the_stable_value_just_stored() {
    let (code, _) = forward(
        "function f(t) {\n  var $KEY_8;\n  if (t) {\n    $KEY_8 = 7;\n    g($KEY_8 + 1);\n  }\n  return $KEY_8;\n}\nfunction g() { sink(); }",
    );
    assert!(code.contains("g(7 + 1);"), "{code}");
    // after the if the store may not have run
    assert!(code.contains("return $KEY_8;"), "{code}");
    // a store in between
    let (code, _) = forward("function f(x) {\n  var $KEY_8 = 1;\n  $KEY_8 = x();\n  return $KEY_8;\n}");
    assert!(code.contains("return $KEY_8;"), "{code}");
}

#[test]
fn reads_a_tested_boolean_as_its_known_value_in_each_arm() {
    let (code, _) = forward(
        "function g() { sink(); }\nfunction f(a) {\n  let $SSA_1 = a < 0;\n  let $CARRY_1;\n  if ($SSA_1) {\n    $CARRY_1 = $SSA_1;\n    g();\n  } else {\n    $CARRY_1 = !$SSA_1;\n    g();\n  }\n  return $CARRY_1;\n}",
    );
    assert!(code.contains("$CARRY_1 = true;") && code.contains("$CARRY_1 = !false;"), "{code}");
    // not a known boolean
    let (code, _) = forward(
        "function g() { sink(); }\nfunction f(a) {\n  let $SSA_1 = a.p;\n  let $CARRY_1;\n  if ($SSA_1) {\n    $CARRY_1 = $SSA_1;\n    g();\n  }\n  return $CARRY_1;\n}",
    );
    assert!(code.contains("$CARRY_1 = $SSA_1;") || code.contains("a.p"), "{code}");
    assert!(!code.contains("true"), "{code}");
}

#[test]
fn writes_early_into_a_carry_declared_inside_the_same_try() {
    let source = "function g() { sink(); }\nfunction f(a, j) {\n  try {\n    let $CARRY_1;\n    if (j) {\n      let $SSA_1 = j + 1;\n      a[$SSA_1] = g();\n      $CARRY_1 = $SSA_1;\n    } else {\n      $CARRY_1 = 0;\n    }\n    return $CARRY_1;\n  } catch (e) {}\n}";
    let (code, _) = forward(source);
    assert!(code.contains("$CARRY_1 = j + 1;\n      a[$CARRY_1] = g();"), "{code}");
    // a carry declared outside the try stays
    let source = "function g() { sink(); }\nfunction f(a, j) {\n  let $CARRY_1 = 5;\n  try {\n    let $SSA_1 = j + 1;\n    a[$SSA_1] = g();\n    $CARRY_1 = $SSA_1;\n  } catch (e) {}\n  return $CARRY_1;\n}";
    let (code, _) = forward(source);
    assert!(code.contains("let $SSA_1 = j + 1;") || code.contains("a[j + 1]"), "{code}");
    assert!(!code.contains("$CARRY_1 = j + 1;"), "{code}");
}

#[test]
fn rejects_a_value_whose_names_would_be_shadowed() {
    let (code, n) = forward(
        "function f(p) {\n  let $SSA_1 = p;\n  {\n    let p = 2;\n    return [$SSA_1, $SSA_1, p];\n  }\n}",
    );
    assert_eq!(n, 0, "{code}");
}

#[test]
fn keeps_calls_of_child_thread_stubs() {
    // A child thread is an empty stub while its parent is cleaned; the
    // linker fills in its body, so calling it is not pure.
    let source = "function $THREAD_SELF() {\n\tfunction $THREAD_5_1() {}\n\t$KEY_1 = $THREAD_5_1;\n\t$THREAD_5_1();\n\t$KEY_1();\n\tlet $SSA_1 = $THREAD_5_1();\n\tsink();\n\treturn $SSA_1;\n}";
    let (code, _) = forward(source);
    assert!(code.contains("\t$THREAD_5_1();\n\t$KEY_1();\n"), "{code}");
    assert!(code.contains("let $SSA_1 = $THREAD_5_1();\n\tsink();"), "{code}");
}

#[test]
fn never_moves_a_store_between_a_snapshot_and_its_reads() {
    // A FunctionToStringComparison probe at cleanup stage 11h5:
    // `$SSA_x5` snapshots `$KEY_12` (the toString outcome) before the fresh
    // array `$SSA_…_589` is stored there, and the byte reads the snapshot.
    // Forwarding the snapshot (its reads read `$KEY_12`) and writing the
    // array to `$KEY_12` early, in one round, made the byte read the array.
    let source = include_str!("../../../../../tests/fixtures/forward_values/snapshot_then_early_store.js");
    let (code, _) = forward(source);
    let byte = code
        .lines()
        .find(|line| line.contains("^ $SSA_8_80068_d438_581()"))
        .expect("the byte stays");
    let byte_at = code.find(byte).expect("found");
    let stored_before = code[..byte_at].contains("$KEY_12 = [");
    assert!(!(stored_before && byte.contains("$KEY_12")), "{code}");
}

#[test]
fn never_moves_a_store_between_a_computed_value_and_its_read() {
    // A FunctionToStringComparison probe at cleanup stage 11h (puma):
    // `$SSA_…_807 = vm_sub_141186_8($CARRY_…_474 | 0, $KEY_21)` packs the
    // toString outcome `$KEY_21` holds; later `$KEY_21 = $SSA_…_813`.
    // Moving the pack to its one read (the byte store) and storing
    // `$SSA_…_813` in `$KEY_21` early, in one round, made the byte read
    // the new value.
    let source = include_str!("../../../../../tests/fixtures/forward_values/early_store_past_computed_read.js");
    let (code, _) = forward(source);
    let byte = code
        .lines()
        .find(|line| line.contains("vm_sub_141845_2($ARRAY_5_130555_d212_598, 3 + 1,"))
        .expect("the byte stays");
    let byte_at = code.find(byte).expect("found");
    let stored_before = code[..byte_at].contains("$KEY_21 = $KEY_12;")
        || code[..byte_at].contains("$KEY_21 = $SSA_5_130555_d212_813;");
    assert!(!(stored_before && byte.contains("$KEY_21")), "{code}");
}

#[test]
fn never_copies_a_call_through_a_copy_into_several_reads() {
    // The writer call runs once; its copy is read by a decoy and by a store.
    let source = "function f(g, k, buf) {\n  var $KEY_32 = k;\n  var $KEY_42 = buf;\n  let $SSA_x31 = g;\n  let $SSA_x32 = $KEY_32;\n  let $SSA_x30 = buf;\n  let $SSA_x29 = $SSA_x31($SSA_x32, 0, buf, 4);\n  var $KEY_37 = $SSA_x29;\n  let $SSA_x37 = $SSA_x29;\n  let $SSA_x38 = $SSA_x30;\n  let $SSA_x39 = $KEY_32;\n  let $SSA_x34 = $SSA_x38;\n  let $SSA_x35 = $SSA_x37 + 1;\n  let $SSA_x36 = $SSA_x39();\n  let $SSA_x33 = $SSA_x37 + 1;\n  $SSA_x33;\n  $SSA_x34[$SSA_x35] = $SSA_x36 ^ 60;\n  return $KEY_42;\n}";
    let (code, _) = forward(source);
    assert_eq!(code.matches("g(").count() + code.matches("$SSA_x31(").count(), 1, "{code}");
}

#[test]
fn never_moves_an_array_literal_into_a_slot_read() {
    let source = "function f(x, g) {\n  let $ARRAY_1 = [x, x];\n  return g($ARRAY_1[0]);\n}";
    let (code, _) = forward(source);
    assert!(!code.contains("[x, x][0]"), "{code}");
}

#[test]
fn never_moves_a_tuple_helper_call_into_a_slot_read() {
    let source = "function f(x, g) {\n  function vm_sub_1($PARAMA) {\n    return [$PARAMA | 0, $PARAMA | 0];\n  }\n  function vm_sub_2($PARAMA) {\n    return [$PARAMA, $PARAMA];\n  }\n  let $SSA_1 = vm_sub_1(x);\n  return g($SSA_1[0]);\n}";
    let (code, _) = forward(source);
    assert!(!code.contains("vm_sub_1(x)[0]") && !code.contains("(vm_sub_1(x))[0]"), "{code}");
}
