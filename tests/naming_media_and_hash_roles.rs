use shape::naming::name_threads;

const MEDIA: &str = r#"function main(){
function $THREAD_8_101(input,feature,start,fallback){
let unit=fallback===void 0?"":fallback;
let low=start;
let attempts=50;
let result=0;
let high;
for(;;){
high=low;
if(((p,q,r,s,t)=>{return q.call(p,r+s+t+")").matches;})(input,input.matchMedia,"("+feature+": ",high,unit)||attempts<=0)break;
--attempts;
low=high*2;
result=high;
}
let limit=50;
while(result<high&&limit>0){
--limit;
let middle=Math.floor((result+high)/2);
if(((p,q,r,s,t)=>{return q.call(p,r+s+t+")").matches;})(input,input.matchMedia,"("+feature+": ",middle,unit)){high=middle;continue;}else{result=middle+1;continue;}
}
return result;
}
return $THREAD_8_101;
}
main();"#;

const TOUCH: &str = r#"function main(){
let sink={put:function(v){return v;}};
let errors={error:function(e){return e;}};
let ready=false;
function sample(){return 7;}
function $THREAD_8_202(event){
try{
let meta={instanceOfUIEvent:event instanceof UIEvent,markedAsTrusted:!!event.isTrusted};
let list=[];
let touches=event[event.touches&&event.touches.length===0?"changedTouches":"touches"];
if(touches&&touches.length!==0){
let count=touches.length;
let i=0;
for(;i<count;i++){
let touch=touches[i];
let target=touch.target;
target=target instanceof Element?target.getBoundingClientRect():null;
const localX=target==null?0:Math.round(touch.clientX-target.left);
target=target==null?0:Math.round(touch.clientY-target.top);
list.push({a:Math.round(touch.pageX),b:Math.round(touch.pageY),c:localX,d:target,e:touch.force});
}
}
let payload={a:[true,false],b:sample()};
payload.c=list;
payload.d=meta.instanceOfUIEvent;
payload.e=meta.markedAsTrusted;
sink.put(payload);
ready=true;
}catch(e){
errors.error(e);
}
}
return $THREAD_8_202;
}
main();"#;

const RANDOM_FIXED: &str = r#"function main(){
let config={},cache={},registry={};
function stateMix(x){return x;}
function fieldMix(a,b,c,d){return d;}
let channel={next:function(v){return v;},complete:function(){return 0;}};
function $THREAD_8_303(values){
let state=Math.random()*4294967296;
let bytes=[(state|0)>>>24,(state|0)>>16&255,(state|0)>>8&255,(state|0)&255];
state=stateMix((state|0)^2367643303);
let acc=fieldMix(state,values[4],bytes,3);
acc=fieldMix(state,values[1],bytes,acc);
acc=fieldMix(state,values[3],bytes,acc);
acc=fieldMix(state,values[0],bytes,acc);
fieldMix(state,values[2],bytes,acc);
channel.next([[],bytes]);
channel.complete();
}
return $THREAD_8_303;
}
main();"#;

const RANDOM_DIGEST: &str = r#"function main(){
let config={},cache={},registry={};
function stateMix(x){return x;}
function fieldMix(a,b,c,d){return d;}
let channel={next:function(v){return v;},complete:function(){return 0;}};
function $THREAD_8_404(values){
let state=Math.random()*4294967296|0;
let bytes=[state>>>24,state>>16&255,state>>8&255,state&255];
let out=[];
state=stateMix(state^2367643303);
let count=values.length;
let acc=fieldMix(state,count,bytes,3);
while(count-1>=0){
acc=fieldMix(state,values[count-1],bytes,acc);
--count;
}
channel.next([out,bytes]);
channel.complete();
}
return $THREAD_8_404;
}
main();"#;

/// Media query search with an inline string builder.
const MEDIA_INLINE_BUILDER: &str = r#"function $THREAD_8_44282($AK, $AL, $AM, $AN) {
			let $AO = $AN === void 0 ? "" : $AN;
			let i = $AM;
			let j = 0;
			let k = 50;
			let $AP, $AQ, i_2;
			do {
				$AP = i;
				$AQ = !(($PARAMA, $PARAMB, $PARAMC) => {
				return $PARAMB.call($PARAMA, $PARAMC).matches;
			})($AK, ($AK.matchMedia), ((($PARAMA, $PARAMB, $PARAMC) => {
				return $PARAMA + $PARAMB + $PARAMC;
			})(("(" + $AL + ": "), $AP, $AO) + ")"));
			} while ($AQ && k > 0 && (i_2 = Number(k) - 1, i = $AP * 2, j = $AP, k = i_2, true));
			let $AR = 50;
			while (j < $AP && $AR > 0) {
				--$AR;
				let $AS = Math.floor((j + $AP) / 2);
				if ((($PARAMA, $PARAMB, $PARAMC) => {
				return $PARAMB.call($PARAMA, $PARAMC).matches;
			})($AK, ($AK.matchMedia), ((($PARAMA, $PARAMB, $PARAMC) => {
				return $PARAMA + $PARAMB + $PARAMC;
			})(("(" + $AL + ": "), $AS, $AO) + ")"))) {
					$AP = $AS;
					continue;
				} else {
					j = $AS + 1;
					continue;
				}
			}
			return j;
		}"#;

/// Media query search with a prefix helper and reordered suffix operands.
const MEDIA_SFOR_PREFIX5: &str = r#"function $THREAD_5_126028($BP, $BQ, $BR, $BS) {
			let $BT = $BS === void 0 ? "" : $BS;
			let $BU = $BR;
			let $BV = 0;
			let $BW = 50;
			let $BX;
			for (;;) {
				$BX = $BU;
				if ((($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME) => {
				return $PARAMB.call($PARAMA, $PARAMC + $PARAME + $PARAMD + ")").matches;
			})($BP, ($BP.matchMedia), ("(" + $BQ + ": "), $BT, $BX) || $BW <= 0) break;
				$BU = $BX * 2;
				$BV = $BX;
				--$BW;
			}
			let $BY = 50;
			while ($BV < $BX && $BY > 0) {
				--$BY;
				let $BZ = Math.floor(($BV + $BX) / 2);
				if ((($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME) => {
				return $PARAMB.call($PARAMA, $PARAMC + $PARAME + $PARAMD + ")").matches;
			})($BP, ($BP.matchMedia), ("(" + $BQ + ": "), $BT, $BZ)) {
					$BX = $BZ;
					continue;
				} else {
					$BV = $BZ + 1;
					continue;
				}
			}
			return $BV;
		}"#;

/// Media query search with a direct string concatenation.
const MEDIA_DIRECT_QUERY: &str = r#"function $THREAD_1_83465($BT, $BU, $BV, $BW) {
			let $BX = $BW === void 0 ? "" : $BW;
			let i = $BV;
			let j = 50;
			let k = 0;
			let $BY, $BZ, i_2;
			do {
				$BY = i;
				$BZ = !(($PARAMA, $PARAMB, $PARAMC) => {
				return $PARAMB.call($PARAMA, $PARAMC).matches;
			})($BT, ($BT.matchMedia), ("(" + $BU + ": " + $BY + $BX + ")"));
			} while ($BZ && j > 0 && (i_2 = Number(j) - 1, i = $BY * 2, j = i_2, k = $BY, true));
			let $CA = 50;
			while (k < $BY && $CA > 0) {
				--$CA;
				let $CB = Math.floor((k + $BY) / 2);
				if ((($PARAMA, $PARAMB, $PARAMC) => {
				return $PARAMB.call($PARAMA, $PARAMC).matches;
			})($BT, ($BT.matchMedia), ("(" + $BU + ": " + $CB + $BX + ")"))) {
					$BY = $CB;
					continue;
				} else {
					k = $CB + 1;
					continue;
				}
			}
			return k;
		}"#;

/// Media query search with helper-built query strings.
const MEDIA_SFOR_PREFIX6: &str = r#"function $THREAD_6_148431($BU, $BV, $BW, $BX) {
			let $BY = $BX === void 0 ? "" : $BX;
			let $BZ = $BW;
			let $CA = 0;
			let $CB = 50;
			let $CC;
			for (;;) {
				$CC = $BZ;
				if ((($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME, $PARAMF) => {
				return $PARAMB.call($PARAMA, $PARAMC + $PARAMD + $PARAMF + $PARAME + ")").matches;
			})($BU, ($BU.matchMedia), ("(" + $BV), ": ", $BY, $CC) || $CB <= 0) break;
				--$CB;
				$BZ = $CC * 2;
				$CA = $CC;
			}
			let $CD = 50;
			while ($CA < $CC && $CD > 0) {
				--$CD;
				let $CE = Math.floor(($CA + $CC) / 2);
				if ((($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME, $PARAMF) => {
				return $PARAMB.call($PARAMA, $PARAMC + $PARAMD + $PARAMF + $PARAME + ")").matches;
			})($BU, ($BU.matchMedia), ("(" + $BV), ": ", $BY, $CE)) {
					$CC = $CE;
					continue;
				} else {
					$CA = $CE + 1;
					continue;
				}
			}
			return $CA;
		}"#;

/// Media query search with captured query components.
const MEDIA_SFOR_PREFIX5_SPLIT: &str = r#"function $THREAD_1_75867($BS, $BT, $BU, $BV) {
					let $BW = $BV === void 0 ? "" : $BV;
					let $BX = $BU;
					let $BY = 50;
					let $BZ = 0;
					let $CA;
					for (;;) {
						$CA = $BX;
						if ((($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME) => {
						return $PARAMB.call($PARAMA, $PARAMC + ": " + $PARAME + $PARAMD + ")").matches;
					})($BS, ($BS.matchMedia), ("(" + $BT), $BW, $CA) || $BY <= 0) break;
						--$BY;
						$BX = $CA * 2;
						$BZ = $CA;
					}
					let $CB = 50;
					while ($BZ < $CA && $CB > 0) {
						--$CB;
						let $CC = Math.floor(($BZ + $CA) / 2);
						if ((($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME) => {
						return $PARAMB.call($PARAMA, $PARAMC + ": " + $PARAME + $PARAMD + ")").matches;
					})($BS, ($BS.matchMedia), ("(" + $BT), $BW, $CC)) {
							$CA = $CC;
							continue;
						} else {
							$BZ = $CC + 1;
							continue;
						}
					}
					return $BZ;
				}"#;

/// Media query search with an alternate helper call layout.
const MEDIA_SFOR_PREFIX5_SPLIT_B: &str = r#"function $THREAD_6_133770($BL, $BM, $BN, $BO) {
					let $BP = $BO === void 0 ? "" : $BO;
					let $BQ = $BN;
					let $BR = 50;
					let $BS = 0;
					let $BT;
					for (;;) {
						$BT = $BQ;
						if ((($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME) => {
						return $PARAMB.call($PARAMA, $PARAMC + ": " + $PARAME + $PARAMD + ")").matches;
					})($BL, ($BL.matchMedia), ("(" + $BM), $BP, $BT) || $BR <= 0) break;
						--$BR;
						$BQ = $BT * 2;
						$BS = $BT;
					}
					let $BU = 50;
					while ($BS < $BT && $BU > 0) {
						--$BU;
						let $BV = Math.floor(($BS + $BT) / 2);
						if ((($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME) => {
						return $PARAMB.call($PARAMA, $PARAMC + ": " + $PARAME + $PARAMD + ")").matches;
					})($BL, ($BL.matchMedia), ("(" + $BM), $BP, $BV)) {
							$BT = $BV;
							continue;
						} else {
							$BS = $BV + 1;
							continue;
						}
					}
					return $BS;
				}"#;

/// Media query search with a default unit and bounded expansion.
const MEDIA_DOWHILE_PREFIX5: &str = r#"function $THREAD_9_31156($AN, $AO, $AP, $AQ) {
			let $AR = $AQ === void 0 ? "" : $AQ;
			let i = $AP;
			let j = 50;
			let k = 0;
			let $AS, $AT, i_2;
			do {
				$AS = i;
				$AT = !(($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME) => {
				return $PARAMB.call($PARAMA, $PARAMC + ": " + $PARAME + $PARAMD + ")").matches;
			})($AN, ($AN.matchMedia), ("(" + $AO), $AR, $AS);
			} while ($AT && j > 0 && (i_2 = Number(j) - 1, i = $AS * 2, j = i_2, k = $AS, true));
			let $AU = 50;
			while (k < $AS && $AU > 0) {
				--$AU;
				let $AV = Math.floor((k + $AS) / 2);
				if ((($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME) => {
				return $PARAMB.call($PARAMA, $PARAMC + ": " + $PARAME + $PARAMD + ")").matches;
			})($AN, ($AN.matchMedia), ("(" + $AO), $AR, $AV)) {
					$AS = $AV;
					continue;
				} else {
					k = $AV + 1;
					continue;
				}
			}
			return k;
		}"#;

/// Media query search with separate query and match helpers.
const MEDIA_NESTED_GROW_THEN_SEARCH: &str = r#"function $THREAD_9_99663($AM, $AN, $AO, $AP) {
			let $AQ = $AP === void 0 ? "" : $AP;
			let $AR = $AO;
			let $AS = 0;
			let $AT = 50;
			for (;;) {
				let $AU = $AR;
				if (!(($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME) => {
				return $PARAMB.call($PARAMA, $PARAMC + ": " + $PARAME + $PARAMD + ")").matches;
			})($AM, ($AM.matchMedia), ("(" + $AN), $AQ, $AU) && $AT > 0) {
					--$AT;
					$AR = $AU * 2;
					$AS = $AU;
					continue;
				}
				let $AV = 50;
				let $AW;
				for (;;) {
					let $AX, $AY;
					if ($AS < $AU) {
						$AX = $AV > 0;
						$AY = 3;
					} else {
						$AX = false;
						$AY = $AW;
					}
					if (!$AX) break;
					--$AV;
					let $AZ = Math.floor(($AS + $AU) / 2);
					if ((($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME) => {
				return $PARAMB.call($PARAMA, $PARAMC + ": " + $PARAME + $PARAMD + ")").matches;
			})($AM, ($AM.matchMedia), ("(" + $AN), $AQ, $AZ)) {
						$AU = $AZ;
						$AW = $AY;
						continue;
					} else {
						$AS = $AZ + 1;
						$AW = $AY;
						continue;
					}
				}
				return $AS;
			}
		}"#;

/// Media query search with a captured match helper.
const MEDIA_CAPTURED_INLINE_BUILDER: &str = r#"function $THREAD_1_149441($BR, $BS, $BT, $BU) {
					let $BV = $BU === void 0 ? "" : $BU;
					let i = $BT;
					let j = 50;
					let k = 0;
					let $BW, $BX, i_2;
					do {
						$BW = i;
						$BX = !(($PARAMA, $PARAMB, $PARAMC) => {
						return $PARAMB.call($PARAMA, $PARAMC).matches;
					})($BR, ($BR.matchMedia), ("(" + $BS + ": " + $BW + $BV + ")"));
					} while ($BX && j > 0 && (i_2 = Number(j) - 1, i = $BW * 2, j = i_2, k = $BW, true));
					let $BY = 50;
					while (k < $BW && $BY > 0) {
						--$BY;
						let $BZ = Math.floor((k + $BW) / 2);
						if ((($PARAMA, $PARAMB, $PARAMC) => {
						return $PARAMB.call($PARAMA, $PARAMC).matches;
					})($BR, ($BR.matchMedia), ("(" + $BS + ": " + $BZ + $BV + ")"))) {
							$BW = $BZ;
							continue;
						} else {
							k = $BZ + 1;
							continue;
						}
					}
					return k;
				}"#;

fn media_source(body: &str, target: &str) -> String {
    format!("function main(){{\n{body}\nreturn {target};\n}}\nmain();")
}

/// A negative replacement must actually change the fixture so the refusal test
/// cannot pass vacuously.
fn media_changed(source: &str, from: &str, to: &str) -> String {
    let changed = source.replace(from, to);
    assert_ne!(
        changed.as_str(),
        source,
        "near-miss replacement {from:?} -> {to:?} did not change the fixture"
    );
    changed
}

fn expected_evaluators(name: &str) -> &'static [&'static str] {
    match name {
        "measureMaxMediaFeatureValue" => &["runtime_media_feature_bisection"],
        "recordTouchSample" => &["events_touch_payload_record"],
        "emitSeededHashEncoding" => &[
            "encoding_seeded_hash_random_bytes",
            "encoding_seeded_hash_digest",
            "encoding_seeded_hash_encoding",
        ],
        "buildSignalReport" => &["signals_signal_report_builder"],
        _ => &[],
    }
}

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.function_roles.iter().any(|role| {
            role.candidates.iter().any(|candidate| candidate == name)
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| expected_evaluators(name).contains(&evaluator.as_str()))
        }) && r.report.outcomes.iter().any(|o| o.binding.name == target)
    })
}

fn replay(
    source: &str,
    result: &str,
    edits: &[shape::naming::program::ProgramSourceEdit],
) -> String {
    let mut replay = source.to_owned();
    let mut edits = edits.to_vec();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result);
    replay
}

/// A negative source must still parse and analyze; only the naming proof is
/// expected to fail. This keeps the refusal tests from passing vacuously.
fn refused(source: &str, target: &str, name: &str) {
    let result = name_threads(source, true).unwrap_or_else(|error| {
        panic!("negative source must parse and analyze: {error}\n{source}")
    });
    result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes:\n{source}"));
    assert!(
        !result.report.function_roles.iter().any(|role| {
            role.candidates.iter().any(|candidate| candidate == name)
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| expected_evaluators(name).contains(&evaluator.as_str()))
        }),
        "accepted {target} as {name}:\n{source}"
    );
}

#[test]
fn media_bisection_is_source_backed() {
    let alpha = MEDIA
        .replace("$THREAD_8_101", "$THREAD_71_887766_2")
        .replace(
            "input,feature,start,fallback",
            "$KEY_10,$KEY_11,$KEY_12,$KEY_13",
        )
        .replace("input", "$KEY_10")
        .replace("feature", "$KEY_11")
        .replace("start", "$KEY_12")
        .replace("fallback", "$KEY_13")
        .replace("unit", "$KEY_14")
        .replace("low", "$KEY_15")
        .replace("attempts", "$KEY_16")
        .replace("result", "$KEY_17")
        .replace("high", "$KEY_18")
        .replace("limit", "$KEY_19")
        .replace("middle", "$KEY_20");
    for (source, target) in [(MEDIA, "$THREAD_8_101"), (&alpha, "$THREAD_71_887766_2")] {
        assert!(
            named(source, target, "measureMaxMediaFeatureValue"),
            "{source}"
        );
        let result = name_threads(source, true).unwrap();
        replay(source, &result.source, &result.report.edits);
    }
}

#[test]
fn media_bisection_refuses_operation_and_bound_changes() {
    for (from, to) in [
        ("matchMedia", "matchMediaQuery"),
        ("Math.floor", "Math.ceil"),
        ("\")\"", "\"]\""),
        ("result<high", "result<=high"),
        ("high*2", "high*3"),
        ("return result;", "return high;"),
        ("attempts<=0", "attempts<0"),
        ("middle+1", "middle+2"),
        ("--limit", "++limit"),
        ("result=middle+1", "result=middle"),
        ("q.call(p", "q.call(other"),
        ("input.matchMedia", "input.media"),
        ("high=middle", "high=middle+1"),
    ] {
        let source = MEDIA.replace(from, to);
        assert!(
            !named(&source, "$THREAD_8_101", "measureMaxMediaFeatureValue"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn media_bisection_refuses_unsafe_formals_and_initializers() {
    for source in [
        MEDIA.replace(
            "(input,feature,start,fallback)",
            "({input},feature,start,fallback)",
        ),
        MEDIA.replace(
            "(input,feature,start,fallback)",
            "(input,feature,start,fallback=0)",
        ),
        MEDIA.replace(
            "(input,feature,start,fallback)",
            "(input,feature,start,...fallback)",
        ),
        MEDIA.replace("let high;", "let high;let high;"),
        MEDIA.replace("function $THREAD_8_101", "async function $THREAD_8_101"),
    ] {
        assert!(
            !named(&source, "$THREAD_8_101", "measureMaxMediaFeatureValue"),
            "{source}"
        );
    }
}

#[test]
fn media_bisection_accepts_actual_variant_bodies() {
    for (body, target) in [
        (MEDIA_INLINE_BUILDER, "$THREAD_8_44282"),
        (MEDIA_SFOR_PREFIX5, "$THREAD_5_126028"),
        (MEDIA_DIRECT_QUERY, "$THREAD_1_83465"),
        (MEDIA_SFOR_PREFIX6, "$THREAD_6_148431"),
        (MEDIA_SFOR_PREFIX5_SPLIT, "$THREAD_1_75867"),
        (MEDIA_SFOR_PREFIX5_SPLIT_B, "$THREAD_6_133770"),
        (MEDIA_DOWHILE_PREFIX5, "$THREAD_9_31156"),
        (MEDIA_NESTED_GROW_THEN_SEARCH, "$THREAD_9_99663"),
        (MEDIA_CAPTURED_INLINE_BUILDER, "$THREAD_1_149441"),
    ] {
        let source = media_source(body, target);
        assert!(
            named(&source, target, "measureMaxMediaFeatureValue"),
            "{target}"
        );
        let result = name_threads(&source, true).unwrap();
        replay(&source, &result.source, &result.report.edits);
    }
}

#[test]
fn media_bisection_refuses_do_while_variant_near_misses() {
    let base = media_source(MEDIA_INLINE_BUILDER, "$THREAD_8_44282");
    for (from, to) in [
        ("Number(k) - 1", "Number(k) + 1"),
        ("k > 0", "k >= 0"),
        ("$AP * 2", "$AP * 3"),
        (
            "i_2 = Number(k) - 1, i = $AP * 2, j = $AP, k = i_2",
            "k = i_2, i = $AP * 2, j = $AP, i_2 = Number(k) - 1",
        ),
        ("let $AP, $AQ, i_2;", "let $AP = 0, $AQ, i_2;"),
        ("$AQ = !(", "$AQ = ("),
        ("$PARAMB.call($PARAMA", "$PARAMB.call($AK"),
        ("$AK.matchMedia", "$AK.media"),
        ("return j;", "return $AP;"),
        ("j = $AS + 1", "j = $AS + 2"),
        ("$AP = $AS", "$AP = $AS + 1"),
        ("while (j < $AP && $AR > 0)", "while (j <= $AP && $AR > 0)"),
        ("Math.floor", "Math.ceil"),
    ] {
        let source = media_changed(&base, from, to);
        refused(&source, "$THREAD_8_44282", "measureMaxMediaFeatureValue");
    }
}

#[test]
fn media_bisection_allows_dependency_free_do_while_reordering() {
    let base = media_source(MEDIA_INLINE_BUILDER, "$THREAD_8_44282");
    assert!(
        named(&base, "$THREAD_8_44282", "measureMaxMediaFeatureValue"),
        "the actual fixture order must still be named"
    );
    for (from, to) in [
        (
            "i_2 = Number(k) - 1, i = $AP * 2, j = $AP, k = i_2",
            "i_2 = Number(k) - 1, j = $AP, i = $AP * 2, k = i_2",
        ),
        (
            "i_2 = Number(k) - 1, i = $AP * 2, j = $AP, k = i_2",
            "j = $AP, i = $AP * 2, i_2 = Number(k) - 1, k = i_2",
        ),
    ] {
        let source = media_changed(&base, from, to);
        assert!(
            named(&source, "$THREAD_8_44282", "measureMaxMediaFeatureValue"),
            "dependency-free reorder must still be named: {to}"
        );
        let result = name_threads(&source, true).unwrap();
        replay(&source, &result.source, &result.report.edits);
    }
}

#[test]
fn media_bisection_refuses_prefix_probe_near_misses() {
    let five = media_source(MEDIA_SFOR_PREFIX5, "$THREAD_5_126028");
    for (from, to) in [
        ("$BW <= 0", "$BW < 0"),
        ("$BX * 2", "$BX * 3"),
        ("$BV = $BZ + 1", "$BV = $BZ + 2"),
        ("$BP.matchMedia", "$BP.media"),
        ("$PARAMB.call($PARAMA", "$PARAMB.call($BP"),
        (", $BT, $BX)", ", $BX, $BT)"),
        ("return $BV;", "return $BX;"),
        ("Math.floor", "Math.ceil"),
    ] {
        let source = media_changed(&five, from, to);
        refused(&source, "$THREAD_5_126028", "measureMaxMediaFeatureValue");
    }
    let six = media_source(MEDIA_SFOR_PREFIX6, "$THREAD_6_148431");
    for (from, to) in [
        ("$CB <= 0", "$CB < 0"),
        ("$CC * 2", "$CC * 3"),
        ("$CA = $CE + 1", "$CA = $CE + 2"),
        ("$BU.matchMedia", "$BU.media"),
        ("$PARAMB.call($PARAMA", "$PARAMB.call($BU"),
        (", \": \", $BY, $CC)", ", \": \", $CC, $BY)"),
        ("return $CA;", "return $CC;"),
        ("Math.floor", "Math.ceil"),
    ] {
        let source = media_changed(&six, from, to);
        refused(&source, "$THREAD_6_148431", "measureMaxMediaFeatureValue");
    }
    let split = media_source(MEDIA_SFOR_PREFIX5_SPLIT, "$THREAD_1_75867");
    for (from, to) in [
        (
            "$PARAMC + \": \" + $PARAME + $PARAMD + \")\"",
            "$PARAMC + \": \" + $PARAMD + $PARAME + \")\"",
        ),
        ("$BX = $CA * 2", "$BX = $CA * 3"),
        ("return $BZ;", "return $CA;"),
    ] {
        let source = media_changed(&split, from, to);
        refused(&source, "$THREAD_1_75867", "measureMaxMediaFeatureValue");
    }
}

#[test]
fn media_bisection_refuses_direct_inline_query_near_misses() {
    let base = media_source(MEDIA_DIRECT_QUERY, "$THREAD_1_83465");
    for (from, to) in [
        (
            "(\"(\" + $BU + \": \" + $BY + $BX + \")\")",
            "(\"(\" + $BU + \": \" + $BX + $BY + \")\")",
        ),
        ("$BZ && j > 0", "$BZ && j >= 0"),
        ("Number(j) - 1", "Number(j) + 1"),
        ("$BY * 2", "$BY * 3"),
        ("let $BY, $BZ, i_2;", "let $BY = 0, $BZ, i_2;"),
        ("return k;", "return $BY;"),
        ("$BY = $CB", "$BY = $CB + 1"),
        ("k = $CB + 1", "k = $CB"),
        ("Math.floor", "Math.ceil"),
    ] {
        let source = media_changed(&base, from, to);
        refused(&source, "$THREAD_1_83465", "measureMaxMediaFeatureValue");
    }
}

#[test]
fn media_bisection_refuses_nested_variant_near_misses() {
    let base = media_source(MEDIA_NESTED_GROW_THEN_SEARCH, "$THREAD_9_99663");
    for (from, to) in [
        ("$AS < $AU", "$AS <= $AU"),
        ("$AT > 0", "$AT >= 0"),
        ("$AR = $AU * 2", "$AR = $AU * 3"),
        ("$AV > 0", "$AV >= 0"),
        ("$AS = $AZ + 1", "$AS = $AZ + 2"),
        ("$AU = $AZ", "$AU = $AZ + 1"),
        ("$AW = $AY", "$AW = $AZ"),
        ("return $AS;", "return $AU;"),
        ("if (!$AX) break;", "if ($AX) break;"),
        ("$AW = $AY;", "continue;"),
        ("Math.floor", "Math.ceil"),
    ] {
        let source = media_changed(&base, from, to);
        refused(&source, "$THREAD_9_99663", "measureMaxMediaFeatureValue");
    }
}

#[test]
fn touch_payload_is_source_backed_across_variants() {
    let reordered = TOUCH.replace(
        "let meta={instanceOfUIEvent:event instanceof UIEvent,markedAsTrusted:!!event.isTrusted};\nlet list=[];",
        "let list=[];\nlet meta={instanceOfUIEvent:event instanceof UIEvent,markedAsTrusted:!!event.isTrusted};",
    );
    let flags = TOUCH.replace("[true,false]", "[false,false]");
    let alpha = TOUCH
        .replace("$THREAD_8_202", "$THREAD_300_761_4")
        .replace("event", "$KEY_40")
        .replace("meta", "$KEY_41")
        .replace("list", "$KEY_42")
        .replace("count", "$KEY_44")
        .replace("localX", "$KEY_47")
        .replace("payload", "$KEY_48")
        .replace("ready", "$KEY_49")
        .replace("sink", "$KEY_50")
        .replace("errors", "$KEY_51");
    for (source, target) in [
        (TOUCH, "$THREAD_8_202"),
        (&reordered, "$THREAD_8_202"),
        (&flags, "$THREAD_8_202"),
        (&alpha, "$THREAD_300_761_4"),
    ] {
        assert!(named(source, target, "recordTouchSample"), "{source}");
    }
}

#[test]
fn touch_payload_refuses_operation_identity_changes() {
    for (from, to) in [
        ("sink.put(payload)", "sink.set(payload)"),
        ("getBoundingClientRect", "getBoundingClientRectX"),
        ("Math.round", "Math.floor"),
        ("!!event.isTrusted", "!event.isTrusted"),
        ("changedTouches", "touches"),
        ("payload.c=list", "payload.c=[]"),
        (
            "payload.d=meta.instanceOfUIEvent",
            "payload.d=meta.markedAsTrusted",
        ),
        (
            "payload.e=meta.markedAsTrusted",
            "payload.e=meta.instanceOfUIEvent",
        ),
        ("errors.error(e)", "errors.report(e)"),
        ("ready=true", "ready=false"),
        ("touch.force", "touch.pressure"),
        ("target.left", "target.right"),
        ("list.push", "other.push"),
        ("UIEvent", "CustomEvent"),
        ("Element", "Node"),
    ] {
        let source = TOUCH.replace(from, to);
        assert!(
            !named(&source, "$THREAD_8_202", "recordTouchSample"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn touch_payload_refuses_unsafe_captures_and_formals() {
    for source in [
        TOUCH.replace(
            "function $THREAD_8_202(event)",
            "function $THREAD_8_202({event})",
        ),
        TOUCH.replace(
            "function $THREAD_8_202(event)",
            "function $THREAD_8_202(...event)",
        ),
        TOUCH.replace("let sink=", "var sink;var sink="),
        TOUCH.replace("let ready=false;", "let ready=false;let ready;"),
        TOUCH.replace(
            "function $THREAD_8_202(event){",
            "function $THREAD_8_202(event){let sink={put:function(v){return v;}};",
        ),
    ] {
        assert!(
            !named(&source, "$THREAD_8_202", "recordTouchSample"),
            "{source}"
        );
    }
}

#[test]
fn random_fixed_is_source_backed() {
    let alpha = RANDOM_FIXED
        .replace("$THREAD_8_303", "$THREAD_912_99881_2")
        .replace("values", "$KEY_60")
        .replace("state", "$KEY_61")
        .replace("bytes", "$KEY_62")
        .replace("acc", "$KEY_63");
    for (source, target) in [
        (RANDOM_FIXED, "$THREAD_8_303"),
        (&alpha, "$THREAD_912_99881_2"),
    ] {
        assert!(named(source, target, "emitSeededHashEncoding"), "{source}");
    }
}

#[test]
fn random_fixed_refuses_order_api_and_mutation_changes() {
    for (from, to) in [
        ("values[4]", "values[0]"),
        ("values[1]", "values[2]"),
        ("values[3]", "values[1]"),
        ("channel.next", "channel.emit"),
        ("channel.complete", "channel.end"),
        ("[],bytes", "[out,bytes]"),
        ("stateMix((state|0)^", "stateMix((state|0)+"),
        ("Math.random()*4294967296", "1*4294967296"),
        (
            "fieldMix(state,values[2],bytes,acc);",
            "fieldMix(state,values[2],bytes,3);",
        ),
        ("let bytes=[", "let bytes=[0,"),
        ("let state=", "var state;var state="),
        (
            "function $THREAD_8_303(values)",
            "function $THREAD_8_303(values,extra)",
        ),
    ] {
        let source = RANDOM_FIXED.replace(from, to);
        assert!(
            !named(&source, "$THREAD_8_303", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn random_digest_is_source_backed() {
    let alpha = RANDOM_DIGEST
        .replace("$THREAD_8_404", "$THREAD_17_554433_2")
        .replace("values", "$KEY_70")
        .replace("stateMix", "__MIX__")
        .replace("state", "$KEY_71")
        .replace("__MIX__", "stateMix")
        .replace("bytes", "$KEY_72")
        .replace("count", "$KEY_73")
        .replace("acc", "$KEY_74")
        .replace("out", "$KEY_75");
    for (source, target) in [
        (RANDOM_DIGEST, "$THREAD_8_404"),
        (&alpha, "$THREAD_17_554433_2"),
    ] {
        assert!(named(source, target, "emitSeededHashEncoding"), "{source}");
    }
}

#[test]
fn random_digest_refuses_order_api_and_mutation_changes() {
    for (from, to) in [
        ("values[count-1]", "values[count]"),
        ("count-1>=0", "count-1>0"),
        ("--count", "++count"),
        ("channel.next([out,bytes])", "channel.next([bytes,out])"),
        (
            "fieldMix(state,values[count-1],bytes,acc)",
            "fieldMix(state,count,bytes,acc)",
        ),
        ("state^2367643303", "state|2367643303"),
        ("Math.random()*4294967296|0", "Math.random()*4294967296"),
        ("let out=[];", "let out=[0];"),
    ] {
        let source = RANDOM_DIGEST.replace(from, to);
        assert!(
            !named(&source, "$THREAD_8_404", "emitSeededHashEncoding"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn digest_field_writer_needs_an_independent_ordered_callee() {
    // The digest's first field mixer is called through a symbol that the loop
    // later reuses. A stable spelling alone is not a callable origin: a
    // non-callable constant, a captured parameter, an origin assigned after
    // the user, and a setup-callback assignment must all be refused.
    let writer = "function fieldMix(a,b,c,d){return d;}";
    let sources = [
        RANDOM_DIGEST.replace(writer, "const fieldMix=0;"),
        RANDOM_DIGEST
            .replace(writer, "")
            .replace("function main(){", "function main(fieldMix){")
            .replacen("main();", "main(function(a,b,c,d){return d;});", 1),
        RANDOM_DIGEST.replace(writer, "var fieldMix;").replace(
            "\nreturn $THREAD_8_404;\n}",
            "\nfieldMix=function(a,b,c,d){return d;};\nreturn $THREAD_8_404;\n}",
        ),
        RANDOM_DIGEST.replace(
            writer,
            "var fieldMix;register(()=>{fieldMix=function(a,b,c,d){return d;};});",
        ),
    ];
    for source in sources {
        refused(&source, "$THREAD_8_404", "emitSeededHashEncoding");
    }
}

#[test]
fn callable_origins_refuse_later_alias_hops() {
    // var alias=later;var later=fn;alias(): the later hop is copied before its
    // own initializer runs, so the callee has no initialization-before-use
    // proof even though the final value is a function declaration.
    for (source, target, name) in [
        (
            TOUCH.replace(
                "function sample(){return 7;}",
                "var sample=later;var later=fn;function fn(){return 7;}",
            ),
            "$THREAD_8_202",
            "recordTouchSample",
        ),
        (
            RANDOM_FIXED.replace(
                "function stateMix(x){return x;}",
                "var stateMix=later;var later=fn;function fn(x){return x;}",
            ),
            "$THREAD_8_303",
            "emitSeededHashEncoding",
        ),
        (
            RANDOM_FIXED.replace(
                "function fieldMix(a,b,c,d){return d;}",
                "var fieldMix=later;var later=fn;function fn(a,b,c,d){return d;}",
            ),
            "$THREAD_8_303",
            "emitSeededHashEncoding",
        ),
        (
            RANDOM_DIGEST.replace(
                "function stateMix(x){return x;}",
                "var stateMix=later;var later=fn;function fn(x){return x;}",
            ),
            "$THREAD_8_404",
            "emitSeededHashEncoding",
        ),
    ] {
        refused(&source, target, name);
    }
}

#[test]
fn callable_sole_assignments_must_be_straight_line_container_statements() {
    // A declaration without an initializer gets a callable origin only from a
    // sole later `=` in the declaration's own statement list. Assignments in a
    // setup callback, condition, loop header or resource declaration are not
    // that proof.
    for (fixture, callee, params, body, target, name) in [
        (
            TOUCH,
            "sample",
            "",
            "return 7;",
            "$THREAD_8_202",
            "recordTouchSample",
        ),
        (
            RANDOM_FIXED,
            "stateMix",
            "x",
            "return x;",
            "$THREAD_8_303",
            "emitSeededHashEncoding",
        ),
        (
            RANDOM_FIXED,
            "fieldMix",
            "a,b,c,d",
            "return d;",
            "$THREAD_8_303",
            "emitSeededHashEncoding",
        ),
        (
            RANDOM_DIGEST,
            "stateMix",
            "x",
            "return x;",
            "$THREAD_8_404",
            "emitSeededHashEncoding",
        ),
    ] {
        let declaration = format!("function {callee}({params}){{{body}}}");
        let replacement = format!("function __fn({params}){{{body}}}");
        for origin in [
            format!("var {callee};register(()=>{{{callee}=__fn;}});{replacement}"),
            format!("var {callee};if({callee}=__fn){{}}{replacement}"),
            format!("var {callee};for(;{callee}=__fn;){{break;}}{replacement}"),
            format!("using {callee}=__fn;{replacement}"),
        ] {
            let source = fixture.replace(&declaration, &origin);
            assert_ne!(
                source, fixture,
                "fixture declaration not replaced: {declaration}"
            );
            refused(&source, target, name);
        }
    }
}

#[test]
fn meaningful_names_and_collisions_preserve_existing_guards() {
    for (source, human, name) in [
        (
            MEDIA.replace("$THREAD_8_101", "humanMedia"),
            "humanMedia",
            "measureMaxMediaFeatureValue",
        ),
        (
            TOUCH.replace("$THREAD_8_202", "humanTouch"),
            "humanTouch",
            "recordTouchSample",
        ),
        (
            RANDOM_FIXED.replace("$THREAD_8_303", "humanRandom"),
            "humanRandom",
            "emitSeededHashEncoding",
        ),
        (
            SIGNAL_REPORT.replace("$THREAD_8_505", "humanSignal"),
            "humanSignal",
            "buildSignalReport",
        ),
    ] {
        assert_eq!(name_threads(&source, true).unwrap().source, source);
        let collision = source.replace(
            "function main(){",
            &format!("function main(){{let {name}=17;useValue({name});"),
        );
        assert!(!named(&collision, human, name), "{name} collision");
    }
}

const SIGNAL_REPORT: &str = r#"function main(){
let config={},cache={},registry={};
let state={},latest={},signalNames={};
let errors={toArray:function(){return [];}};
function $THREAD_8_505(options){
let opts=options===void 0?{}:options;
let filter=opts.signalFilter===void 0?config:opts.signalFilter;
if(opts.notifyLastChance===void 0||opts.notifyLastChance){
filter.forEach(key=>{
if(state[key]!=null){
state[key].call(void 0);
}
});
}
let mapped;
if(opts.includeDebugNames!==void 0&&opts.includeDebugNames){
mapped=filter.map(key=>{return {value:latest[key],signal:signalNames[key]};});
}else{
mapped=filter.map(key=>latest[key]);
}
let plain=[];
let positional=[];
let keyed=[];
let i=0;
for(;i<mapped.length;i++){
let record=registry[filter[i]];
let sink,push,item;
if(Object.prototype.hasOwnProperty.call(record,"name")){
sink=keyed;
push=sink.push;
item=[record.name,mapped[i]];
}else{
push=positional.push;
sink=positional;
item=mapped[i];
}
push.call(sink,item);
}
return {signals:plain,positionalSignals:positional,keyedSignals:keyed,errors:errors.toArray()};
}
return $THREAD_8_505;
}
main();"#;

#[test]
fn signal_report_is_source_backed() {
    let alpha = SIGNAL_REPORT
        .replace("$THREAD_8_505", "$THREAD_44_123456_3")
        .replace("options", "$KEY_80")
        .replace("opts", "$KEY_81")
        .replace("filter", "$KEY_82")
        .replace("mapped", "$KEY_83")
        .replace("plain", "$KEY_84")
        .replace("record", "$KEY_87");
    for (source, target) in [
        (SIGNAL_REPORT, "$THREAD_8_505"),
        (&alpha, "$THREAD_44_123456_3"),
    ] {
        assert!(named(source, target, "buildSignalReport"), "{source}");
        let result = name_threads(source, true).unwrap();
        replay(source, &result.source, &result.report.edits);
    }
}

#[test]
fn signal_report_refuses_option_api_and_routing_changes() {
    for (from, to) in [
        ("signalFilter", "signalSelector"),
        ("notifyLastChance", "notify"),
        ("includeDebugNames", "includeNames"),
        ("hasOwnProperty", "hasOwn"),
        ("record.name", "record.key"),
        ("push.call(sink,item)", "push.apply(sink,item)"),
        ("positionalSignals", "positions"),
        ("keyedSignals", "keyed"),
        ("errors.toArray()", "errors.items()"),
        ("mapped.length", "mapped.size"),
        ("sink=keyed", "sink=positional"),
        ("push=positional.push", "push=keyed.push"),
        ("let plain=[];", "let plain=[0];"),
        ("filter.map(key=>latest[key])", "filter.map(key=>latest[0])"),
        ("filter.forEach", "filter.map"),
        ("registry[filter[i]]", "registry[filter[0]]"),
    ] {
        let source = SIGNAL_REPORT.replace(from, to);
        assert!(
            !named(&source, "$THREAD_8_505", "buildSignalReport"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn signal_report_refuses_unsafe_formals_and_captures() {
    for source in [
        SIGNAL_REPORT.replace("(options)", "({options})"),
        SIGNAL_REPORT.replace("(options)", "(options,...rest)"),
        SIGNAL_REPORT.replace("let mapped;", "var mapped;var mapped;"),
        SIGNAL_REPORT.replace("let config={}", "var config;var config={}"),
        SIGNAL_REPORT.replace(
            "function $THREAD_8_505(options){",
            "function $THREAD_8_505(options){let config={};",
        ),
    ] {
        assert!(
            !named(&source, "$THREAD_8_505", "buildSignalReport"),
            "{source}"
        );
    }
}
