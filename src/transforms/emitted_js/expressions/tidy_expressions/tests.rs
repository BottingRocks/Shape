use super::tidy_expressions;

#[test]
fn unwraps_meaningless_commas_and_extra_parentheses() {
    let source = "function f(o, k) {\n  let a = (0, o[\"p\"]);\n  g(((0, o.q)), ((a + 1)) * 2);\n  return ((0, o.r))[(0, k)];\n}";
    let result = tidy_expressions(source).unwrap();
    assert_eq!(result.unwrapped, 4, "{}", result.code);
    assert!(result.code.contains("let a = o[\"p\"];"), "{}", result.code);
    assert!(result.code.contains("g(o.q, (a + 1) * 2);"), "{}", result.code);
    assert!(result.code.contains("return o.r[k];"), "{}", result.code);
}

#[test]
fn drops_empty_statements_from_lists_only() {
    let result = tidy_expressions("function f(c) {\n  ;\n  if (c) {\n    ;\n    g();\n  } else;\n  for (;;);\n}").unwrap();
    assert_eq!(result.code.matches(';').count(), 5, "{}", result.code);
    assert!(result.code.contains("else;"), "{}", result.code);
}

#[test]
fn drops_a_local_test_with_identical_arms() {
    let result = super::resugar_expressions("function f($c, g) {\n  return [$c ? arguments[2] : arguments[2], g ? 1 : 1, $c ? g() : g(1)];\n}").unwrap();
    let flat: String = result.code.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(flat.contains("[ arguments[2], g ? 1 : 1, $c ? g() : g(1) ]"), "{}", result.code);
}

#[test]
fn folds_literals_drops_discarded_reads_and_steps_for_counters() {
    let source = "function f(a) {\n\ta[3 + 1 + 1] = 2;\n\targuments[0];\n\ttypeof ($JOIN_1 + $JOIN_1);\n\t$JOIN_1 + 1;\n\t\"\" + $SSA_2;\n\tg() + 1;\n\tfor (let k = 0; k < 3; k = Number(k) + 1) {}\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert!(code.contains("a[5] = 2;"), "{code}");
    assert!(!code.contains("arguments[0]") && !code.contains("$JOIN_1") && !code.contains("$SSA_2"), "{code}");
    assert!(code.contains("g() + 1;"), "{code}");
    assert!(code.contains("k++)"), "{code}");
    let code = super::tidy_final_expressions("if (a) {\n\tf();\n} else {\n\tif (b) {\n\t\tg();\n\t}\n}\n").unwrap().code;
    assert!(code.contains("} else if (b) {"), "{code}");
}

#[test]
fn folds_counter_steps_into_the_stores_they_index() {
    let source = "function f(a, g) {\n\tlet i = 0;\n\ti = i + 1;\n\ta[i] = g();\n\ta[i + 1] = g();\n\ti = i + 1;\n\tlet $ONCE_1 = i + 1;\n\ti = i + 1;\n\ta[$ONCE_1] = 2;\n\ta[i + 1] = i;\n\ti = i + 1;\n\treturn i;\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert_eq!(code.matches("a[++i] = ").count(), 3, "{code}");
    assert!(code.contains("a[i + 1] = i;\n\t++i;"), "{code}");
    assert!(!code.contains("$ONCE_1"), "{code}");
}

#[test]
fn folds_boolean_choices_into_logical_operators() {
    let source = "function f(a, b) {\n\treturn [(a >= 90 ? a <= 190 : false) ? 1 === b : false, a > 1 ? true : b, a ? b : false];\n}\n";
    let code = super::resugar_expressions(source).unwrap().code;
    assert!(code.contains("a >= 90 && a <= 190 && 1 === b"), "{code}");
    assert!(code.contains("a > 1 || b"), "{code}");
    assert!(code.contains("a ? b : false"), "{code}");
}

#[test]
fn writes_a_loop_that_exits_first_with_its_test() {
    let source = "function f(a, i) {\n\tfor (;;) {\n\t\tif (!(i < 3)) break;\n\t\ti++;\n\t}\n\tfor (let k = 0;; k++) {\n\t\tif (k > a) break;\n\t}\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert!(code.contains("while (i < 3) {"), "{code}");
    assert!(code.contains("for (let k = 0; !(k > a); k++)"), "{code}");
}

#[test]
fn splices_blocks_that_declare_nothing() {
    let source = "function f(a) {\n\t{\n\t\ta();\n\t}\n\t{\n\t\tlet x = 1;\n\t\ta(x);\n\t}\n\t{\n\t\tlet y = 1;\n\t\ta(y);\n\t}\n\tlet y = 2;\n}\nfunction g(a) {\n\tlet x = 3;\n\treturn x;\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert!(code.starts_with("function f(a) {\n\ta();\n\tlet x = 1;\n\ta(x);\n\t{\n"), "{code}");
    let source = "function f(a) {\n\ta();\n\t{\n\t\tfunction g() {\n\t\t\treturn 1;\n\t\t}\n\t\ta(g);\n\t\tdocument;\n\t\t\"x\" + Math[\"random\"]()[\"toString\"](36)[\"slice\"](2);\n\t}\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert_eq!(code, "function f(a) {\n\ta();\n\tfunction g() {\n\t\treturn 1;\n\t}\n\ta(g);\n}\n");
}

#[test]
fn folds_deferred_counter_steps_into_their_stores() {
    let source = "function f(a, g) {\n\tlet i = 0;\n\ta[i + 1] = g();\n\tlet x = g();\n\ta[i + 1 + 1] = x;\n\ti = i + 1 + 1 + 1;\n\ta[i] = 5;\n\ta[i + 1 + 1] = 6;\n\treturn i;\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert_eq!(code.matches("a[++i]").count(), 3, "{code}");
    assert!(code.contains("a[i + 2] = 6;"), "{code}");
}

#[test]
fn joins_strings_flips_false_choices_and_drops_double_nots() {
    let source = "function f(s, x, g) {\n\treturn [s + \"/\" + 2 + \"?a=\", 1 | ((!x || !(typeof x === \"object\" || typeof x === \"function\")) ? false : g(x)) << 5];\n}\n";
    let resugared = super::resugar_expressions(source).unwrap().code;
    let code = super::tidy_final_expressions(&resugared).unwrap().code;
    assert!(code.contains("s + \"/2?a=\""), "{code}");
    assert!(code.contains("(x && (typeof x === \"object\" || typeof x === \"function\") && g(x)) << 5 | 1"), "{code}");
}

#[test]
fn starts_a_counter_one_lower_when_every_store_steps_it() {
    let source = "function f(a, g) {\n\tlet i = 4;\n\ta[i] = g();\n\ta[++i] = g();\n\treturn i;\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert!(code.contains("let i = 3;\n\ta[++i] = g();\n\ta[++i] = g();"), "{code}");
}

#[test]
fn folds_null_checked_member_steps_into_an_optional_chain() {
    let source = "function f(k) {\n\tlet $J_1 = k[\"H\"];\n\tlet $J_2;\n\tif (null == $J_1) {\n\t\t$J_2 = true;\n\t} else {\n\t\t$J_1 = $J_1[\"prototype\"];\n\t\t$J_2 = $J_1 == null;\n\t}\n\t$J_2 = $J_2 ? void 0 : $J_1[\"canPlayType\"];\n\treturn $J_2;\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert!(code.contains("let $J_2 = k[\"H\"]?.[\"prototype\"]?.[\"canPlayType\"];\n\treturn $J_2;"), "{code}");
}

#[test]
fn folds_a_repeated_null_checked_path_into_an_optional_chain() {
    let source = "function f(k) {\n\tlet $J_1, $J_2;\n\tif (k[\"H\"] == null) {\n\t\t$J_1 = true;\n\t\t$J_2 = k[\"H\"];\n\t} else {\n\t\t$J_2 = k[\"H\"][\"prototype\"];\n\t\t$J_1 = null == $J_2;\n\t}\n\t$J_1 = $J_1 ? void 0 : $J_2[\"canPlayType\"];\n\treturn $J_1;\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert!(code.contains("let $J_1 = k[\"H\"]?.[\"prototype\"]?.[\"canPlayType\"];\n\treturn $J_1;"), "{code}");
}

#[test]
fn drops_a_nan_guard_whose_other_arm_is_a_32_bit_integer() {
    let source = "function f(x, v) {\n\tx = x === x ? x ^ 1103515245 : 1103515245;\n\tlet y = x === x ? (v | 0) ^ x : v | 0;\n\treturn [x, y];\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert!(code.contains("x = x ^ 1103515245;"), "{code}");
    assert!(code.contains("let y = (v | 0) ^ x;"), "{code}");
    let node = |source: &str| {
        let output = std::process::Command::new("node")
            .args(["-e", &format!("{source};console.log(JSON.stringify([f(NaN, 7), f(5, 3e9), f(undefined, 2)]));")])
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap()
    };
    assert_eq!(node(source), node(&code));
}

#[test]
fn drops_a_nan_guard_a_bitwise_operator_absorbs() {
    let source = "function f(v, j, k) {\n\treturn [(j === j ? v ^ j : v) ^ k, j === j ? v ^ j : v];\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert!(code.contains("v ^ j ^ k"), "{code}");
    assert!(code.contains("j === j ? v ^ j : v"), "{code}");
}

#[test]
fn steps_stores_before_a_call_that_continues_from_them() {
    let source = "function f(a, g, w) {\n\tlet i = 0;\n\ti = w(a, i);\n\ta[i + 1] = g();\n\tlet j = 0;\n\ti = w(a, i + 1);\n\treturn i;\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert!(code.contains("a[++i] = g();\n\tlet j = 0;\n\ti = w(a, i);"), "{code}");
}

#[test]
fn normalizes_comparisons_and_negated_tests() {
    let source = "function f(a, b) {\n\tif (!(a !== \"x\")) {\n\t\tg(1);\n\t}\n\tif (!(null == a)) {\n\t\tg(2);\n\t} else {\n\t\tg(3);\n\t}\n\tif (!b) {\n\t\tg(4);\n\t} else {\n\t\tg(5);\n\t}\n\treturn [\"function\" === typeof a, 255 & b, !b ? 1 : 2, \"a\" + b];\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert!(code.contains("if (a === \"x\")"), "{code}");
    assert!(code.contains("if (a != null) {\n\t\tg(2);"), "{code}");
    assert!(code.contains("if (b) {\n\t\tg(5);\n\t} else {\n\t\tg(4);"), "{code}");
    assert!(code.contains("typeof a === \"function\",\n\t\tb & 255,\n\t\tb ? 2 : 1,\n\t\t\"a\" + b"), "{code}");
}

#[test]
fn reduces_a_typeof_check_on_a_property_path() {
    let source = "function f(g, v) {\n\treturn v | (!(F.p.t ? !(typeof F.p.t === \"object\" ? typeof F.p.t === \"object\" : typeof F.p.t === \"function\") : !F.p.t) && g(F.p.t)) << 2;\n}\n";
    let resugared = super::resugar_expressions(source).unwrap().code;
    let code = super::tidy_final_expressions(&resugared).unwrap().code;
    assert!(code.contains("v | (F.p.t && (typeof F.p.t === \"object\" || typeof F.p.t === \"function\") && g(F.p.t)) << 2"), "{code}");
}

#[test]
fn restructures_vm_loops_into_loops_with_their_exit_after() {
    let source = "function f(a, n) {\n\tlet i = -1, x = 5, z = n, b;\n\tvm_loop_1: for (;;) {\n\t\ta[++i] = x | 128;\n\t\tlet y = z & 127;\n\t\tif (z >>> 7 > 0) {\n\t\t\tz = z >>> 7;\n\t\t\tx = y;\n\t\t\tcontinue vm_loop_1;\n\t\t}\n\t\tb = y + i;\n\t\tbreak vm_loop_1;\n\t}\n\tvm_loop_2: for (;;) {\n\t\tif (i - 1 >= 0) {\n\t\t\ta[i] = 0;\n\t\t\t--i;\n\t\t\tcontinue vm_loop_2;\n\t\t} else {\n\t\t\treturn [a, b];\n\t\t}\n\t}\n}\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert!(!code.contains("vm_loop"), "{code}");
    assert!(code.contains("while (i - 1 >= 0) {"), "{code}");
    assert!(code.contains("let y;"), "{code}");
    let run = |source: &str| {
        let output = std::process::Command::new("node")
            .args(["-e", &format!("{source};console.log(JSON.stringify(f([], 300)));")])
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap()
    };
    assert_eq!(run(source), run(&code), "{code}");
}

#[test]
fn writes_busy_returned_choices_as_if_else_and_paths_as_optional_chains() {
    let source = "function f(a, g, i) {\n\tlet v = (a.b.c == null ? void 0 : a.b.c[\"d\"][\"e\"]) != null;\n\treturn i !== a.length ? g(i) | 0 : i;\n}\n";
    let resugared = super::resugar_expressions(source).unwrap().code;
    let code = super::tidy_final_expressions(&resugared).unwrap().code;
    assert!(code.contains("a.b.c?.[\"d\"][\"e\"] != null"), "{code}");
    assert!(code.contains("if (i !== a.length) {\n\t\treturn g(i) | 0;\n\t} else {\n\t\treturn i;"), "{code}");
}

#[test]
fn writes_counter_steps_as_updates() {
    let source = "a = Number(a) + 1;\nb = Number(b) - 1;\nc = c - 1;\nd = d + 1;\ne = Number(f) + 1;\ng = Number(g - 1);\n";
    let code = super::tidy_final_expressions(source).unwrap().code;
    assert_eq!(code, "++a;\n--b;\n--c;\n++d;\ne = Number(f) + 1;\n--g;\n");
}

#[test]
fn drops_an_inert_test_with_identical_arms() {
    let source = "function f() {\n\treturn [null == $KEY_1 ? arguments[0] : arguments[0], \"function\" === typeof g ? 1 : 1, h() ? 2 : 2];\n}\n";
    let code = super::resugar_expressions(source).unwrap().code;
    assert!(code.contains("arguments[0],\n\t\t1,\n\t\th() ? 2 : 2"), "{code}");
}

#[test]
fn keeps_a_loop_exit_that_another_break_skips() {
    // findIndex: the not-found break must not run the found exit.
    let source = "function f(a, g) {\n\tlet i = 0;\n\tlet r;\n\tfor (;;) {\n\t\tr = void 0;\n\t\tif (!(i < a.length)) {\n\t\t\tr = -1;\n\t\t\tbreak;\n\t\t}\n\t\tif (!g(a[i])) {\n\t\t\ti = i + 1;\n\t\t\tcontinue;\n\t\t}\n\t\tr = i;\n\t\tbreak;\n\t}\n\treturn r;\n}";
    let code = super::tidy_final_expressions(source).unwrap().code;
    let flat: String = code.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(!flat.contains("} r = i; return r;"), "{code}");
    assert!(flat.contains("r = i; break;"), "{code}");
}

#[test]
fn drops_empty_arms() {
    let result = tidy_expressions("function f(c, d) {\n  if (c) {\n    g();\n  } else {}\n  if (c && d) {} else {\n    g();\n  }\n}").unwrap();
    let flat: String = result.code.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(flat.contains("if (c) { g(); } if (!(c && d)) { g(); }"), "{}", result.code);
}

#[test]
fn resugars_method_calls_and_reused_tests() {
    let source = "function f(o, c, b) {\n  g(o[\"m\"].call(o, 1), o.n.call(o));\n  return [c ? c : b, c ? b : c, c == null ? b : c, null == c ? void 0 : c.p, c != null ? c[1] : void 0];\n}";
    let result = super::resugar_expressions(source).unwrap();
    assert_eq!(result.unwrapped, 7, "{}", result.code);
    assert!(result.code.contains("g(o[\"m\"](1), o.n());"), "{}", result.code);
    let wrapped = super::resugar_expressions("function f(o, e) {\n  (0, o[\"error\"]).call(o, e);\n}").unwrap();
    assert!(wrapped.code.contains("o[\"error\"](e);"), "{}", wrapped.code);
    assert!(result.code.contains("return [\n\t\tc || b,") || result.code.contains("[c || b, c && b, c ?? b, c?.p, c?.[1]]"), "{}", result.code);
    // a different receiver stays a .call
    let other = super::resugar_expressions("function f(o, p) {\n  return o.m.call(p);\n}").unwrap();
    assert_eq!(other.unwrapped, 0, "{}", other.code);
    let tests = super::resugar_expressions("function f(a, b) {\n  return [a === false ? b === false : a === false, a == null ? a == null : b, a.p ? b : a.p];\n}").unwrap();
    // a property path reads the same twice (no getters)
    assert_eq!(tests.unwrapped, 3, "{}", tests.code);
    assert!(tests.code.contains("a === false && b === false"), "{}", tests.code);
    assert!(tests.code.contains("a.p && b"), "{}", tests.code);
    assert!(tests.code.contains("a == null || b"), "{}", tests.code);
}

#[test]
fn reads_a_repeated_comparison_as_a_logical() {
    let source = "function f(a) {\n  let x;\n  do {\n    x = a[\"pop\"]();\n  } while (!(x > 0 ? x < 1 : x > 0));\n  return x <= 2 ? x <= 2 : a;\n}";
    let result = super::resugar_expressions(source).unwrap();
    assert!(result.code.contains("while (!(x > 0 && x < 1));"), "{}", result.code);
    assert!(result.code.contains("return x <= 2 || a;"), "{}", result.code);
}

#[test]
fn resugars_method_calls_on_receiver_paths() {
    let source = "function f() {\n  arguments[1][\"push\"].call(arguments[1], 2);\n  this[\"pipe\"].call(this, 3);\n  return this[\"a\"][\"m\"][\"call\"](this[\"a\"]);\n}";
    let result = super::resugar_expressions(source).unwrap();
    assert_eq!(result.unwrapped, 3, "{}", result.code);
    assert!(result.code.contains("arguments[1][\"push\"](2);"), "{}", result.code);
    assert!(result.code.contains("this[\"pipe\"](3);"), "{}", result.code);
    assert!(result.code.contains("return this[\"a\"][\"m\"]();"), "{}", result.code);
    for source in [
        // another element of the arguments
        "function f() {\n  return arguments[1][\"push\"].call(arguments[2], 2);\n}",
        // a computed key is not the same path
        "function f(k) {\n  return this[k].m.call(this[k]);\n}",
        // a call in the path runs twice
        "function f(g) {\n  return g().m.call(g());\n}",
    ] {
        let result = super::resugar_expressions(source).unwrap();
        assert_eq!(result.unwrapped, 0, "{source} -> {}", result.code);
    }
}

#[test]
fn resugars_a_method_call_on_a_comma_wrapped_member() {
    let source = "function f(o) {\n  (0, o[\"m\"]).call(o, 1);\n  return (0, o.n).call(o);\n}";
    let result = super::resugar_expressions(source).unwrap();
    assert_eq!(result.unwrapped, 2, "{}", result.code);
    assert!(result.code.contains("o[\"m\"](1);"), "{}", result.code);
    assert!(result.code.contains("return o.n();"), "{}", result.code);
    for source in [
        // the receiver must be the member's object
        "function f(o, p) {\n  return (0, o.n).call(p);\n}",
        // a comma head with an effect is not droppable
        "function f(o) {\n  return (g(), o.n).call(o);\n}",
        // an optional member is a different call
        "function f(o) {\n  return (0, o?.n).call(o);\n}",
        // three sequence elements
        "function f(o, x) {\n  return (0, x, o.n).call(o);\n}",
        // a bare identifier is called without a receiver
        "function f(o) {\n  return (0, o.n).call(null);\n}",
    ] {
        let result = super::resugar_expressions(source).unwrap();
        assert_eq!(result.unwrapped, 0, "{source} -> {}", result.code);
    }
}

#[test]
fn keeps_commas_that_change_meaning() {
    let source = "function f(o) {\n  (0, o.m)(1);\n  (0, o.t)`x`;\n  delete (0, o.p);\n  (0, setTimeout)(o, 0);\n  return typeof (0, missing);\n}";
    let result = tidy_expressions(source).unwrap();
    assert_eq!(result.unwrapped, 2, "{}", result.code);
    assert!(result.code.contains("\n\tsetTimeout(o, 0);"), "{}", result.code);
    assert!(result.code.contains("o.m.call(void 0, 1)"), "{}", result.code);
    assert!(result.code.contains("(0, o.t)`x`"), "{}", result.code);
    assert!(result.code.contains("delete (0, o.p)"), "{}", result.code);
    assert!(result.code.contains("typeof (0, missing)"), "{}", result.code);
    // inside `with`, a name may be a property: its call would get `this`
    let with = tidy_expressions("with (o) {\n  (0, f)();\n}").unwrap();
    assert_eq!(with.unwrapped, 0, "{}", with.code);
}

#[test]
fn picks_a_slot_out_of_a_quiet_array_literal() {
    let code = super::tidy_expressions("function f($A, $B) {\n  return g([$A, $B][1], [$A, h()][0], [$A][2], [$A, !$A][0], [o.p, $A][0], [o.p, $A][1]);\n}").unwrap().code;
    assert!(code.contains("g($B, [$A, h()][0], [$A][2], $A, o.p, [o.p, $A][1])"), "{code}");
}
