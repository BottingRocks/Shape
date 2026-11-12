//! Own-body options, notification, mapping and partition protocol. Captured
//! collections contain runtime values; no type, native identity or successful
//! initialization is inferred from their labels or declaration text positions.
use super::*;
#[path = "snapshot_syntax.rs"]
pub(in crate::naming::functions::observables) mod syntax;
use syntax::{compile_complete, complete_capture_specs, CompleteSpec};
use std::sync::LazyLock;
const BASE: &str = r#"function snapshot(input){
let options=input===void 0?{}:input;
let selected=options.signalFilter===void 0?CAP_DEFAULT:options.signalFilter;
if(options.notifyLastChance===void 0||options.notifyLastChance){selected.forEach(id=>{if(CAP_NOTIFY[id]!=null){CAP_NOTIFY[id].call(void 0);}});}
let values;
if(options.includeDebugNames!==void 0&&options.includeDebugNames){values=selected.map(id=>{return {value:CAP_DATA[id],signal:CAP_NAMES[id]};});}else{values=selected.map(id=>CAP_DATA[id]);}
DECLARATIONS
LOOP
return {signals:named,positionalSignals:positional,keyedSignals:keyed,errors:CAP_ERRORS.toArray()};
}"#;
const LOOP_BODY: &str = r#"let metadata=CAP_META[selected[i]];
let receiver,method,value;
if(Object.prototype.hasOwnProperty.call(metadata,"name")){
method=(typeof metadata.name==="number"?keyed:named).push;
value=[metadata.name,values[i]];
receiver=typeof metadata.name==="number"?keyed:named;
}else{receiver=positional;method=positional.push;value=values[i];}
method.call(receiver,value);"#;
const LOCAL_BRANCH: &str = r#"let metadata=CAP_META[selected[i]];
if(Object.prototype.hasOwnProperty.call(metadata,"name")){
let method=(typeof metadata.name==="number"?keyed:named).push;
let value=[metadata.name,values[i]];
method.call(typeof metadata.name==="number"?keyed:named,value);
}else{positional.push(values[i]);}"#;
static SPECS: LazyLock<Vec<CompleteSpec>> = LazyLock::new(|| {
    let mut result=Vec::new();
    for order in [["positional","keyed","named"],["positional","named","keyed"],["keyed","positional","named"],["keyed","named","positional"],["named","positional","keyed"],["named","keyed","positional"]] {
        let declarations=order.map(|name|format!("let {name}=[];")).join("");
        for body in [LOOP_BODY,LOCAL_BRANCH] {
            for layout in 0..3 {
                let loop_=match layout {
                    0=>format!("for(let i=0;i<values.length;i++){{{body}}}"),
                    1=>format!("let i=0;for(;i<values.length;i++){{{body}}}"),
                    _=>format!("let i=0;while(i<values.length){{{body}++i;}}"),
                };
                let source=BASE.replace("DECLARATIONS",&declarations).replace("LOOP",&loop_);
                for split_notify in [false,true] {
                    let source=if split_notify { source.replace("if(options.notifyLastChance===void 0||options.notifyLastChance)","let notify;if(options.notifyLastChance===void 0){notify=true;}else{notify=options.notifyLastChance;}if(notify)") } else { source.clone() };
                    for split_record in [false,true] {
                        let source=if split_record { source.replace("return {value:CAP_DATA[id],signal:CAP_NAMES[id]};", "let record={value:void 0,signal:void 0};record.value=CAP_DATA[id];record.signal=CAP_NAMES[id];return record;") } else {source.clone()};
                        result.push(compile_complete(&source));
                    }
                }
            }
        }
    }
    result
});
pub(super) fn evaluate(model:&Model<'_, '_>,site:FunctionSite,f:&Facts<'_>)->Vec<Proposal>{
    if !f.members.contains("signalFilter") || !f.members.contains("notifyLastChance") || !complete_capture_specs(model,site,&SPECS) {return vec![];}
    let mut claim=proposal("buildPartitionedSignalSnapshot","observables_partitioned_signal_snapshot","Complete own options/default-filter protocol; optional per-ID notifications with exact null guard and void0 receiver, debug value/name records or raw values, full traversal partitions metadata with own name into numeric-keyed or string-named arrays and otherwise positional values, then returns those exact arrays and captured toArray result. Every callback, alias, append receiver, index, scoped source collection and effect order matched; captured collections are opaque runtime reads, with no type, availability, native identity or external meaning inferred",f);
    claim.evidence=vec![site.span.into()];
    vec![claim]
}
