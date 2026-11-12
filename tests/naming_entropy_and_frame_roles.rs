use shape::naming::name_threads;

fn expected_evaluators(name: &str) -> &'static [&'static str] {
    match name {
        "hashStringOrBuffer" => &["signals_hash_string_or_buffer"],
        "recordAnimationFrameSample" => &["graphics_frame_sample_request"],
        "accumulatePointerTravelDistance" => &["events_pointer_travel_try_catch"],
        "encodeNumericArrayWithEntropyTag" => &["encoding_entropy_tagged_numeric_array"],
        _ => &[],
    }
}

fn named(source: &str, expected: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.function_roles.iter().any(|role| {
            role.candidates
                .iter()
                .any(|candidate| candidate == expected)
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| expected_evaluators(expected).contains(&evaluator.as_str()))
        })
    })
}

fn replay(source: &str) -> String {
    let report = name_threads(source, true).unwrap();
    let mut replay = source.to_owned();
    let mut edits = report.report.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    replay
}

const ANIMATION: &str = r#"function main(){let samples=[];
let sampling=false;
let divisor=16;
let api={requestAnimationFrame:function(callback){return callback;}};
let next=function(){};
function $THREAD_9_100(timestamp){
samples.push(Math.round(timestamp/divisor));
sampling=true;
if(samples.length<20){api.requestAnimationFrame(next);}
}
return $THREAD_9_100;}main();"#;

const MOUSE: &str = r#"function main(){let mathApi={sqrt:Math.sqrt};
let lastX=0;
let lastY=0;
let total=0;
let reporter={error:function(error){return error;}};
function $THREAD_6_200(event){
try{
let delta=event.clientY-lastY;
delta=mathApi.sqrt(Math.pow(event.clientX-lastX,2)+Math.pow(delta,2));
lastX=event.clientX;
lastY=event.clientY;
total=total+(isNaN(delta)?0:delta);
}catch(e){reporter.error(e);}
}
return $THREAD_6_200;}main();"#;

const ENCODE: &str = r#"function main(){let d0=0,d1=0,d2=0,d3=0,d4=0,d5=0,d6=0,d7=0;
let makeStream=function(seed){return function(){return seed&255;};};
let encode=function(stream,value,buffer,offset){buffer[offset]=value&255;return offset+1;};
let observer={next:function(value){return value;},complete:function(){return 1;}};
function $THREAD_9_74032(input){
let seed=Math.random()*4294967296|0;
let tag=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
let state=makeStream(seed^1798154811);
let count=input.length;
let offset=encode(state,count,tag,3);
for(;;){--count;if(!(count>=0))break;offset=encode(state,input[count],tag,offset);count=Number(count);}
observer.next([[],tag]);
observer.complete();
}
return $THREAD_9_74032;}main();"#;

#[test]
fn animation_frame_sampling_proves_bounded_rearm() {
    for source in [
        ANIMATION.to_owned(),
        ANIMATION
            .replace("$THREAD_9_100", "$THREAD_71_887766_2")
            .replace("samples", "$KEY_910")
            .replace("sampling", "$KEY_911")
            .replace("divisor", "$KEY_912")
            .replace("api", "$KEY_913")
            .replace("next", "$KEY_914")
            .replace("timestamp", "$KEY_915"),
        ANIMATION.replace("function(){return callback;}", "()=>callback"),
        ANIMATION.replace("function(){return callback;}", "(callback)=>callback"),
        ANIMATION
            .replace("let api={requestAnimationFrame:function(callback){return callback;}};", "let apiImpl={requestAnimationFrame:function(callback){return callback;}};let api=apiImpl;")
            .replace("let next=function(){};", "let nextImpl=function(){};let next=nextImpl;"),
    ] {
        assert!(named(&source, "recordAnimationFrameSample"), "{source}");
        assert_eq!(replay(&source), name_threads(&source, true).unwrap().source);
    }
}

#[test]
fn animation_frame_sampling_refuses_changed_apis_bounds_and_origins() {
    for (from, to) in [
        ("Math.round", "Math.floor"),
        ("timestamp/divisor", "divisor/timestamp"),
        ("samples.push", "other.push"),
        ("samples.length<20", "samples.length<=20"),
        ("samples.length<20", "samples.length<0"),
        ("api.requestAnimationFrame(next)", "api.requestIdleCallback(next)"),
        ("api.requestAnimationFrame(next)", "api.requestAnimationFrame()"),
        ("sampling=true", "sampling=1"),
        ("sampling=true", "sampling=true;sampling=false"),
        ("let next=function(){};", "let next=function(){};next=function(){};"),
        ("let next=function(){};", "let next=laterNext;let laterNext=function(){};"),
        ("let api={requestAnimationFrame:function(callback){return callback;}};", "let api=laterApi;let laterApi={requestAnimationFrame:function(callback){return callback;}};"),
        ("let samples=[];", "let {samples}=provider;"),
        ("let samples=[];", "let samples=[];var samples;"),
        (
            "let api={requestAnimationFrame:function(callback){return callback;}};",
            "if(true){var api={requestAnimationFrame:function(callback){return callback;}};}",
        ),
        ("let next=function(){};", "if(true){var next=function(){};}"),
        ("function $THREAD_9_100(timestamp){", "function $THREAD_9_100(timestamp=0){"),
        ("function $THREAD_9_100(timestamp){", "function $THREAD_9_100(...timestamp){"),
    ] {
        let source = ANIMATION.replace(from, to);
        assert!(
            !named(&source, "recordAnimationFrameSample"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn pointer_travel_accumulation_proves_exact_distance_formula() {
    for source in [
        MOUSE.to_owned(),
        MOUSE
            .replace("$THREAD_6_200", "$THREAD_300_761_4")
            .replace("mathApi", "$KEY_920")
            .replace("lastX", "$KEY_921")
            .replace("lastY", "$KEY_922")
            .replace("total", "$KEY_923")
            .replace("reporter", "$KEY_924")
            .replace("event", "$KEY_925")
            .replace("delta", "$KEY_926"),
        MOUSE.replace(
            "total=total+(isNaN(delta)?0:delta);",
            "total = total + (isNaN(delta) ? 0 : delta);",
        ),
        MOUSE.replace("try{\n", "try {\n"),
    ] {
        assert!(
            named(&source, "accumulatePointerTravelDistance"),
            "{source}"
        );
        assert_eq!(replay(&source), name_threads(&source, true).unwrap().source);
    }
}

#[test]
fn pointer_travel_refuses_changed_coordinates_and_control_flow() {
    for (from, to) in [
        ("event.clientY-lastY", "event.clientX-lastY"),
        ("event.clientX-lastX", "event.clientY-lastX"),
        (
            "Math.pow(event.clientX-lastX,2)",
            "Math.hypot(event.clientX-lastX,2)",
        ),
        ("Math.pow(delta,2)", "Math.pow(delta,3)"),
        ("isNaN(delta)", "isFinite(delta)"),
        ("?0:delta", "?1:delta"),
        ("lastX=event.clientX", "lastX=event.pageX"),
        ("lastY=event.clientY", "lastX=event.clientY"),
        ("total=total+", "total=lastX+"),
        ("catch(e){reporter.error(e);}", "catch(e){}"),
        ("reporter.error(e)", "reporter.warn(e)"),
        (
            "function $THREAD_6_200(event){",
            "function $THREAD_6_200({event}){",
        ),
        (
            "function $THREAD_6_200(event){",
            "function $THREAD_6_200(event=null){",
        ),
        (
            "let delta=event.clientY-lastY;",
            "let delta;delta=event.clientY-lastY;delta=0;",
        ),
        ("let total=0;", "var total=0;var total;"),
        ("try{", "if(event)try{"),
        (
            "}catch(e){reporter.error(e);}",
            "}catch(e){reporter.error(e);}finally{}",
        ),
    ] {
        let source = MOUSE.replace(from, to);
        assert!(
            !named(&source, "accumulatePointerTravelDistance"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn entropy_tagged_sequence_encoder_proves_ordered_captured_calls() {
    for source in [
        ENCODE.to_owned(),
        ENCODE
            .replace("$THREAD_9_74032", "$THREAD_923_182734_2")
            .replace("makeStream", "$KEY_930")
            .replace("encode", "$KEY_931")
            .replace("observer", "$KEY_932")
            .replace("input", "$KEY_933")
            .replace("seed", "$KEY_934")
            .replace("tag", "$KEY_935")
            .replace("state", "$KEY_936")
            .replace("count", "$KEY_937")
            .replace("offset", "$KEY_938"),
        ENCODE.replace(
            "for(;;){--count;if(!(count>=0))break;offset=encode(state,input[count],tag,offset);count=Number(count);}",
            "for (;;) { --count; if (!(count >= 0)) break; offset = encode(state, input[count], tag, offset); count = Number(count); }",
        ),
        ENCODE.replace("let observer={next:function(value){return value;},complete:function(){return 1;}};",
            "const observer = { next: (value) => value, complete: () => 1 };"),
        ENCODE
            .replace("let makeStream=function(seed){return function(){return seed&255;};};", "let makeStreamImpl=function(seed){return function(){return seed&255;};};let makeStream=makeStreamImpl;")
            .replace("let encode=function(stream,value,buffer,offset){buffer[offset]=value&255;return offset+1;};", "let encodeImpl=function(stream,value,buffer,offset){buffer[offset]=value&255;return offset+1;};let encode=encodeImpl;"),
    ] {
        assert!(named(&source, "encodeNumericArrayWithEntropyTag"), "{source}");
        assert_eq!(replay(&source), name_threads(&source, true).unwrap().source);
    }
}

#[test]
fn entropy_tagged_sequence_refuses_changed_operations_and_origins() {
    for (from, to) in [
        ("Math.random()*4294967296|0", "Math.random()*4294967296"),
        ("seed>>>24", "seed>>>25"),
        ("seed>>16&255", "seed>>15&255"),
        ("seed&255", "seed&127"),
        ("makeStream(seed^1798154811)", "makeStream(seed|1798154811)"),
        ("let makeStream=function(seed){return function(){return seed&255;};};", "let makeStream=laterMakeStream;let laterMakeStream=function(seed){return function(){return seed&255;};};"),
        ("let encode=function(stream,value,buffer,offset){buffer[offset]=value&255;return offset+1;};", "let encode=laterEncode;let laterEncode=function(stream,value,buffer,offset){buffer[offset]=value&255;return offset+1;};"),
        ("encode(state,count,tag,3)", "encode(state,count,tag,4)"),
        ("input.length", "input.size"),
        ("for(;;){--count;", "for(;;){count--;"),
        ("if(!(count>=0))break;", "if(!(count>0))break;"),
        ("input[count]", "input[offset]"),
        ("count=Number(count);", "count=Number(offset);"),
        ("observer.next([[],tag])", "observer.next([tag])"),
        ("observer.complete()", "observer.next(tag)"),
        ("function $THREAD_9_74032(input){", "function $THREAD_9_74032({input}){"),
        ("function $THREAD_9_74032(input){", "function $THREAD_9_74032(input,extra){"),
        ("let seed=Math.random()", "var seed=Math.random();var seed"),
        ("let encode=function", "let encode=arguments[0]||function"),
    ] {
        let source = ENCODE.replace(from, to);
        assert!(
            !named(&source, "encodeNumericArrayWithEntropyTag"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn meaningful_existing_names_are_preserved() {
    let source = ANIMATION.replace("$THREAD_9_100", "recordAnimationFrameSample");
    let report = name_threads(&source, true).unwrap();
    assert_eq!(report.source, source);
    assert!(report
        .report
        .outcomes
        .iter()
        .all(|o| !(o.applied && o.proposed_name.as_deref() == Some("recordAnimationFrameSample"))));
}

#[test]
fn unrelated_small_wrappers_do_not_gain_these_roles() {
    for source in [
        r#"function main(){function $THREAD_1_1(a){return a+1;}return $THREAD_1_1;}main();"#,
        r#"function main(){function $THREAD_1_2(a){let b=a;return b;}return $THREAD_1_2;}main();"#,
        r#"function main(){let api={run:function(){}};function $THREAD_1_3(a){try{api.run(a);}catch(e){}}return $THREAD_1_3;}main();"#,
    ] {
        assert!(!named(source, "recordAnimationFrameSample"));
        assert!(!named(source, "accumulatePointerTravelDistance"));
        assert!(!named(source, "encodeNumericArrayWithEntropyTag"));
    }
}

const HASH: &str = r#"function main(){let d0=0,d1=0,d2=0,d3=0,d4=0,d5=0,d6=0,d7=0,d8=0,d9=0,d10=0,d11=0,d12=0,d13=0;
let charCode=function(sourceString){return sourceString.charCodeAt();};
let supported=false;
function $THREAD_6_42036(input){
if(typeof input==="string"){
let even=input.length>>1<<1;
let i=0;
let hash=0;
for(;i<even;i=i+2){hash=(hash<<5)-hash+(charCode(input[i])<<16|charCode(input[i+1]))|0;}
if(even!==input.length){return (hash<<5)-hash+(charCode(input[even])<<16)|0;}else{return hash;}
}
let flag=supported;
if(flag){flag=ArrayBuffer.isView(input);}
if(flag){
let buffer;
if(input.byteLength%4===0){buffer=input.buffer;}
else{buffer=new ArrayBuffer(input.byteLength+(4-input.byteLength%4));new Int8Array(buffer).set(new Int8Array(input.buffer));}
buffer=new Int32Array(buffer);
let hash=0;
let j=0;
for(;j<buffer.length;j++){hash=(hash<<5)-hash+buffer[j]|0;}
return hash;
}else{throw new TypeError();}
}
return $THREAD_6_42036;}main();"#;

#[test]
fn string_and_view_hash_proves_both_exact_loops() {
    for source in [
        HASH.to_owned(),
        HASH
            .replace("$THREAD_6_42036", "$THREAD_71_887766_2")
            .replace("charCode=", "$KEY_940=")
            .replace("charCode(input", "$KEY_940(input")
            .replace("supported", "$KEY_941")
            .replace("input", "$KEY_942")
            .replace("even", "$KEY_943")
            .replace("hash", "$KEY_944")
            .replace("flag", "$KEY_946"),
        HASH.replace("function(sourceString){return sourceString.charCodeAt();}", "(sourceString)=>sourceString.charCodeAt()"),
        HASH.replace("charCode=function(sourceString){return sourceString.charCodeAt();};", "charCode=(function(sourceString){return sourceString.charCodeAt();});"),
        HASH.replace("let charCode=function(sourceString){return sourceString.charCodeAt();};", "let charCodeImpl=function(sourceString){return sourceString.charCodeAt();};let charCode=charCodeImpl;"),
        HASH.replace("for(;i<even;i=i+2){hash=(hash<<5)-hash+(charCode(input[i])<<16|charCode(input[i+1]))|0;}",
            "for (; i < even; i = i + 2) { hash = (hash << 5) - hash + (charCode(input[i]) << 16 | charCode(input[i + 1])) | 0; }"),
    ] {
        assert!(named(&source, "hashStringOrBuffer"), "{source}");
        assert_eq!(replay(&source), name_threads(&source, true).unwrap().source);
    }
}

#[test]
fn string_and_view_hash_refuses_changed_accessor_loops_and_origins() {
    for (from, to) in [
        ("typeof input===\"string\"", "typeof input===\"number\""),
        ("input.length>>1<<1", "input.length>>1<<2"),
        ("i=i+2", "i=i+1"),
        ("hash<<5", "hash<<6"),
        ("charCode(input[i])<<16", "charCode(input[i])<<8"),
        ("charCode(input[i+1])", "charCode(input[i+2])"),
        ("charCode(input[even])", "charCode(input[even+1])"),
        ("even!==input.length", "even===input.length"),
        ("let hash=0;", "let hash=1;"),
        ("return hash;", "return even;"),
        ("return sourceString.charCodeAt();", "return sourceString.codePointAt();"),
        ("return sourceString.charCodeAt();", "return other.charCodeAt();"),
        ("charCode=function(sourceString){", "charCode=function(sourceString=0){"),
        ("let charCode=function(sourceString){return sourceString.charCodeAt();};", "let charCode=laterCharCode;let laterCharCode=function(sourceString){return sourceString.charCodeAt();};"),
        ("charCode=function(sourceString){", "charCode=(...sourceString)=>"),
        ("let flag=supported;", "let flag=input;"),
        ("ArrayBuffer.isView(input)", "ArrayBuffer.isView(buffer)"),
        ("input.byteLength%4===0", "input.byteLength%4!==0"),
        ("new Int32Array(buffer)", "new Int16Array(buffer)"),
        ("new Int8Array(buffer).set(new Int8Array(input.buffer))", "new Int8Array(buffer).set(new Int8Array(input))"),
        ("buffer=input.buffer", "buffer=input.data"),
        ("j<buffer.length", "j<=buffer.length"),
        ("j++", "j--"),
        ("buffer[j]", "buffer[i]"),
        ("throw new TypeError()", "throw new Error()"),
        ("function $THREAD_6_42036(input){", "function $THREAD_6_42036({input}){"),
        ("function $THREAD_6_42036(input){", "function $THREAD_6_42036(input,extra){"),
    ] {
        let source = HASH.replace(from, to);
        assert!(
            !named(&source, "hashStringOrBuffer"),
            "accepted {from} -> {to}"
        );
    }
}
