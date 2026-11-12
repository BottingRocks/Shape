use shape::naming::name_threads;
const SNAPSHOT: &str=r#"function main(){let defaults=[],notifications=[],data=[],names=[],metadata=[],errors={toArray(){return [];}};
function $THREAD_1_2(input){let options=input===void 0?{}:input;let selected=options.signalFilter===void 0?defaults:options.signalFilter;
if(options.notifyLastChance===void 0||options.notifyLastChance){selected.forEach(id=>{if(notifications[id]!=null){notifications[id].call(void 0);}});}
let values;if(options.includeDebugNames!==void 0&&options.includeDebugNames){values=selected.map(id=>{return {value:data[id],signal:names[id]};});}else{values=selected.map(id=>data[id]);}
let positional=[];let keyed=[];let named=[];
for(let i=0;i<values.length;i++){let entry=metadata[selected[i]];let receiver,method,value;
if(Object.prototype.hasOwnProperty.call(entry,"name")){method=(typeof entry.name==="number"?keyed:named).push;value=[entry.name,values[i]];receiver=typeof entry.name==="number"?keyed:named;}
else{receiver=positional;method=positional.push;value=values[i];}method.call(receiver,value);}
return {signals:named,positionalSignals:positional,keyedSignals:keyed,errors:errors.toArray()};}return $THREAD_1_2;}main();"#;
fn snapshot_claim(s:&str)->bool {name_threads(s,true).unwrap().report.outcomes.iter().any(|o|o.applied&&o.binding.name=="$THREAD_1_2"&&o.evaluator.as_deref()==Some("observables_partitioned_signal_snapshot"))}
#[test]
fn snapshot_full_own_body_and_alpha_variants() {
    assert!(snapshot_claim(SNAPSHOT));
    assert!(snapshot_claim(&SNAPSHOT.replace("entry","fieldInfo").replace("selected","ids").replace("keyed","numericFields").replace("numericFieldsSignals","keyedSignals")));
    let split=SNAPSHOT.replace("if(options.notifyLastChance===void 0||options.notifyLastChance)","let notify;if(options.notifyLastChance===void 0){notify=true;}else{notify=options.notifyLastChance;}if(notify)").replace("return {value:data[id],signal:names[id]};", "let record={value:void 0,signal:void 0};record.value=data[id];record.signal=names[id];return record;").replace("for(let i=0;i<values.length;i++)", "let i=0;for(;i<values.length;i++)");
    assert!(snapshot_claim(&split));
    let r=name_threads(SNAPSHOT,true).unwrap();let mut replay=SNAPSHOT.to_string();for e in r.report.edits.iter().rev(){replay.replace_range(e.start as usize..e.end as usize,&e.replacement);}assert_eq!(replay,r.source);
}
#[test]
fn snapshot_rejects_wrong_bindings_shadowing_writers_receivers_order_and_values() {
    for (a,b) in [
        ("notifyLastChance===void 0||", "notifyLastChance!==void 0||"),
        ("includeDebugNames!==void 0&&", "includeDebugNames===void 0&&"),
        ("notifications[id].call(void 0)","notifications[id].call(options)"),
        ("notifications[id].call(void 0)","notifications[id].call(void 0,id)"),
        ("signal:names[id]", "signal:data[id]"),
        ("value:data[id],signal:names[id]", "signal:names[id],value:data[id]"),
        ("metadata[selected[i]]", "metadata[i]"),
        ("value=[entry.name,values[i]]", "value=[values[i],entry.name]"),
        ("receiver=typeof entry.name===\"number\"?keyed:named", "receiver=typeof entry.name===\"number\"?named:keyed"),
        ("method.call(receiver,value)","method.call(positional,value)"),
        ("for(let i=0;i<values.length;i++)", "for(let i=1;i<values.length;i++)"),
        ("let values;", "let values;sideEffect();"),
        ("id=>{return {value", "id=>{let data=foreign;return {value"),
        ("function main(){", "function main(Object){"),
        ("errors:errors.toArray()", "errors:errors.toArray(values)"),
        ("signals:named,positionalSignals:positional", "signals:positional,positionalSignals:named"),
    ] {assert!(SNAPSHOT.contains(a),"missing {a}");assert!(!snapshot_claim(&SNAPSHOT.replace(a,b)),"accepted {a}->{b}");}
    assert!(!snapshot_claim(&SNAPSHOT.replace("function main(){let defaults=[]", "function main(defaults){let replaced=[]")));
}

const ENVELOPE:&str=r#"function main(){let defaults=[],encoder=provider,transform=other,prefix="p",tag=hasher;
function $THREAD_1_2(input){let options;if(input!=null){options=input;}else{options=Object.create(null);}let filter=options.signalFilter===void 0?defaults:options.signalFilter;let encode=typeof(options.encode===void 0?null:options.encode)==="function"?options.encode===void 0?null:options.encode:encoder;let result=Object.create(null);encode=encode(transform({signalFilter:filter,includeDebugNames:false,notifyLastChance:(options.notifyLastChance===void 0||options.notifyLastChance)??true}));result.f=prefix;result.b=tag(prefix+encode).toString(36);result.c="opaque-build";result.d=options.descriptor===void 0?"opaque-descriptor":options.descriptor;result.z="q";result.a=encode;return result;}return $THREAD_1_2;}main();"#;
fn envelope_claim(s:&str)->bool{name_threads(s,true).unwrap().report.outcomes.iter().any(|o|o.applied&&o.binding.name=="$THREAD_1_2"&&o.evaluator.as_deref()==Some("signals_options_transform_envelope"))}
#[test]fn options_envelope_full_body_and_alpha(){assert!(envelope_claim(ENVELOPE));assert!(envelope_claim(&ENVELOPE.replace("result","envelope").replace("filter","selected").replace("opaque-build","unrelated literal")));let split=ENVELOPE.replace("encode=encode(transform({signalFilter:filter,includeDebugNames:false,notifyLastChance:(options.notifyLastChance===void 0||options.notifyLastChance)??true}));","filter={signalFilter:filter,includeDebugNames:false,notifyLastChance:void 0};filter.notifyLastChance=(options.notifyLastChance===void 0||options.notifyLastChance)??true;encode=encode(transform(filter));");assert!(envelope_claim(&split));}
#[test]fn options_envelope_rejects_slot_receiver_order_effect_and_capture_mismatches(){for(a,b)in[
("input!=null","input!==void 0"),("Object.create(null)","Object.create({})"),("signalFilter:filter","signalFilter:options"),("includeDebugNames:false","includeDebugNames:true"),("??true","??false"),("encode(transform(","transform(encode("),("prefix+encode","encode+prefix"),(".toString(36)",".toString(16)"),("result.a=encode","result.a=filter"),("result.z=\"q\"","result.z=\"x\""),("return result","sideEffect();return result"),("function main(){","function main(Object){"),("result.f=prefix;result.b=","result.b="),("encoder;let result","transform;let result"),("result.c=\"opaque-build\"","result.c=input"),("options.descriptor===void 0","options.descriptor==null")]{assert!(ENVELOPE.contains(a),"{a}");assert!(!envelope_claim(&ENVELOPE.replace(a,b)),"{a}->{b}");}}
const REPORTS:&str=r#"function main(){let records=[],transform=provider;function $THREAD_1_2(input){let options=input===void 0?{}:input;let debug=options.includeDebugNames!==void 0&&options.includeDebugNames;let notify=options.notifyLastChance===void 0||options.notifyLastChance;return records.map(record=>{return {beaconEndpoint:record.a,beaconData:transform({signalFilter:record.b,includeDebugNames:debug,notifyLastChance:notify})};});}return $THREAD_1_2;}main();"#;
fn reports_claim(s:&str)->bool{name_threads(s,true).unwrap().report.outcomes.iter().any(|o|o.applied&&o.binding.name=="$THREAD_1_2"&&o.evaluator.as_deref()==Some("signals_beacon_endpoint_data_map"))}
#[test]fn endpoint_map_full_body_alpha_and_negatives(){assert!(reports_claim(REPORTS));assert!(reports_claim(&REPORTS.replace("record","entry").replace("debug","includeNames")));for(a,b)in[("beaconEndpoint:record.a","beaconEndpoint:record.b"),("signalFilter:record.b","signalFilter:record.a"),("notifyLastChance:notify","notifyLastChance:debug"),("includeDebugNames:debug","includeDebugNames:notify"),("transform({","transform.call(records,{"),("record=>{return","record=>{sideEffect();return"),("input===void 0","input==null"),("!==void 0&&","===void 0&&")]{assert!(REPORTS.contains(a));assert!(!reports_claim(&REPORTS.replace(a,b)),"{a}->{b}");}}

const BASE64_OPTIONS:&str=include_str!("fixtures/signal_serialization_family/base64_options_envelope.js");
#[test]fn base64_options_envelope_complete_byte_proof(){assert!(envelope_claim(BASE64_OPTIONS));assert!(envelope_claim(&BASE64_OPTIONS.replace("$AY","cursor").replace("$AZ","encodedText")));for(a,b)in[("$AY + 2","$AY + 1"),("$AY += 3","$AY += 2"),("& 63","& 62"),("% 3 === 2","% 3 === 1"),("$BA >> 6","$BA >> 5"),("$AW.charAt(","$AW.other("),("$KEY_11 = $AU($KEY_216($KEY_11))","$KEY_11 = $KEY_216($AU($KEY_11))")]{assert!(BASE64_OPTIONS.contains(a),"{a}");assert!(!envelope_claim(&BASE64_OPTIONS.replace(a,b)),"{a}->{b}");}}
