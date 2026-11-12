//! Complete options-to-envelope lowerings. All captured providers are opaque
//! runtime values. This rule establishes neither a signal collector identity
//! nor an encoder/hash algorithm from the envelope's public slots.
use super::*;
use super::snapshot::partitioned::syntax::{compile_complete,complete_capture_specs,CompleteSpec};
use std::sync::LazyLock;
const ENVELOPE: &str=r#"function envelope(input){
let options;if(input!=null){options=input;}else{options=Object.create(null);}
let filter=options.signalFilter===void 0?CAP_DEFAULT:options.signalFilter;
let encode=typeof(options.encode===void 0?null:options.encode)==="function"?options.encode===void 0?null:options.encode:CAP_ENCODE;
let result=Object.create(null);
ASSEMBLY
FINALIZE
result.f=CAP_PREFIX;
result.b=CAP_TAG(CAP_PREFIX+PAYLOAD).toString(36);
result.c="BUILD_TOKEN";
result.d=options.descriptor===void 0?"DESCRIPTOR":options.descriptor;
result.z="q";result.a=PAYLOAD;return result;
}"#;
const INLINE: &str=r#"encode=encode(CAP_TRANSFORM({signalFilter:filter,includeDebugNames:false,notifyLastChance:(options.notifyLastChance===void 0||options.notifyLastChance)??true}));"#;
const SPLIT: &str=r#"filter={signalFilter:filter,includeDebugNames:false,notifyLastChance:void 0};filter.notifyLastChance=(options.notifyLastChance===void 0||options.notifyLastChance)??true;encode=encode(CAP_TRANSFORM(filter));"#;
const BASE64: &str=r#"let alphabet=CAP_ALPHABET??"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=";
let bound=filter.length-filter.length%3;let cursor=0;let text="";
while(cursor<bound){let third=filter[cursor+2];text=text+(alphabet.charAt(filter[cursor]>>2)+alphabet.charAt((filter[cursor]&3)<<4|filter[cursor+1]>>4)+alphabet.charAt((filter[cursor+1]&15)<<2|third>>6)+alphabet.charAt(third&63));cursor+=3;}
if(filter.length%3===2){text=text+(alphabet.charAt(filter[cursor]>>2)+alphabet.charAt((filter[cursor]&3)<<4|filter[cursor+1]>>4)+alphabet.charAt((filter[cursor+1]&15)<<2));}else if(filter.length%3===1){filter=filter[cursor];text=text+(alphabet.charAt(filter>>2)+alphabet.charAt((filter&3)<<4));}"#;
static SPECS:LazyLock<Vec<CompleteSpec>>=LazyLock::new(||{
let mut out=vec![];
for split in [false,true]{let assembly=if split{SPLIT}else{INLINE};out.push(compile_complete(&ENVELOPE.replace("ASSEMBLY",assembly).replace("FINALIZE","").replace("PAYLOAD","encode")));}
let assembly=SPLIT.replace("let encode=", "const encode=").replace(",notifyLastChance:void 0", "").replace("encode=encode(CAP_TRANSFORM(filter));", "filter=encode(CAP_TRANSFORM(filter));");
let source=ENVELOPE.replace("let encode=","const encode=").replace("ASSEMBLY",&assembly).replace("FINALIZE",BASE64).replace("PAYLOAD","text");out.push(compile_complete(&source));out
});
const MAP:&str=r#"function reports(input){let options=input===void 0?{}:input;let debug=options.includeDebugNames!==void 0&&options.includeDebugNames;let notify=options.notifyLastChance===void 0||options.notifyLastChance;return CAP_RECORDS.map(record=>{return {beaconEndpoint:record.a,beaconData:CAP_TRANSFORM({signalFilter:record.b,includeDebugNames:debug,notifyLastChance:notify})};});}"#;
static MAP_SPEC:LazyLock<Vec<CompleteSpec>>=LazyLock::new(||vec![compile_complete(MAP)]);
pub(super) fn evaluate(model:&Model<'_, '_>,site:FunctionSite,f:&Facts<'_>)->Vec<Proposal>{
if !f.members.contains("notifyLastChance"){return vec![];}
let claim=if f.members.contains("descriptor")&&complete_capture_specs(model,site,&SPECS){
proposal("buildOptionsTransformEnvelope","signals_options_transform_envelope","Complete own input slot 0 nullish default, undefined-only filter/encode/descriptor defaults, callable option selection, false debug slot and exact nullish last-chance expression. Ordered calls pass the exact three-field options object to one captured opaque transform then selected opaque callable; optional fully owned unpadded base64url loop preserves every byte/index/remainder. Null-prototype result receives f, b=opaqueTag(f+payload).toString(36), opaque literal c, selected literal/input d, z=q and a=payload in exact order. Distinct captured variable identities only: no collector, encoder or hash purpose, initialization success, type or purity inferred from sibling evidence or envelope shape",f)
}else if f.members.contains("includeDebugNames")&&complete_capture_specs(model,site,&MAP_SPEC){
proposal("mapBeaconEndpointData","signals_beacon_endpoint_data_map","Complete own undefined-only options default, debug and notify expressions, captured records map callback, endpoint from item.a and data from the captured opaque callable with exactly signalFilter=item.b/includeDebugNames/notifyLastChance in order. All own callback, parameter, capture identities, arguments, effects and return slots matched; no collector purpose inferred for callable or records",f)
}else{return vec![];};vec![claim]
}
