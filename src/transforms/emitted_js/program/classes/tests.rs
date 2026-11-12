use super::*;

fn rebuild(source: &str) -> (String, usize) {
    rebuild_classes(source).unwrap()
}

#[test]
fn gathers_the_constructor_methods_and_statics() {
    let source = "function f() {\n\tfunction $THREAD_1() {\n\t\t\"use strict\";\n\t\tthis[\"a\"] = arguments[0];\n\t}\n\tfunction $THREAD_2() {\n\t\t\"use strict\";\n\t\treturn this[\"a\"];\n\t}\n\t$THREAD_1[\"prototype\"][\"get\"] = $THREAD_2;\n\tlet $KEY_1;\n\tfunction $THREAD_3() {\n\t\t\"use strict\";\n\t\treturn new $THREAD_1(1);\n\t}\n\t$THREAD_1[\"make\"] = $THREAD_3;\n\tlet $KEY_2 = $THREAD_1;\n\treturn new $KEY_2(2);\n}\n";
    let (code, count) = rebuild(source);
    assert_eq!(count, 1, "{code}");
    assert_eq!(code, "function f() {\n\tclass $THREAD_1 {\n\t\tconstructor() {\n\t\t\tthis[\"a\"] = arguments[0];\n\t\t}\n\t\tget() {\n\t\t\treturn this[\"a\"];\n\t\t}\n\t\tstatic make() {\n\t\t\treturn new $THREAD_1(1);\n\t\t}\n\t}\n\tlet $KEY_1;\n\tlet $KEY_2 = $THREAD_1;\n\treturn new $KEY_2(2);\n}\n");
}

#[test]
fn follows_a_factory_and_its_copies() {
    let source = "function $THREAD_0() {\n\tfunction $THREAD_1() {\n\t\t\"use strict\";\n\t}\n\tlet $KEY_1 = $THREAD_1;\n\tfunction $THREAD_2() {\n\t\t\"use strict\";\n\t\treturn 1;\n\t}\n\t$KEY_1[\"prototype\"][\"one\"] = $THREAD_2;\n\treturn $KEY_1;\n}\nlet $KEY_9 = $THREAD_0();\nnew $KEY_9();\n";
    let (code, count) = rebuild(source);
    assert_eq!(count, 1, "{code}");
    assert!(code.contains("class $THREAD_1 {\n\t\tconstructor() {\n\t\t}\n\t\tone() {\n\t\t\treturn 1;\n\t\t}\n\t}\n\tlet $KEY_1 = $THREAD_1;\n\treturn $KEY_1;"), "{code}");
}

#[test]
fn keeps_functions_a_class_could_not_replace() {
    for source in [
        // called without new
        "function f() {\n\tfunction $THREAD_1() {\n\t\t\"use strict\";\n\t}\n\tfunction $THREAD_2() {\n\t\t\"use strict\";\n\t}\n\t$THREAD_1[\"prototype\"][\"m\"] = $THREAD_2;\n\t$THREAD_1();\n}\n",
        // handed to other code
        "function f(g) {\n\tfunction $THREAD_1() {\n\t\t\"use strict\";\n\t}\n\tfunction $THREAD_2() {\n\t\t\"use strict\";\n\t}\n\t$THREAD_1[\"prototype\"][\"m\"] = $THREAD_2;\n\tg($THREAD_1);\n}\n",
        // used before its declaration (hoisted)
        "function f() {\n\tlet x = new $THREAD_1();\n\tfunction $THREAD_1() {\n\t\t\"use strict\";\n\t}\n\tfunction $THREAD_2() {\n\t\t\"use strict\";\n\t}\n\t$THREAD_1[\"prototype\"][\"m\"] = $THREAD_2;\n}\n",
        // a sloppy method reading this
        "function f() {\n\tfunction $THREAD_1() {\n\t\t\"use strict\";\n\t}\n\tfunction $THREAD_2() {\n\t\treturn this;\n\t}\n\t$THREAD_1[\"prototype\"][\"m\"] = $THREAD_2;\n\treturn new $THREAD_1();\n}\n",
        // no members
        "function f() {\n\tfunction $THREAD_1() {\n\t\t\"use strict\";\n\t}\n\treturn new $THREAD_1();\n}\n",
    ] {
        let (code, count) = rebuild(source);
        assert_eq!(count, 0, "{source} -> {code}");
    }
}

#[test]
fn stops_at_code_that_could_see_the_class_half_built() {
    let source = "function f(g) {\n\tfunction $THREAD_1() {\n\t\t\"use strict\";\n\t}\n\tfunction $THREAD_2() {\n\t\t\"use strict\";\n\t}\n\t$THREAD_1[\"prototype\"][\"a\"] = $THREAD_2;\n\tg();\n\tfunction $THREAD_3() {\n\t\t\"use strict\";\n\t}\n\t$THREAD_1[\"prototype\"][\"b\"] = $THREAD_3;\n\treturn new $THREAD_1();\n}\n";
    let (code, count) = rebuild(source);
    assert_eq!(count, 1);
    assert!(code.contains("\t\ta() {\n\t\t}\n\t}\n\tg();\n\tfunction $THREAD_3()"), "{code}");
    assert!(code.contains("$THREAD_1[\"prototype\"][\"b\"] = $THREAD_3;"), "{code}");
}
#[test]
fn reads_an_inlined_create_class_block_as_accessors() {
    let loop_ = "\tfor (let i = 0; i < $P[\"length\"]; i++) {\n\t\t$P[i][\"enumerable\"] = $P[i][\"enumerable\"] || false;\n\t\t$P[i][\"configurable\"] = true;\n\t\tif (\"value\" in $P[i]) {\n\t\t\t$P[i][\"writable\"] = true;\n\t\t}\n\t\tlet $Q;\n\t\tif (typeof $P[i][\"key\"] != \"object\" || !$P[i][\"key\"]) {\n\t\t\t$Q = $P[i][\"key\"];\n\t\t} else if ($P[i][\"key\"][Symbol[\"toPrimitive\"]] !== void 0) {\n\t\t\t$Q = $P[i][\"key\"][Symbol[\"toPrimitive\"]][\"call\"]($P[i][\"key\"], \"string\");\n\t\t\tif (typeof $Q == \"object\") {\n\t\t\t\tthrow new TypeError(\"@@toPrimitive must return a primitive value.\");\n\t\t\t}\n\t\t} else {\n\t\t\t$Q = String($P[i][\"key\"]);\n\t\t}\n\t\tObject[\"defineProperty\"]($THREAD_1[\"prototype\"], typeof $Q == \"symbol\" ? $Q : String($Q), $P[i]);\n\t}\n";
    let source = format!("function f() {{\n\tfunction $THREAD_1() {{\n\t\t\"use strict\";\n\t}}\n\tfunction $THREAD_2() {{\n\t\t\"use strict\";\n\t\treturn 1;\n\t}}\n\t$THREAD_1[\"prototype\"][\"m\"] = $THREAD_2;\n\tlet $P = {{ [\"key\"]: \"closed\" }};\n\tfunction $THREAD_3() {{\n\t\t\"use strict\";\n\t\treturn 2;\n\t}}\n\t$P[\"get\"] = $THREAD_3;\n\t$P = [$P];\n{loop_}\tObject[\"defineProperty\"]($THREAD_1, \"prototype\", {{ [\"writable\"]: false }});\n\tlet $KEY_1 = $THREAD_1;\n\treturn new $KEY_1();\n}}\n");
    let (code, count) = rebuild(&source);
    assert_eq!(count, 1, "{code}");
    assert_eq!(code, "function f() {\n\tclass $THREAD_1 {\n\t\tconstructor() {\n\t\t}\n\t\tm() {\n\t\t\treturn 1;\n\t\t}\n\t\tget closed() {\n\t\t\treturn 2;\n\t\t}\n\t}\n\tlet $KEY_1 = $THREAD_1;\n\treturn new $KEY_1();\n}\n");
}
#[test]
fn rebuilds_a_typescript_subclass_and_then_its_base() {
    let source = "let $KEY_376;\nfunction $THREAD_7() {\n\tlet $A;\n\tif (Object[\"setPrototypeOf\"]) {\n\t\t$A = Object[\"setPrototypeOf\"];\n\t} else {\n\t\tfunction $THREAD_8() {\n\t\t\targuments[0][\"__proto__\"] = arguments[1];\n\t\t\treturn arguments[0];\n\t\t}\n\t\t$A = $THREAD_8;\n\t}\n\t$KEY_376 = $A;\n\treturn $A(arguments[0], arguments[1]);\n}\n$KEY_376 = $THREAD_7;\nfunction $THREAD_1() {\n\t\"use strict\";\n\tthis[\"a\"] = 1;\n}\nfunction $THREAD_2() {\n\t\"use strict\";\n\treturn this[\"a\"];\n}\n$THREAD_1[\"prototype\"][\"get\"] = $THREAD_2;\nfunction $THREAD_3() {\n\tlet $KEY_0 = arguments[0];\n\tfunction $THREAD_4() {\n\t\t\"use strict\";\n\t\treturn $KEY_0[\"apply\"](this, arguments) || this;\n\t}\n\tlet $KEY_3 = $KEY_0;\n\t$THREAD_4[\"prototype\"] = Object[\"create\"]($KEY_3[\"prototype\"]);\n\t$THREAD_4[\"prototype\"][\"constructor\"] = $THREAD_4;\n\t$KEY_376($THREAD_4, $KEY_3);\n\tfunction $THREAD_5() {\n\t\t\"use strict\";\n\t\treturn $KEY_0[\"prototype\"][\"get\"][\"call\"](this) + 1;\n\t}\n\t$THREAD_4[\"prototype\"][\"get\"] = $THREAD_5;\n\treturn $THREAD_4;\n}\nlet $AF = $THREAD_3($THREAD_1);\nlet $KEY_9 = new $AF();\n";
    let (code, count) = rebuild(source);
    assert_eq!(count, 2, "{code}");
    assert!(code.contains("class $THREAD_1 {\n\tconstructor() {\n\t\tthis[\"a\"] = 1;\n\t}\n\tget() {\n\t\treturn this[\"a\"];\n\t}\n}\n"), "{code}");
    assert!(code.contains("\tclass $THREAD_4 extends $KEY_0 {\n\t\tconstructor() {\n\t\t\tsuper(...arguments);\n\t\t}\n\t\tget() {\n\t\t\treturn super.get() + 1;\n\t\t}\n\t}\n"), "{code}");
    assert!(!code.contains("Object[\"create\"]") && !code.contains("$KEY_376($THREAD_4"), "{code}");
    assert!(!code.contains("let $KEY_3 = $KEY_0;"), "{code}");
}
const CREATE_CLASS: &str = "function $THREAD_60() {\n\tfunction $THREAD_61() {\n\t\tif (arguments[1]) {\n\t\t\tlet j = 0;\n\t\t\tfor (; j < arguments[1][\"length\"]; j++) {\n\t\t\t\targuments[1][j][\"enumerable\"] = arguments[1][j][\"enumerable\"] || false;\n\t\t\t\targuments[1][j][\"configurable\"] = true;\n\t\t\t\tif (\"value\" in arguments[1][j]) {\n\t\t\t\t\targuments[1][j][\"writable\"] = true;\n\t\t\t\t}\n\t\t\t\tObject[\"defineProperty\"](arguments[0][\"prototype\"], arguments[1][j][\"key\"], arguments[1][j]);\n\t\t\t}\n\t\t}\n\t\tif (arguments[2]) {\n\t\t\tlet i = 0;\n\t\t\tfor (; i < arguments[2][\"length\"]; i++) {\n\t\t\t\targuments[2][i][\"enumerable\"] = arguments[2][i][\"enumerable\"] || false;\n\t\t\t\targuments[2][i][\"configurable\"] = true;\n\t\t\t\tif (\"value\" in arguments[2][i]) {\n\t\t\t\t\targuments[2][i][\"writable\"] = true;\n\t\t\t\t}\n\t\t\t\tObject[\"defineProperty\"](arguments[0], arguments[2][i][\"key\"], arguments[2][i]);\n\t\t\t}\n\t\t}\n\t\treturn arguments[0];\n\t}\n\treturn $THREAD_61;\n}\nlet $KEY_51 = $THREAD_60();\n";

fn babel_factory(tail: &str) -> String {
    format!("{CREATE_CLASS}function $THREAD_9() {{\n\tlet $KEY_5;\n\tfunction $THREAD_5() {{\n\t\t\"use strict\";\n\t\tif (!(this instanceof $KEY_5)) {{\n\t\t\tthrow new TypeError(\"Cannot call a class as a function\");\n\t\t}}\n\t\tif (arguments[0] == null) {{\n\t\t\tthrow new TypeError(\"x\");\n\t\t}}\n\t\tlet $J = ($KEY_5[\"__proto__\"] || Object[\"getPrototypeOf\"]($KEY_5))[\"call\"](this, 1);\n\t\tif (this) {{\n\t\t\t$J = $J && (typeof $J === \"object\" || typeof $J === \"function\") ? $J : this;\n\t\t\t$J[\"a\"] = arguments[0];\n\t\t\treturn $J;\n\t\t}} else {{\n\t\t\tthrow new ReferenceError(\"this hasn't been initialised - super() hasn't been called\");\n\t\t}}\n\t}}\n\t$KEY_5 = $THREAD_5;\n\tif (typeof arguments[0] !== \"function\" && arguments[0] !== null) {{\n\t\tthrow new TypeError(\"Super expression must either be null or a function, not \" + typeof arguments[0]);\n\t}} else {{\n\t\tlet $O = {{\n\t\t\t[\"value\"]: $THREAD_5,\n\t\t\t[\"enumerable\"]: false,\n\t\t\t[\"writable\"]: true,\n\t\t\t[\"configurable\"]: true\n\t\t}};\n\t\tlet $R = {{}};\n\t\t$R[\"constructor\"] = $O;\n\t\t$THREAD_5[\"prototype\"] = Object[\"create\"](arguments[0] && arguments[0][\"prototype\"], $R);\n\t\tif (arguments[0]) {{\n\t\t\tif (Object[\"setPrototypeOf\"]) {{\n\t\t\t\tObject[\"setPrototypeOf\"]($THREAD_5, arguments[0]);\n\t\t\t}} else {{\n\t\t\t\t$THREAD_5[\"__proto__\"] = arguments[0];\n\t\t\t}}\n\t\t}}\n\t\tlet $S = $KEY_5;\n{tail}\t\treturn $KEY_5;\n\t}}\n}}\nlet $KEY_9 = new ($THREAD_9(Object))(2);\n")
}

#[test]
fn rebuilds_a_babel_subclass() {
    let tail = "\t\tlet $D = { [\"key\"]: \"one\" };\n\t\tfunction $THREAD_6() {\n\t\t\t\"use strict\";\n\t\t\treturn this[\"a\"];\n\t\t}\n\t\t$D[\"value\"] = $THREAD_6;\n\t\t$D = [$D];\n\t\tlet $E = {\n\t\t\t[\"key\"]: \"two\",\n\t\t\t[\"value\"]: void 0\n\t\t};\n\t\tfunction $THREAD_7() {\n\t\t\t\"use strict\";\n\t\t\treturn 2;\n\t\t}\n\t\t$E[\"value\"] = $THREAD_7;\n\t\t$D[1] = $E;\n\t\tlet $H = { [\"key\"]: \"make\" };\n\t\tfunction $THREAD_8() {\n\t\t\treturn new $KEY_5(arguments[0]);\n\t\t}\n\t\t$H[\"value\"] = $THREAD_8;\n\t\t$KEY_51($S, $D, [$H]);\n";
    let (code, count) = rebuild(&babel_factory(tail));
    assert_eq!(count, 1, "{code}");
    assert!(code.contains("function $THREAD_9() {\n\tlet $KEY_5;\n\tclass $THREAD_5 extends arguments[0] {\n\t\tconstructor() {\n\t\t\tif (arguments[0] == null) {\n\t\t\t\tthrow new TypeError(\"x\");\n\t\t\t}\n\t\t\tsuper(1);\n\t\t\tthis[\"a\"] = arguments[0];\n\t\t}\n\t\tone() {\n\t\t\treturn this[\"a\"];\n\t\t}\n\t\ttwo() {\n\t\t\treturn 2;\n\t\t}\n\t\tstatic make() {\n\t\t\treturn new $KEY_5(arguments[0]);\n\t\t}\n\t}\n\t$KEY_5 = $THREAD_5;\n\treturn $KEY_5;\n}\n"), "{code}");
}

#[test]
fn keeps_a_babel_subclass_whose_descriptors_do_not_all_reach_the_call() {
    // $E is never put in the array: the block is not read whole.
    let tail = "\t\tlet $D = { [\"key\"]: \"one\" };\n\t\tfunction $THREAD_6() {\n\t\t\t\"use strict\";\n\t\t\treturn 1;\n\t\t}\n\t\t$D[\"value\"] = $THREAD_6;\n\t\t$D = [$D];\n\t\tlet $E = { [\"key\"]: \"two\" };\n\t\tfunction $THREAD_7() {\n\t\t\t\"use strict\";\n\t\t\treturn 2;\n\t\t}\n\t\t$E[\"value\"] = $THREAD_7;\n\t\t$KEY_51($S, $D);\n";
    let (code, count) = rebuild(&babel_factory(tail));
    assert_eq!(count, 0, "{code}");
}

/// A class whose static `m` is the sloppy thread `method`, stored after `of`.
fn class_with_sloppy_method(method: &str) -> String {
    format!("function f() {{\n\tfunction $THREAD_1() {{\n\t\t\"use strict\";\n\t}}\n\tfunction $THREAD_2() {{\n\t\t\"use strict\";\n\t\treturn 1;\n\t}}\n\t$THREAD_1[\"prototype\"][\"of\"] = $THREAD_2;\n{method}\t$THREAD_1[\"m\"] = $THREAD_3;\n\treturn new $THREAD_1();\n}}\n")
}

#[test]
fn moves_a_sloppy_method_whose_nested_helper_has_parameters() {
    // Target014's Observable `from`: a VM helper with parameters, its own
    // `arguments` unread, declared and read in a block of a sloppy method
    // reading only its own `arguments`.
    let method = "\tfunction $THREAD_3() {\n\t\tlet $KEY_0 = arguments[0];\n\t\tif ($KEY_0) {\n\t\t\tvm_sub_1($KEY_0, $KEY_0[\"next\"], $KEY_0, 0);\n\t\t\tfunction vm_sub_1($PARAMA, $PARAMB, $PARAMC, $PARAMD) {\n\t\t\t\t$PARAMB[\"call\"]($PARAMA, $PARAMC[$PARAMD]);\n\t\t\t}\n\t\t}\n\t}\n";
    let (code, count) = rebuild(&class_with_sloppy_method(method));
    assert_eq!(count, 1, "{code}");
    assert!(code.contains("\t\tof() {\n\t\t\treturn 1;\n\t\t}\n\t\tstatic m() {\n\t\t\tlet $KEY_0 = arguments[0];\n\t\t\tif ($KEY_0) {\n\t\t\t\tvm_sub_1($KEY_0, $KEY_0[\"next\"], $KEY_0, 0);\n\t\t\t\tfunction vm_sub_1($PARAMA, $PARAMB, $PARAMC, $PARAMD) {"), "{code}");
    assert!(!code.contains("$THREAD_1[\"m\"] = $THREAD_3;"), "{code}");
}

#[test]
fn keeps_a_sloppy_method_strict_mode_would_change() {
    for method in [
        // the helper reads the `arguments` its parameters alias
        "\tfunction $THREAD_3() {\n\t\tfunction vm_sub_1($PARAMA) {\n\t\t\t$PARAMA = 2;\n\t\t\treturn arguments[0];\n\t\t}\n\t\treturn vm_sub_1(1);\n\t}\n",
        // ...through an arrow
        "\tfunction $THREAD_3() {\n\t\tfunction vm_sub_1($PARAMA) {\n\t\t\t$PARAMA = 2;\n\t\t\treturn (() => arguments[0])();\n\t\t}\n\t\treturn vm_sub_1(1);\n\t}\n",
        // the method's own parameters, aliased
        "\tfunction $THREAD_3($PARAMA) {\n\t\t$PARAMA = 2;\n\t\treturn arguments[0];\n\t}\n",
        // a repeated parameter: a strict SyntaxError
        "\tfunction $THREAD_3() {\n\t\tfunction vm_sub_1($PARAMA, $PARAMA) {\n\t\t\treturn $PARAMA;\n\t\t}\n\t\treturn vm_sub_1(1, 2);\n\t}\n",
        // a function declared in a block, read outside it: sloppy code
        // binds it in the method too
        "\tfunction $THREAD_3() {\n\t\tif (arguments[0]) {\n\t\t\tfunction vm_sub_1() {\n\t\t\t\treturn 1;\n\t\t\t}\n\t\t}\n\t\treturn vm_sub_1;\n\t}\n",
        // a binding named `arguments` or `eval`: a strict SyntaxError
        "\tfunction $THREAD_3() {\n\t\tlet arguments = 1;\n\t\treturn arguments;\n\t}\n",
        "\tfunction $THREAD_3() {\n\t\tfunction vm_sub_1(eval) {\n\t\t\treturn 1;\n\t\t}\n\t\treturn vm_sub_1();\n\t}\n",
    ] {
        let (code, count) = rebuild(&class_with_sloppy_method(method));
        assert_eq!(count, 1, "{code}");
        assert!(code.contains("\t\tof() {\n\t\t\treturn 1;\n\t\t}\n\t}\n"), "{method} -> {code}");
        assert!(code.contains("$THREAD_1[\"m\"] = $THREAD_3;"), "{method} -> {code}");
    }
}
