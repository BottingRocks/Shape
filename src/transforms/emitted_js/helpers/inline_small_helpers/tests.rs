use super::{inline_remaining_helpers, inline_small_helpers};

#[test]
fn expands_one_line_helpers_at_their_calls() {
    let source = "function $VMHELPER_1(a) {\n  return a;\n}\nfunction $VMHELPER_2(a) {\n  return a[\"push\"];\n}\nfunction $VMHELPER_3(a, b, c, d) {\n  a[b] = c ^ d();\n}\nfunction f(o, $SSA_1, g) {\n  $VMHELPER_3(o, 1, $VMHELPER_1(g()), $SSA_1);\n  return $VMHELPER_2($VMHELPER_1(o.q)).call(o);\n}";
    let result = inline_small_helpers(source).unwrap();
    assert_eq!(result.expanded, 4, "{}", result.code);
    assert!(!result.code.contains("$VMHELPER_"), "{}", result.code);
    assert!(result.code.contains("(o)[(1)] = (((g()))) ^ ($SSA_1)();"), "{}", result.code);
    assert!(result.code.contains("((((o.q)))[\"push\"]).call(o)"), "{}", result.code);
}

#[test]
fn expands_value_helpers_in_statement_position() {
    let source = "function $VMHELPER_1(a, b, c) {\n  a[b] = c;\n  return a;\n}\nfunction g() {}\nfunction f(v) {\n  let $SSA_1 = $VMHELPER_1({}, \"k\", g());\n  return $SSA_1;\n}";
    let result = inline_small_helpers(source).unwrap();
    assert_eq!(result.expanded, 1, "{}", result.code);
    assert!(result.code.contains("let $SSA_x2 = (g());\n  let $SSA_x1 = ({});\n  $SSA_x1[(\"k\")] = $SSA_x2;\n  let $SSA_1 = ($SSA_x1);"), "{}", result.code);
    // inside an expression the same call stays
    let source = "function $VMHELPER_1(a, b, c) {\n  a[b] = c;\n  return a;\n}\nfunction g() {}\nfunction f(v) {\n  return g($VMHELPER_1({}, \"k\", v));\n}";
    assert_eq!(inline_small_helpers(source).unwrap().expanded, 0);
}

#[test]
fn reads_a_slot_argument_in_place_when_only_property_stores_come_first() {
    let source = "function $VMHELPER_475(a, b, c, d, e) {\n  c[d] = e;\n  b.call(a, c);\n}\nfunction f() {\n  $VMHELPER_475($KEY_19, $KEY_19[\"next\"], [[]], 1, $ARRAY_1);\n}\n";
    let result = inline_small_helpers(source).unwrap();
    assert_eq!(result.expanded, 1, "{}", result.code);
    assert!(!result.code.contains("$VMHELPER_475("), "{}", result.code);
}

#[test]
fn expands_empty_helpers_and_literal_factories() {
    let source = "function $VMHELPER_1() {}\nfunction $VMHELPER_2() {\n  return [false];\n}\nfunction f() {\n  $VMHELPER_1();\n  let x = $VMHELPER_2();\n  return x;\n}\n";
    let result = inline_small_helpers(source).unwrap();
    assert!(!result.code.contains("$VMHELPER_"), "{}", result.code);
    assert!(result.code.contains("let x = ([false]);"), "{}", result.code);
}

#[test]
fn hoists_a_nested_statement_helper_call_before_its_statement() {
    let source = "function $VMHELPER_375(a, b, c) {\n  a[b] = c;\n  a[2] = 0;\n  return a;\n}\nfunction f(g, r) {\n  g(r, 1, $VMHELPER_375([0], 1, 1), 3);\n}\n";
    let result = super::hoist_helper_calls(source).unwrap();
    assert_eq!(result.expanded, 1, "{}", result.code);
    assert!(result.code.contains("= $VMHELPER_375([0], 1, 1);\n  g(r, 1, $SSA_x"), "{}", result.code);
    // a call before it would run after the helper
    let source = "function $VMHELPER_375(a, b, c) {\n  a[b] = c;\n  a[2] = 0;\n  return a;\n}\nfunction f(g, r) {\n  g(r(), $VMHELPER_375([0], 1, 1));\n}\n";
    assert_eq!(super::hoist_helper_calls(source).unwrap().expanded, 0);
}

#[test]
fn exact_expansion_renames_the_helpers_own_bindings_at_each_call() {
    let source = "function f(a, b) {\n\tfunction vm_sub_1_1(s) {\n\t\tlet t = s[0] ^ s[1];\n\t\ts[0] = t << 7 | t >>> 25;\n\t\treturn [t, s];\n\t}\n\tvm_sub_1_1(a);\n\tvm_sub_1_1(b);\n}\n";
    let result = inline_remaining_helpers(source).unwrap();
    assert_eq!(result.expanded, 2, "{}", result.code);
    assert_eq!(
        result.code,
        // Arguments get temporaries in exact mode; forwarding folds them.
        "function f(a, b) {\n\tlet $SSA_x1 = (a);\n\tlet $SSA_x2 = $SSA_x1[0] ^ $SSA_x1[1];\n\t$SSA_x1[0] = $SSA_x2 << 7 | $SSA_x2 >>> 25;\n\tlet $SSA_x3 = (b);\n\tlet $SSA_x4 = $SSA_x3[0] ^ $SSA_x3[1];\n\t$SSA_x3[0] = $SSA_x4 << 7 | $SSA_x4 >>> 25;\n}\n"
    );
}

#[test]
fn keeps_calls_whose_arguments_would_reorder() {
    for source in [
        // the second computed argument is read first
        "function $VMHELPER_1(a, b) {\n  return b + a;\n}\nfunction f(x, y) {\n  return [$VMHELPER_1(x(), y())];\n}",
        // a computed argument read twice
        "function $VMHELPER_1(a) {\n  return a + a;\n}\nfunction f(x) {\n  return [$VMHELPER_1(x())];\n}",
        // the body runs code before reading a computed argument
        "function $VMHELPER_1(a, b) {\n  return a() + b;\n}\nfunction f(x, y) {\n  return [$VMHELPER_1(x, y())];\n}",
        // an unused effectful argument
        "function $VMHELPER_1(a) {\n  return 1;\n}\nfunction f(x) {\n  return [$VMHELPER_1(x())];\n}",
    ] {
        let result = inline_small_helpers(source).unwrap();
        assert_eq!(result.expanded, 0, "{source} -> {}", result.code);
    }
}

#[test]
fn hoists_a_store_and_return_helper_out_of_a_store_to_the_array_it_is_handed() {
    // The VM's prefix-byte helper: store one byte, hand back the seed.
    let source = "function f(x) {\n\tfunction vm_sub_1_1(a, b, c, d, e) {\n\t\ta[b] = c & d;\n\t\treturn e;\n\t}\n\tlet o = [x >>> 24, x >> 16 & 255];\n\to[3] = vm_sub_1_1(o, 2, 255, x >> 8, x) & 255;\n\treturn o;\n}\n";
    let hoisted = super::hoist_remaining_helper_calls(source).unwrap();
    assert_eq!(hoisted.expanded, 1, "{}", hoisted.code);
    assert!(hoisted.code.contains("= vm_sub_1_1(o, 2, 255, x >> 8, x);\n\to[3] = $SSA_x"), "{}", hoisted.code);
    let expanded = inline_remaining_helpers(&hoisted.code).unwrap();
    assert!(!expanded.code.contains("vm_sub_1_1"), "{}", expanded.code);
    // A reassigned object may name another array once the call has run:
    // the store's object is bound first.
    let source = "function f(x, g) {\n\tfunction vm_sub_1_1(a, b, c, d, e) {\n\t\ta[b] = c & d;\n\t\treturn e;\n\t}\n\tlet o = [x];\n\to = g(o);\n\to[3] = vm_sub_1_1(o, 2, 255, x >> 8, x) & 255;\n\treturn o;\n}\n";
    let code = expand_exactly(source);
    assert!(!code.contains("vm_sub_1_1"), "{code}");
    let run = "console.log(JSON.stringify(f(70000, (o) => o.concat([1]))));";
    assert_eq!(observe(source, run), observe(&code, run));
    // A computed key is a read the call could change.
    let source = "function f(x) {\n\tfunction vm_sub_1_1(a, b, c, d, e) {\n\t\ta[b] = c & d;\n\t\treturn e;\n\t}\n\tlet o = [x];\n\to[o[0]] = vm_sub_1_1(o, 0, 255, x >> 8, x) & 255;\n\treturn o;\n}\n";
    assert_eq!(super::hoist_remaining_helper_calls(source).unwrap().expanded, 0);
}

/// Hoist and expand exactly until nothing changes, as `late_helpers` does.
fn expand_exactly(source: &str) -> String {
    let mut code = source.to_string();
    for _ in 0..8 {
        let hoisted = super::hoist_remaining_helper_calls(&code).unwrap();
        let exact = inline_remaining_helpers(&hoisted.code).unwrap();
        code = exact.code;
        if exact.expanded == 0 && hoisted.expanded == 0 {
            break;
        }
    }
    code
}

#[test]
fn hoists_nested_expression_helpers_whose_arguments_need_temporaries() {
    // The body converts `d ^ e` before reading `f` and `g`: the inner call
    // must run first, so neither call expands in place. The code ahead of
    // the calls (`k(1)`) is bound first, then the calls inside-out.
    let source = "function f(k, p, x, y, z, w) {\n\tfunction vm_sub_1_1(a, b, c, d, e, f, g) {\n\t\treturn a(b, c + (d ^ e), f, g);\n\t}\n\tlet r = k(1)(\"s\", w, vm_sub_1_1(p, x, y << 8, z & 255, y, w, vm_sub_1_1(p, x, z << 8, y & 255, z, w, 3)));\n\treturn r;\n}\n";
    let code = expand_exactly(source);
    assert!(!code.contains("vm_sub_1_1"), "{code}");
    let first = code.find("= k(1);").expect(&code);
    let inner = code.find("(z) << 8").or_else(|| code.find("z << 8")).expect(&code);
    let outer = code.find("(y) << 8").or_else(|| code.find("y << 8")).expect(&code);
    assert!(first < inner && inner < outer, "{code}");
    // A name a call could change, read before the code to bind, is bound
    // too, in order.
    let source = "function f(k, p, x, y, z, w) {\n\tfunction vm_sub_1_1(a, b, c, d, e, f, g) {\n\t\treturn a(b, c + (d ^ e), f, g);\n\t}\n\ty = k;\n\tlet r = k(y, k(1), vm_sub_1_1(p, x, y << 8, z & 255, y, w, 3));\n\treturn r;\n}\n";
    let code = expand_exactly(source);
    assert!(!code.contains("vm_sub_1_1"), "{code}");
    let run = "const log = []; const k = (...a) => { log.push(a.length); return a.map(String); }; console.log(JSON.stringify([f(k, (...a) => a, 1, 2, 3, 4), log]));";
    assert_eq!(observe(source, run), observe(&code, run));
    // The non-exact round leaves them alone.
    let source = "function f(p, x, y, z, w) {\n\tfunction vm_sub_1_1(a, b, c, d, e, f, g) {\n\t\treturn a(b, c + (d ^ e), f, g);\n\t}\n\treturn [vm_sub_1_1(p, x, y << 8, z & 255, y, w, p())];\n}\n";
    assert_eq!(super::hoist_helper_calls(source).unwrap().expanded, 0);
}

/// What `run` prints before and after, run by node.
fn observe(code: &str, run: &str) -> String {
    let script = format!("{code}\n{run}");
    let output = std::process::Command::new("node").arg("-e").arg(&script).output().unwrap();
    assert!(output.status.success(), "{script}\n{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn splits_a_conditional_store_so_the_helper_in_one_arm_expands_there() {
    // `x = c ? null : H(…)`: the statement becomes an `if`, each arm a store.
    let source = "function f(c, log) {\n\tfunction vm_sub_1_1(a, b) {\n\t\ta[b] = 248;\n\t\ta[7] = 127;\n\t\treturn a;\n\t}\n\tlet x;\n\tx = (log.push(\"test\"), c) ? null : vm_sub_1_1([log.push(\"arg\"), 0], 6);\n\treturn x;\n}\n";
    let code = expand_exactly(source);
    assert!(!code.contains("vm_sub_1_1"), "{code}");
    assert!(!code.contains("=>"), "{code}");
    assert!(code.contains("if (log.push(\"test\"), c) {\n\t\tx = null;\n\t} else {"), "{code}");
    let run = "for (const c of [true, false]) { const log = []; console.log(JSON.stringify([f(c, log), log])); }";
    assert_eq!(observe(source, run), observe(&code, run));
}

#[test]
fn moves_a_nested_conditional_out_and_splits_it_with_a_written_parameter() {
    // The parameter the body writes gets a binding of its own; the test runs
    // first, the other arm is a plain value.
    let source = "function f(k, x) {\n\tfunction vm_sub_1_1(a, b) {\n\t\ta = new a(1);\n\t\ta[0] = b;\n\t\treturn a;\n\t}\n\tk = new Uint8Array((k[0] !== x ? vm_sub_1_1(Float64Array, x) : k).buffer);\n\treturn Array.from(k);\n}\n";
    let code = expand_exactly(source);
    assert!(!code.contains("vm_sub_1_1"), "{code}");
    assert!(!code.contains("Float64Array ="), "{code}");
    let run = "console.log(JSON.stringify([f(new Float32Array([0.5]), 0.5), f(new Float32Array([0.1]), 0.1)]));";
    assert_eq!(observe(source, run), observe(&code, run));
}

#[test]
fn splits_conditional_and_logical_values_of_declarations_returns_and_statements() {
    let source = "function f(c, log) {\n\tfunction vm_sub_1_1(a, b) {\n\t\tlog.push(a);\n\t\treturn b;\n\t}\n\tlet a = c && vm_sub_1_1(\"and\", 1);\n\tlet b = c || vm_sub_1_1(\"or\", 2);\n\tlet d = c ?? vm_sub_1_1(\"nullish\", 3);\n\tc ? vm_sub_1_1(\"then\", 4) : vm_sub_1_1(\"else\", 5);\n\tc && vm_sub_1_1(\"guard\", 6);\n\treturn [a, b, d, c ? vm_sub_1_1(\"returned\", 7) : 8];\n}\nfunction g(c, log) {\n\tfunction vm_sub_1_2(a, b) {\n\t\tlog.push(a);\n\t\treturn b;\n\t}\n\treturn c ? vm_sub_1_2(\"then\", 1) : vm_sub_1_2(\"else\", 2);\n}\n";
    let code = expand_exactly(source);
    assert!(!code.contains("vm_sub_1_"), "{code}");
    let run = "for (const c of [0, 1, null]) { const log = []; console.log(JSON.stringify([f(c, log), g(c, log), log])); }";
    assert_eq!(observe(source, run), observe(&code, run));
}

#[test]
fn keeps_a_declared_name_a_closure_reads_out_of_the_split() {
    // `x` is read by a closure, so it is not assigned in the arms; the
    // choice moves into a fresh temporary instead.
    let source = "function f(c, log) {\n\tfunction vm_sub_1_1(a) {\n\t\tlog.push(a);\n\t\treturn a;\n\t}\n\tlet x = c ? vm_sub_1_1(1) : 2;\n\tlet read = () => x;\n\treturn read();\n}\n";
    let code = expand_exactly(source);
    assert!(!code.contains("vm_sub_1_1"), "{code}");
    assert!(code.contains("let x = $SSA_x"), "{code}");
    let run = "for (const c of [0, 1]) { const log = []; console.log(JSON.stringify([f(c, log), log])); }";
    assert_eq!(observe(source, run), observe(&code, run));
}

#[test]
fn leaves_a_helper_inside_an_optional_chain() {
    let source = "function f(o) {\n\tfunction vm_sub_1_1(a) {\n\t\ta.n = 1;\n\t\treturn a;\n\t}\n\tlet x = o?.m(vm_sub_1_1({}));\n\treturn x;\n}\n";
    assert_eq!(super::hoist_remaining_helper_calls(source).unwrap().expanded, 0);
}

#[test]
fn hoists_past_a_standard_namespace_method_and_another_positional_argument() {
    // `Object.defineProperty` reads the same function after the call;
    // `arguments[0].m` is not handed `arguments[1]`.
    let source = "function f(o, v) {\n\tfunction vm_sub_1_1(a, b, c, d, e, f, g, h) {\n\t\tf[g] = h;\n\t\td[e] = f;\n\t\treturn b.call(a, c, d);\n\t}\n\tObject[\"defineProperty\"](o, \"meta\", vm_sub_1_1(Object, Object.create, null, {}, \"value\", {}, \"value\", v));\n\treturn o.meta.value;\n}\nfunction g() {\n\tfunction vm_sub_1_2(a, b, c) {\n\t\treturn a + \": \" + c + b;\n\t}\n\treturn arguments[0][\"m\"](vm_sub_1_2(\"(\" + arguments[1], arguments[2], arguments[3]) + \")\");\n}\n";
    let code = expand_exactly(source);
    assert!(!code.contains("vm_sub_1_"), "{code}");
    let run = "console.log(JSON.stringify([f({}, 5), g({ m: (s) => s }, \"w\", \"x\", \"y\")]));";
    assert_eq!(observe(source, run), observe(&code, run));
    // A call handed the object whose method is read first stays put.
    let source = "function g() {\n\tfunction vm_sub_1_2(a, b) {\n\t\ta.m = b;\n\t\treturn 1;\n\t}\n\treturn arguments[0].m(vm_sub_1_2(arguments[0], arguments[1]));\n}\n";
    assert_eq!(super::hoist_remaining_helper_calls(source).unwrap().expanded, 0);
}

#[test]
fn moves_a_for_update_holding_a_helper_to_the_end_of_the_body() {
    // The update runs after the body and after every `continue` to the loop
    // (a `continue` of the inner loop stays its own); a `let` loop without
    // closures moves its update too.
    let source = "function f(log) {\n\tfunction vm_sub_1_1(a, b, c, d, e, f, g) {\n\t\tb.call(a, c, d[e % f]);\n\t\treturn g;\n\t}\n\tlet o = { add(x, y) { log.push([x, y]); } };\n\tlet i = 0;\n\tfor (; i < 10; i = Number(vm_sub_1_1(o, o.add, i / 9, [\"a\", \"b\", \"c\"], i, 3, i)) + 1) {\n\t\tif (i % 4 == 1) {\n\t\t\tlog.push(\"skip\" + i);\n\t\t\tcontinue;\n\t\t}\n\t\tfor (let j = 0; j < 2; j++) {\n\t\t\tif (j) continue;\n\t\t\tlog.push(\"j\");\n\t\t}\n\t\tlog.push(\"body\" + i);\n\t}\n\tfor (let k = 0; k < 3; k = Number(vm_sub_1_1(o, o.add, k, [\"x\"], 0, 1, k)) + 1) {}\n\tfor (; i < 12; i = Number(vm_sub_1_1(o, o.add, i, [\"y\"], 0, 1, i)) + 1) log.push(\"one\" + i);\n\treturn i;\n}\n";
    let code = expand_exactly(source);
    assert!(!code.contains("vm_sub_1_1"), "{code}");
    assert!(code.contains("break iteration;"), "{code}");
    assert!(code.contains("if (j) continue;"), "{code}");
    let run = "const log = []; console.log(JSON.stringify([f(log), log]));";
    assert_eq!(observe(source, run), observe(&code, run));
}

#[test]
fn keeps_the_update_of_a_let_loop_a_closure_captures() {
    // Moved into the body, the update would run before the per-iteration
    // copy of `k`, and the closures would see it.
    let source = "function f(log) {\n\tfunction vm_sub_1_1(a, b) {\n\t\tlog.push(a);\n\t\treturn b;\n\t}\n\tlet read = [];\n\tfor (let k = 0; k < 3; k = Number(vm_sub_1_1(\"u\", k)) + 1) {\n\t\tread.push(() => k);\n\t}\n\treturn read.map((r) => r());\n}\n";
    assert_eq!(super::hoist_remaining_helper_calls(source).unwrap().expanded, 0);
}

#[test]
fn turns_loop_tests_holding_a_helper_into_a_break() {
    // `do … while (!s.done && (i = sink, H(…), true))`: a `continue` still
    // reaches the test, the helper runs only when `!s.done`, and the
    // sequence splits into statements. A `while` test holding an
    // expression helper whose arguments need temporaries is checked first
    // in each iteration; the body's own `n` keeps its block.
    let source = "function f(items, sink, log) {\n\tfunction vm_sub_1_2(a, b, c, d) {\n\t\tb.call(a, c[d]);\n\t}\n\tlet it = items[Symbol.iterator]();\n\tlet s;\n\tlet i;\n\tdo {\n\t\ts = it.next();\n\t\tlog.push(\"next\");\n\t\tif (s.value === 2) continue;\n\t} while (!s.done && (i = sink, vm_sub_1_2(i, i.push, s, \"value\"), true));\n\treturn sink;\n}\nfunction g(log) {\n\tfunction vm_sub_1_3(a, b, c) {\n\t\treturn c + b + a + \")\";\n\t}\n\tlet n = 3;\n\tlet w = 1;\n\twhile (!log.check(vm_sub_1_3(w + \":\", n, \"(\")) && n > 0 && (n = n - 1, w = w * 2, true)) {\n\t\tlet n = 0;\n\t\tlog.push(n);\n\t}\n\treturn [n, w];\n}\n";
    let code = expand_exactly(source);
    assert!(!code.contains("vm_sub_1_"), "{code}");
    assert!(!code.contains("=>"), "{code}");
    let run = "const log = []; log.check = (s) => { log.push(s); return s.length > 12; }; console.log(JSON.stringify([f([1, 2, 3], [], log), g(log), log]));";
    assert_eq!(observe(source, run), observe(&code, run));
}

#[test]
fn binds_reads_and_code_before_a_helper_call_in_order() {
    // `x = k(l)("s", x, H(…))`: `k(l)` and the read of `x` (which the
    // helper's arguments change) are bound first; `s = C[0] + H(C[1], …)`
    // binds the read `C[0]`; `P[0][1] = H(P[1], …)` binds the store's
    // object `P[0]`.
    let source = "function h(k, log) {\n\tfunction vm_sub_1_4(a, b, c, d, e, f, g) {\n\t\treturn a(b, c + (d ^ e), f, g);\n\t}\n\tlet x = 1;\n\tx = k(log)(\"s\", x, vm_sub_1_4(k, log, x << 8, x & 255, x, 2, (x = x + 5, x)));\n\treturn x;\n}\nfunction m(C) {\n\tfunction vm_sub_1_5(a, b, c, d, e, f) {\n\t\treturn a.charAt.call(b, c) + d.charAt(f) + d.charAt(e);\n\t}\n\tlet s = \"xyz\";\n\ts = C[0] + vm_sub_1_5(C[1], C[2], 1, s, 0, 2);\n\treturn s;\n}\nfunction p() {\n\tfunction vm_sub_1_6(a, b, c) {\n\t\ta[b] = c;\n\t\ta[1] = 42;\n\t\treturn a;\n\t}\n\tlet P = [[[53, 46]], []];\n\tP[0][1] = vm_sub_1_6(P[1], 0, 55);\n\treturn P;\n}\n";
    let code = expand_exactly(source);
    assert!(!code.contains("vm_sub_1_"), "{code}");
    let run = "const log = []; const k = (...a) => { log.push(a.length); return (...b) => b; }; const C = new Proxy([\"q\", \"ab\", \"cd\"], { get(t, key) { log.push(String(key)); return t[key]; } }); console.log(JSON.stringify([h(k, log), m(C), p(), log]));";
    assert_eq!(observe(source, run), observe(&code, run));
}
