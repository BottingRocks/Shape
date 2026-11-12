use shape::naming::name_threads;
fn check(source: &str, expected: Option<&str>) {
    let r = name_threads(source, true).unwrap();
    let o = r
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_91_1")
        .unwrap();
    if let Some(name) = expected {
        assert!(o.applied, "{o:?}");
        assert_eq!(o.role_name.as_deref(), Some(name));
    } else {
        assert!(
            !matches!(
                o.evaluator.as_deref(),
                Some(
                    "shared_captured_observer_forwarding"
                        | "counted_function_to_string_getter"
                        | "events_input_paste_marker_hoisted_capture"
                )
            ),
            "{o:?}"
        );
    }
}
#[test]
fn whole_scope_forwarders() {
    for (body, params, role) in [
        (
            "return receiver.error(value);",
            "value",
            "forwardErrorToCapturedObserver",
        ),
        (
            "return receiver['next'](value);",
            "value",
            "forwardValueToCapturedObserver",
        ),
        (
            "return receiver.complete();",
            "",
            "completeCapturedObserver",
        ),
        (
            "return receiver.forEach(item => item.unsubscribe());",
            "",
            "unsubscribeEachCapturedHandle",
        ),
    ] {
        let s=format!("function setup(input){{let receiver=input;function $THREAD_91_1({params}){{{body}}}return $THREAD_91_1;}}setup(opaque);");
        check(&s, Some(role));
        check(
            &s.replace("receiver", "renamedCapture")
                .replace("value", "renamedValue"),
            Some(role),
        );
    }
    check("function setup(source){let handle;handle=source.subscribe(value=>value);function $THREAD_91_1(){return handle.unsubscribe();}return $THREAD_91_1;}setup(opaque);",None);
}
#[test]
fn refuses_changed_protocols_and_capture_epochs() {
    for body in [
        "return receiver.error(other);",
        "return receiver?.error(value);",
        "return receiver.error?.(value);",
        "return receiver.error(...value);",
        "receiver.error(value);",
        "return receiver.error(value,value);",
        "extra();return receiver.error(value);",
        "function donor(){return receiver.error(value);}return other(value);",
        "return receiver.forEach(item=>other.unsubscribe());",
    ] {
        check(&format!("function setup(input){{let receiver=input;function $THREAD_91_1(value){{{body}}}return $THREAD_91_1;}}setup(opaque);"),None);
    }
    for s in [
        "function setup(input){$THREAD_91_1(0);let receiver=input;function $THREAD_91_1(value){return receiver.error(value);}return $THREAD_91_1;}setup(opaque);",
        "function setup(input){let receiver;if(input)receiver=input;function $THREAD_91_1(value){return receiver.error(value);}return $THREAD_91_1;}setup(opaque);",
        "function setup(input){let receiver=input;function $THREAD_91_1(value){return receiver.error(value);}receiver=other;return $THREAD_91_1;}setup(opaque);",
        "function setup(input){let receiver=input.subscribe($THREAD_91_1);function $THREAD_91_1(value){return receiver.error(value);}return $THREAD_91_1;}setup(opaque);",
        "function setup(input){let receiver=(()=>{receiver=input;return input;})();function $THREAD_91_1(value){return receiver.error(value);}return $THREAD_91_1;}setup(opaque);",
        "function setup(input){const early=$THREAD_91_1;early(0);let receiver=input;function $THREAD_91_1(value){return receiver.error(value);}return $THREAD_91_1;}setup(opaque);",
    ] {check(s,None);}
}
#[test]
fn counted_to_string_getter_scope() {
    let source="function setup(){let count=0;function $THREAD_91_1(){++count;return Function.prototype.toString;}function stub(){}let descriptor={get:void 0};descriptor.get=$THREAD_91_1;Object.defineProperty(stub,'toString',descriptor);return [''+stub,count];}setup();";
    check(source, Some("toStringGetterStub"));
    let owned_source = source.replace(
        "Object.defineProperty(stub,'toString',descriptor);",
        "void descriptor;",
    );
    let source = owned_source.as_str();
    check(source, Some("countAccessAndGetFunctionToString"));
    check(
        &source.replace("count", "alphaCounter"),
        Some("countAccessAndGetFunctionToString"),
    );
    for source in [
        source.replace("++count", "count++"),
        source.replace("++count", "--count"),
        source.replace("count=0", "count=1"),
        source.replace(
            "return Function.prototype.toString",
            "extra();return Function.prototype.toString",
        ),
        source.replace("function setup()", "function setup(Function)"),
        source.replace("let count=0;", "$THREAD_91_1();let count=0;"),
        source.replace("let count=0;", "let count;if(ready)count=0;"),
        source.replace("return [''+stub,count]", "count=2;return [''+stub,count]"),
        source.replace("let count=0;", "let count=run($THREAD_91_1);"),
    ] {
        check(&source, None);
    }
}
#[test]
fn producer_alias_must_be_ready_at_copy() {
    for source in [
        "function setup(input){let receiver=late;let late=input;function $THREAD_91_1(value){return receiver.error(value);}return $THREAD_91_1;}setup(opaque);",
        "function setup(input){let receiver=original;let original;if(input)original=input;function $THREAD_91_1(value){return receiver.error(value);}return $THREAD_91_1;}setup(opaque);",
        "function setup(input){let original=input;let receiver=original;original=other;function $THREAD_91_1(value){return receiver.error(value);}return $THREAD_91_1;}setup(opaque);",
    ] {check(source,None);}
}
#[test]
fn paste_whole_factory_scope_and_epoch_negatives() {
    let source="function factory(ports,context,register){let alias=context;let receiver,count,inactive;receiver=ports[1];let doc=alias.global.document;inactive=false;count=0;register(()=>{function $THREAD_91_1(event){try{if(event.target.tagName==='INPUT'){if(event.inputType==='insertFromPaste'){++count;}else if(event.target!==doc.activeElement){inactive=true;}}}catch(error){receiver.error(error);}}let handler=$THREAD_91_1;doc.addEventListener('input',handler,true);});}factory(ports,context,register);";
    check(source, Some("recordDocumentInputEvent"));
    let owned_source = source.replace(
        "doc.addEventListener('input',handler,true);",
        "registerHandler(handler);",
    );
    let source = owned_source.as_str();
    check(source, Some("countInputPasteAndMarkInactiveTarget"));
    check(
        &source
            .replace("count", "pasteCounter")
            .replace("inactive", "inactiveFlag")
            .replace("receiver", "errorPort"),
        Some("countInputPasteAndMarkInactiveTarget"),
    );
    for altered in [
        source.replace("++count;", "++count;extra();"),
        source.replace("++count;", "count++;"),
        source.replace("receiver.error(error)", "receiver.error(other)"),
        source.replace(
            "event.target!==doc.activeElement",
            "other.target!==doc.activeElement",
        ),
        source.replace("count=0;", "if(ready)count=0;"),
        source
            .replace("count=0;", "")
            .replace("let handler=", "$THREAD_91_1(event);count=0;let handler="),
        source.replace(
            "let handler=$THREAD_91_1;",
            "count=5;let handler=$THREAD_91_1;",
        ),
        source.replace("receiver=ports[1];", "").replace(
            "let handler=",
            "receiver=register($THREAD_91_1);let handler=",
        ),
        source.replace(
            "let handler=$THREAD_91_1;",
            "receiver=other;let handler=$THREAD_91_1;",
        ),
        source.replace("inactive=true;", "inactive=true;count=7;"),
    ] {
        check(&altered, None);
    }
}
