use shape::naming::name_threads;
const PROBE: &str = r#"function main(){
function $THREAD_1_7(){
if(typeof Proxy !== "function"){return false;}else{
let seen=false;let proto={};
function $THREAD_2_8(){seen=true;return proto;}
return Object.getPrototypeOf(new Proxy({},{getPrototypeOf:$THREAD_2_8}))===proto && seen;
}}
return $THREAD_1_7;
}main();"#;
fn roles(source: &str) -> Vec<String> {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .filter(|o| o.applied && o.evaluator.as_deref() == Some("shared_browser_proxy_probe"))
        .filter_map(|o| o.role_name.clone())
        .collect()
}
#[test]
fn proxy_and_exact_registered_trap_variants() {
    for source in [
        PROBE.to_string(),
        PROBE
            .replace("seen", "flag")
            .replace("proto", "sentinel")
            .replace("$THREAD_1_7", "$THREAD_90_900_2")
            .replace("$THREAD_2_8", "$THREAD_91_999"),
        PROBE
            .replace(
                "let seen=false;let proto={};",
                "const proto={};let seen=false;",
            )
            .replace(
                "const proto={};let seen=false;",
                "let seen=false;const proto={};",
            ),
    ] {
        let found = roles(&source);
        assert_eq!(found.len(), 2, "{source}: {found:?}");
        assert!(found.contains(&"supportsProxyGetPrototypeOf".into()));
        assert!(found.contains(&"markProxyPrototypeTrap".into()));
    }
}
#[test]
fn rejects_wrong_bindings_effects_and_cross_callable_traps() {
    for (before, after) in [
        (
            "Object.getPrototypeOf(new Proxy({},",
            "Object.getPrototypeOf(...new Proxy({},",
        ),
        ("seen=true;", "other=true;"),
        ("return proto;", "return {};"),
        ("===proto && seen", "===other && seen"),
        ("&& seen", "&& other"),
        ("seen=false", "seen=true"),
        ("new Proxy({},", "new Proxy(proto,"),
        ("getPrototypeOf:$THREAD_2_8", "getPrototypeOf:other"),
        ("getPrototypeOf:$THREAD_2_8", "setPrototypeOf:$THREAD_2_8"),
        ("seen=true;return proto;", "seen=true;evil();return proto;"),
        ("return false;", "return true;"),
        ("typeof Proxy", "typeof Other"),
        ("function main(){", "function main(Proxy){"),
        ("function main(){", "function main(Object){"),
        ("let proto={};", "let proto={x:1};"),
        (
            "function $THREAD_2_8(){seen=true;return proto;}",
            "function donor(){function $THREAD_2_8(){seen=true;return proto;}return $THREAD_2_8;}",
        ),
        (
            "return Object.getPrototypeOf",
            "$THREAD_2_8=other;return Object.getPrototypeOf",
        ),
        (
            "let seen=false;",
            "let seen=false;function donor(){seen=true;}",
        ),
    ] {
        let mutant = PROBE.replace(before, after);
        assert_ne!(mutant, PROBE);
        assert!(roles(&mutant).is_empty(), "{mutant}");
    }
}
