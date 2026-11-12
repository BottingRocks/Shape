use super::merge_derived_values;

fn run(source: &str) -> String {
    let output = std::process::Command::new("node")
        .args(["-e", &format!("{source};console.log(JSON.stringify(f()));")])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Merge to a fixpoint; the result must run like the source.
fn merged(source: &str) -> (String, usize) {
    let mut code = source.to_string();
    let mut total = 0;
    loop {
        let result = merge_derived_values(&code).unwrap();
        if result.merged == 0 {
            break;
        }
        total += result.merged;
        code = result.code;
    }
    assert_eq!(run(source), run(&code), "{code}");
    (code, total)
}

#[test]
fn merges_a_countdown_back_into_its_counter() {
    let source = "function f() { const a = [1, 2, 3]; const out = []; let $LOOP_1 = a.length; vm: for (;;) { let $SSA_2 = $LOOP_1 - 1; if ($SSA_2 >= 0) { out.push(a[$SSA_2]); $LOOP_1 = Number($SSA_2); continue vm; } else { return out; } } }";
    let (code, count) = merged(source);
    assert_eq!(count, 1, "{code}");
    assert!(code.contains("$LOOP_1 = $LOOP_1 - 1;"), "{code}");
    assert!(!code.contains("$SSA_2"), "{code}");
}

#[test]
fn merges_a_chain_one_link_at_a_time() {
    let source = "function f() { let i = 0; const out = []; let $SSA_1 = Number(i) + 1; out[$SSA_1] = 7; let $SSA_2 = Number($SSA_1) + 1; out[$SSA_2] = 8; return [out, $SSA_2]; }";
    let (code, count) = merged(source);
    assert_eq!(count, 2, "{code}");
    assert!(!code.contains("$SSA_"), "{code}");
}

#[test]
fn keeps_a_value_whose_source_is_read_again() {
    let source = "function f() { let i = 4; let $SSA_1 = i - 1; return [i, $SSA_1]; }";
    assert_eq!(merged(source).1, 0);
}

#[test]
fn keeps_a_value_when_a_loop_reads_the_old_source() {
    let source = "function f() { let i = 1; const out = []; while (i > 0) { let $SSA_1 = i - 1; out.push($SSA_1); if ($SSA_1 < 2) { i = 0; } } return out; }";
    let (code, count) = merged(source);
    assert_eq!(count, 0, "{code}");
}

#[test]
fn keeps_a_value_when_the_source_is_stored_before_its_reads() {
    let source = "function f() { let i = 3; let $SSA_1 = i + 1; i = 10; return [i, $SSA_1]; }";
    assert_eq!(merged(source).1, 0);
}

#[test]
fn keeps_a_buffer_across_a_later_var_initializer() {
    // A declaration initializer is a store too, although it has no write
    // reference in the semantic reference list. Reusing this scratch slot
    // used to replace the buffer with the optional field before serialization.
    let source = "function f() { let $ARRAY_1 = [1, 2, 3, 4]; var $KEY_2 = 'field'; if ($KEY_2 == null) { $KEY_2 = $ARRAY_1; $ARRAY_1[4] = 130; } else { $ARRAY_1.push($KEY_2); } $ARRAY_1.push(7); return $ARRAY_1; }";
    let (code, _) = merged(source);
    assert!(code.contains("$ARRAY_1"), "{code}");
}

#[test]
fn keeps_a_value_a_closure_could_see() {
    let source = "function f() { let i = 3; const g = () => i; let $SSA_1 = i + 1; return [g(), $SSA_1]; }";
    assert_eq!(merged(source).1, 0);
}

#[test]
fn keeps_a_value_inside_a_try_block_whose_handler_reads_the_old_value() {
    let source = "function f() { let i = 3; try { let $SSA_1 = i + 1; return $SSA_1; } catch (e) { return i; } }";
    assert_eq!(merged(source).1, 0);
    let source = "function f() { let i = 3; let $JOIN_1 = i; try { $JOIN_1 = g(); i = $JOIN_1; } catch (e) { return i; } return i; } function g() { throw 1; }";
    assert_eq!(merged(source).1, 0);
}

#[test]
fn merges_a_copy_stored_inside_a_try() {
    let source = "function f() { let $LOOP_1 = 3; let $JOIN_2 = $LOOP_1; try { $JOIN_2 = g(); } catch (e) {} $LOOP_1 = $JOIN_2; return $LOOP_1; } function g() { throw 1; }";
    let (code, count) = merged(source);
    assert_eq!(count, 1, "{code}");
    assert!(!code.contains("$JOIN_2"), "{code}");
}

#[test]
fn merges_a_copy_written_back_into_its_local() {
    let source = "function f() { const a = [0, 5, 0, 7]; let $LOOP_1 = 0; for (let i = 0; i < a.length; i++) { let $JOIN_2 = $LOOP_1; if (a[i]) { use_it($LOOP_1); $JOIN_2 = a[i]; } $LOOP_1 = $JOIN_2; } return $LOOP_1; } function use_it() {}";
    let (code, count) = merged(source);
    assert_eq!(count, 1, "{code}");
    assert!(!code.contains("$JOIN_2") && code.contains("$LOOP_1 = a[i];"), "{code}");
}

#[test]
fn keeps_a_copy_when_the_local_is_read_after_the_copy_changes() {
    let source = "function f() { let $LOOP_1 = 1; let $JOIN_2 = $LOOP_1; $JOIN_2 = 5; const old = $LOOP_1; $LOOP_1 = $JOIN_2; return [old, $LOOP_1]; }";
    assert_eq!(merged(source).1, 0);
}

#[test]
fn merges_a_join_every_arm_stores_into_its_source() {
    let source = "function f() { const out = []; let $LOOP_1 = 2; let $LOOP_2; if (out.length === 0) { $LOOP_2 = $LOOP_1 + 1; out[$LOOP_2] = 5; } else { $LOOP_2 = $LOOP_1; } return [$LOOP_2, out]; }";
    let (code, count) = merged(source);
    assert_eq!(count, 1, "{code}");
    assert!(!code.contains("$LOOP_2"), "{code}");
    let kept = "function f() { let $LOOP_1 = 2; let $LOOP_2; if (g()) { $LOOP_2 = $LOOP_1 + 1; } return [$LOOP_2, $LOOP_1]; } function g() { return true; }";
    assert_eq!(merged(kept).1, 0);
}

#[test]
fn merges_into_an_outer_scratch_slot_its_function_always_stores_first() {
    let source = "function f() { let $KEY_8; const out = []; function g(n) { let $SSA_1 = Number(n) + 1; out[$SSA_1] = 1; $KEY_8 = $SSA_1; $KEY_8 = Number($SSA_1) + 1; out[$KEY_8] = 2; return $KEY_8; } g(0); return [g(2), out]; }";
    let (code, count) = merged(source);
    assert_eq!(count, 1, "{code}");
    assert!(!code.contains("$SSA_1"), "{code}");
    // read before stored: the value crosses calls
    let kept = "function f() { let $KEY_8 = 5; function g() { let $SSA_1 = $KEY_8 + 1; $KEY_8 = 0; return $SSA_1; } return [g(), g()]; }";
    assert_eq!(merged(kept).1, 0);
}

#[test]
fn keeps_a_value_copied_into_a_local_declared_later_inside() {
    let source = "function f() { const a = { x: 1 }; let $SSA_1 = a.x; if (a) { let $CARRY_2; $CARRY_2 = $SSA_1; return [$CARRY_2, $SSA_1]; } return $SSA_1; }";
    let (code, _) = merged(source);
    assert!(code.contains("let $CARRY_2") || !code.contains("$CARRY_2 ="), "{code}");
    assert!(!code.contains("$CARRY_2 = a.x"), "{code}");
}

#[test]
fn merges_joins_declared_together_and_stored_after_the_local_changes() {
    let source = "function f(n) { let $LOOP_1 = 0; let $LOOP_2 = 5; const out = []; for (let i = 0; i < n; i++) { let $JOIN_1, $JOIN_2; if (i % 2) { $JOIN_1 = $LOOP_1; $JOIN_2 = $LOOP_2; } else { for (let k = 0; k < 2; k++) { $LOOP_1 = $LOOP_1 + k; } $JOIN_1 = $LOOP_1; $JOIN_2 = i; } out.push($JOIN_1); $LOOP_1 = $JOIN_1; $LOOP_2 = $JOIN_2; } return [out, $LOOP_1, $LOOP_2]; }";
    let (code, count) = merged(source);
    assert_eq!(count, 2, "{code}");
    assert!(!code.contains("$JOIN_"), "{code}");
}

#[test]
fn splits_a_value_only_the_next_store_reads() {
    let source = "function f() { const a = { x: 3, y: 5 }; const g = (n) => n + 1; let v = g(a.x); v = 0 | v; let r = g(a.y); v = v | r << 1; return v; }";
    let result = super::split_single_reads(source).unwrap();
    assert_eq!(result.merged, 1, "{}", result.code);
    assert!(result.code.contains("let $SSA_s1 = 0 | v;") && result.code.contains("v = $SSA_s1 | r << 1;"), "{}", result.code);
    assert_eq!(run(source), run(&result.code));
}

#[test]
fn never_merges_a_counter_into_an_object_it_reads() {
    let source = "function f() { const out = []; let $OBJECT_1 = { p: 2 }; let i = 0; let $JOIN_2; if ($OBJECT_1.p == null) { $JOIN_2 = i + 1; } else { $JOIN_2 = $OBJECT_1.p + i; } out[$JOIN_2] = 1; return out; }";
    let (code, _) = merged(source);
    assert!(!code.contains("$OBJECT_1 = i"), "{code}");
}

#[test]
fn follows_a_break_to_reads_after_the_loop() {
    let source = "function f() { let i = 3; for (;;) { let $SSA_1 = i - 1; if ($SSA_1 > 0) { i = $SSA_1; continue; } break; } return i; }";
    let (code, count) = merged(source);
    assert_eq!(count, 0, "{code}");
}

#[test]
fn keeps_a_value_only_copied_into_a_local_nothing_reads_after() {
    // The VM parks the buffer in a register it never reads again.
    let source = "function f(a, g) {\n  var $KEY_11 = a;\n  if ($KEY_11 == null) {\n    g(1);\n  }\n  let $CARRY_1;\n  if (g(2)) {\n    let $ARRAY_1 = [1, 2];\n    $KEY_11 = $ARRAY_1;\n    $ARRAY_1[2] = g(3);\n    $KEY_11 = $ARRAY_1;\n    $CARRY_1 = $ARRAY_1;\n  } else {\n    $CARRY_1 = [3];\n  }\n  return $CARRY_1;\n}";
    let mut code = source.to_string();
    for _ in 0..8 {
        code = super::merge_derived_values(&code).unwrap().code;
    }
    // The carry, read after the join, may hold it; the register may not,
    // and its dead copies go.
    assert!(!code.contains("$KEY_11 = ["), "{code}");
    assert!(!code.contains("$KEY_11[2]"), "{code}");
    assert!(!code.contains("$KEY_11 = $"), "{code}");
    assert!(code.contains("if ($KEY_11 == null)"), "{code}");
}
