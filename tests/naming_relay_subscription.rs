use shape::naming::name_threads;

fn named(source: &str, name: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| {
            o.applied
                && o.proposed_name.as_deref() == Some(name)
                && o.evaluator.as_deref() == Some("shared_frontier_relay_subscription")
        })
}

fn unchanged(source: &str) -> bool {
    name_threads(source, true).unwrap().source == source
}

// Portable main wrapper only. The callback input is deliberately opaque DATA:
// nothing here pretends a native or vendor-specific subscriber was proven.
const RELAY: &str = r#"function main(){
let registry=[];
let Ctor=function Observable(executor){executor({next(){}});};
function $THREAD_7_101_2(){
return new Ctor($THREAD_subscriber=>{
let channel=$THREAD_subscriber;
registry.push($THREAD_value=>channel.next($THREAD_value));
});
}
return $THREAD_7_101_2;
}
main();"#;

#[test]
fn relay_subscription_proves_owned_executor_and_relay_identities() {
    for source in [
        RELAY.to_string(),
        RELAY
            .replace("$THREAD_7_101_2", "$THREAD_999_878787_2")
            .replace("registry", "relays")
            .replace("Ctor", "ObservableType")
            .replace("channel", "relay")
            .replace("subscriber", "listener")
            .replace("value", "item"),
        RELAY.replace("let registry=[];", "let registry;registry=[];"),
        RELAY.replace(
            "let Ctor=function Observable(executor){executor({next(){}});};",
            "let Ctor;Ctor=function Observable(executor){executor({next(){}});};",
        ),
        RELAY
            .replace("let registry=[];", "const registry=[];")
            .replace("let Ctor=", "const Ctor=")
            .replace("let channel=", "const channel="),
        RELAY
            .replace("let registry=[];", "var registry=[];")
            .replace("let Ctor=", "var Ctor=")
            .replace("let channel=", "var channel="),
    ] {
        assert!(named(&source, "createRelaySubscription"), "{source}");
    }
}

#[test]
fn relay_subscription_rejects_changed_operations_and_callback_identity() {
    for (from, to) in [
        ("registry.push", "relays.push"),
        ("registry.push", "registry.unshift"),
        ("registry.push", "registry?.push"),
        ("channel.next", "channel.emit"),
        ("channel.next", "channel?.next"),
        ("channel.next($THREAD_value)", "channel.next(0)"),
        ("channel.next($THREAD_value)", "other.next($THREAD_value)"),
        ("let channel=$THREAD_subscriber;", "let channel=other;"),
        (
            "let channel=$THREAD_subscriber;",
            "let channel=$THREAD_subscriber;channel=other;",
        ),
        ("$THREAD_subscriber=>{", "($THREAD_subscriber=other)=>{"),
        ("$THREAD_subscriber=>{", "(...$THREAD_subscriber)=>{"),
        (
            "$THREAD_value=>channel.next",
            "($THREAD_value,$KEY_other)=>channel.next",
        ),
        (
            "registry.push($THREAD_value=>channel.next($THREAD_value));",
            "registry.push($THREAD_value=>channel.next($THREAD_value));cleanup();",
        ),
        (
            "let channel=$THREAD_subscriber;",
            "let channel=$THREAD_subscriber;$THREAD_subscriber.next(0);",
        ),
        (
            "let channel=$THREAD_subscriber;",
            "let channel=$THREAD_subscriber;let extra=channel;",
        ),
        ("return new Ctor(", "return Ctor("),
        (
            "return new Ctor($THREAD_subscriber",
            "return new Ctor(1,$THREAD_subscriber",
        ),
        (
            "$THREAD_value=>channel.next($THREAD_value)",
            "()=>channel.next(0)",
        ),
    ] {
        let source = RELAY.replace(from, to);
        assert!(!named(&source, "createRelaySubscription"), "{source}");
    }
}

#[test]
fn relay_subscription_rejects_unproven_capture_origins_and_shapes() {
    for (from, to) in [
        ("let registry=[];", "using registry=[];"),
        ("let registry=[];", "let {registry}=other;"),
        (
            "function $THREAD_7_101_2(){",
            "function $THREAD_7_101_2(extra){",
        ),
        (
            "function $THREAD_7_101_2(){",
            "async function $THREAD_7_101_2(){",
        ),
        (
            "function $THREAD_7_101_2(){",
            "function* $THREAD_7_101_2(){",
        ),
    ] {
        let source = RELAY.replace(from, to);
        assert!(!named(&source, "createRelaySubscription"), "{source}");
    }
    // Constructor provenance is intentionally opaque: an unknown constructor
    // expression is valid evidence for this generic consumer protocol. Queue
    // identity remains structurally checked, while unsupported queue origins
    // above are still refused.
    assert!(named(
        &RELAY.replace(
            "let Ctor=function Observable(executor){executor({next(){}});};",
            "let Ctor=unknownFactory();",
        ),
        "createRelaySubscription"
    ));
    // A direct subscriber forward without the owned relay/collection step.
    let direct = RELAY.replace(
        "let channel=$THREAD_subscriber;\nregistry.push($THREAD_value=>channel.next($THREAD_value));",
        "$THREAD_subscriber.next($THREAD_value);",
    );
    assert!(!named(&direct, "createRelaySubscription"), "{direct}");
    // A relay using the opaque callback as a receiver but not collecting it.
    let uncollected = RELAY.replace(
        "registry.push($THREAD_value=>channel.next($THREAD_value));",
        "$THREAD_subscriber.next($THREAD_value);",
    );
    assert!(
        !named(&uncollected, "createRelaySubscription"),
        "{uncollected}"
    );
}

#[test]
fn human_names_and_collisions_remain_guarded() {
    let human = RELAY.replace("$THREAD_7_101_2", "createRelaySubscription");
    assert!(unchanged(&human));
    let collision = RELAY.replace(
        "let registry=[];",
        "let registry=[];let createRelaySubscription=1;",
    );
    assert!(!named(&collision, "createRelaySubscription"), "{collision}");
    // Another unrelated generated function must not inherit the role.
    let unrelated = RELAY.replace(
        "return new Ctor($THREAD_subscriber=>{\nlet channel=$THREAD_subscriber;\nregistry.push($THREAD_value=>channel.next($THREAD_value));\n});",
        "return new Ctor($THREAD_subscriber=>{return $THREAD_subscriber.next(1);});",
    );
    assert!(!named(&unrelated, "createRelaySubscription"), "{unrelated}");
}
