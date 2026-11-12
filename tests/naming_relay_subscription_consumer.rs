use shape::naming::name_threads;

fn named(source: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some("createRelaySubscription")
                && outcome.evaluator.as_deref() == Some("shared_frontier_relay_subscription")
        })
}

const FACTORY: &str = r#"function main(){let listeners=[];
function $THREAD_9_38575(){return new Observable(subscriber=>{let receiver=subscriber;listeners.push(value=>receiver.next(value));});}
return $THREAD_9_38575;}main();"#;

#[test]
fn exact_owned_next_forwarder_accepts_generated_and_alpha_bindings() {
    assert!(named(FACTORY));
    assert!(named(
        &FACTORY
            .replace("$THREAD_9_38575", "$THREAD_901_7788_2")
            .replace("let receiver", "let sink")
            .replace("receiver.next", "sink.next")
    ));
}

#[test]
fn operation_argument_order_and_owned_callback_identity_are_required() {
    for source in [
        FACTORY.replace("listeners.push", "listeners.concat"),
        FACTORY.replace("receiver.next(value)", "receiver.error(value)"),
        FACTORY.replace("receiver.next(value)", "receiver.next(receiver)"),
        FACTORY.replace("receiver.next(value)", "receiver.next(value, 0)"),
        FACTORY.replace("let receiver=subscriber;", "let receiver=other;"),
        FACTORY.replace(
            "let receiver=subscriber;listeners.push",
            "let receiver=subscriber;receiver=other;listeners.push",
        ),
    ] {
        assert!(!named(&source), "unexpected claim for {source}");
    }
}

#[test]
fn constructor_arguments_and_callback_shapes_are_not_guessed() {
    for source in [
        FACTORY.replace(
            "new Observable(subscriber=>",
            "new Observable(other, subscriber=>",
        ),
        FACTORY.replace(
            "listeners.push(value=>receiver.next(value))",
            "listeners.push((value, other)=>receiver.next(value))",
        ),
        FACTORY.replace(
            "listeners.push(value=>receiver.next(value))",
            "listeners.push(value=>{receiver.next(value);})",
        ),
        FACTORY.replace(
            "return new Observable(subscriber=>{",
            "return new Observable((subscriber=other)=>{",
        ),
    ] {
        assert!(!named(&source), "unexpected claim for {source}");
    }
}

#[test]
fn existing_names_and_scope_collisions_remain_protected() {
    let human = FACTORY.replace("$THREAD_9_38575", "keepReadableName");
    assert!(!named(&human));
    let collision = FACTORY.replace(
        "function $THREAD_9_38575",
        "let createRelaySubscription=0;function $THREAD_9_38575",
    );
    assert!(!named(&collision));
}
