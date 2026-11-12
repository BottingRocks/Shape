use shape::naming::name_threads;

// Compact portable copy of the signal serializer factory. Every consumer input
// is a scoped declaration, and the returned snapshot serializer reads all four
// signal members from its single formal in its own call frame.
const POSITIVE: &str = r#"function host(){
let capturedSentinelExtension, capturedDepthExtension;
function $THREAD_700_11(keysetsToOmit, reservedSlot, seedKey){
    let config;
    let seed = seedKey;
    let encodeConfig = {
        keysetsToOmit: keysetsToOmit,
        depthBound: 100,
        extensions: void 0
    };
    let extensions = { "0": capturedSentinelExtension };
    let depthExtension = capturedDepthExtension;
    if ("3735928559" in extensions) {
        depthExtension = {
            value: depthExtension,
            enumerable: true,
            configurable: true,
            writable: true
        };
        Object.defineProperty(extensions, "3735928559", depthExtension);
    } else {
        extensions[3735928559] = depthExtension;
    }
    encodeConfig.extensions = extensions;
    config = encodeConfig;
    function $THREAD_701_22(snapshot){
        let bytes = [];
        bytes.push(snapshot.errors.length, snapshot.positionalSignals.length,
            snapshot.keyedSignals.length, snapshot.signals.length);
        bytes.push(config.depthBound, seed);
        return bytes;
    }
    return $THREAD_701_22;
}
return $THREAD_700_11;
}
host();"#;

// Alpha/spelling variant: generated thread names, every local and captured
// spelling changes, the declaration order is swapped, the extension id is a
// different integer, the encode-config and descriptor member order is free, and
// the configuration uses the `let config = encodeConfig` declarator form.
const ALPHA: &str = r#"function host(){
let $KEY_900, $KEY_901;
function $THREAD_900_777($K1, $K2, $K3){
    let $L0 = $K3;
    let $L2 = { depthBound: 100, keysetsToOmit: $K1, extensions: void 0 };
    let $L3 = { "0": $KEY_900 };
    let $L4 = $KEY_901;
    if ("42" in $L3) {
        $L4 = { writable: true, value: $L4, configurable: true, enumerable: true };
        Object.defineProperty($L3, "42", $L4);
    } else {
        $L3[42] = $L4;
    }
    $L2.extensions = $L3;
    let $L5 = $L2;
    function $THREAD_901_888($S){
        let $A = [];
        $A.push($S.signals.length, $S.keyedSignals.length, $S.positionalSignals.length, $S.errors.length);
        $A.push($L5.depthBound, $L0);
        return $A;
    }
    return $THREAD_901_888;
}
return $THREAD_900_777;
}
host();"#;

// A returned captured function that also reads the signal members cannot stand
// in for the owned nested serializer.
const RETURNED_CAPTURED_DECOY: &str = r#"function host(){
let capturedSentinelExtension, capturedDepthExtension;
function capturedSerializer(snapshot){
    return snapshot.errors.length + snapshot.positionalSignals.length + snapshot.keyedSignals.length + snapshot.signals.length;
}
function $THREAD_700_11(keysetsToOmit, reservedSlot, seedKey){
    let seed = seedKey;
    let encodeConfig = { keysetsToOmit: keysetsToOmit, depthBound: 100, extensions: void 0 };
    let extensions = { "0": capturedSentinelExtension };
    let depthExtension = capturedDepthExtension;
    if ("3735928559" in extensions) {
        depthExtension = { value: depthExtension, enumerable: true, configurable: true, writable: true };
        Object.defineProperty(extensions, "3735928559", depthExtension);
    } else {
        extensions[3735928559] = depthExtension;
    }
    encodeConfig.extensions = extensions;
    let config = encodeConfig;
    function $THREAD_701_22(snapshot){
        let bytes = [];
        bytes.push(snapshot.errors.length, snapshot.positionalSignals.length,
            snapshot.keyedSignals.length, snapshot.signals.length);
        bytes.push(config.depthBound, seed);
        return bytes;
    }
    return capturedSerializer;
}
return $THREAD_700_11;
}
host();"#;

// The four field reads happen only inside a child arrow closure, so the
// serializer's own call frame does not own the proof.
const NESTED_CALLBACK_DECOY: &str = r#"function host(){
let capturedSentinelExtension, capturedDepthExtension;
function $THREAD_700_11(keysetsToOmit, reservedSlot, seedKey){
    let seed = seedKey;
    let encodeConfig = { keysetsToOmit: keysetsToOmit, depthBound: 100, extensions: void 0 };
    let extensions = { "0": capturedSentinelExtension };
    let depthExtension = capturedDepthExtension;
    if ("3735928559" in extensions) {
        depthExtension = { value: depthExtension, enumerable: true, configurable: true, writable: true };
        Object.defineProperty(extensions, "3735928559", depthExtension);
    } else {
        extensions[3735928559] = depthExtension;
    }
    encodeConfig.extensions = extensions;
    let config = encodeConfig;
    function $THREAD_701_22(snapshot){
        let bytes = [];
        let collect = () => [snapshot.errors, snapshot.positionalSignals,
            snapshot.keyedSignals, snapshot.signals];
        bytes.push(collect().length, config.depthBound, seed);
        return bytes;
    }
    return $THREAD_701_22;
}
return $THREAD_700_11;
}
host();"#;

// The depth extension point is a local that merely copies the captured sentinel
// under a captured-looking spelling; symbol identity, not the spelling, decides.
const SHADOWED_DEPTH_CAPTURE: &str = r#"function host(){
let capturedSentinelExtension;
function $THREAD_700_11(keysetsToOmit, reservedSlot, seedKey){
    let seed = seedKey;
    let encodeConfig = { keysetsToOmit: keysetsToOmit, depthBound: 100, extensions: void 0 };
    let extensions = { "0": capturedSentinelExtension };
    let capturedDepthExtension = capturedSentinelExtension;
    if ("3735928559" in extensions) {
        capturedDepthExtension = { value: capturedDepthExtension, enumerable: true, configurable: true, writable: true };
        Object.defineProperty(extensions, "3735928559", capturedDepthExtension);
    } else {
        extensions[3735928559] = capturedDepthExtension;
    }
    encodeConfig.extensions = extensions;
    let config = encodeConfig;
    function $THREAD_701_22(snapshot){
        let bytes = [];
        bytes.push(snapshot.errors.length, snapshot.positionalSignals.length,
            snapshot.keyedSignals.length, snapshot.signals.length);
        bytes.push(config.depthBound, seed);
        return bytes;
    }
    return $THREAD_701_22;
}
return $THREAD_700_11;
}
host();"#;

// Three of the four members are read from a captured decoy table, not from the
// serializer's own snapshot formal.
const RECEIVER_DECOY: &str = r#"function host(){
let capturedSentinelExtension, capturedDepthExtension, decoyTable;
function $THREAD_700_11(keysetsToOmit, reservedSlot, seedKey){
    let seed = seedKey;
    let encodeConfig = { keysetsToOmit: keysetsToOmit, depthBound: 100, extensions: void 0 };
    let extensions = { "0": capturedSentinelExtension };
    let depthExtension = capturedDepthExtension;
    if ("3735928559" in extensions) {
        depthExtension = { value: depthExtension, enumerable: true, configurable: true, writable: true };
        Object.defineProperty(extensions, "3735928559", depthExtension);
    } else {
        extensions[3735928559] = depthExtension;
    }
    encodeConfig.extensions = extensions;
    let config = encodeConfig;
    function $THREAD_701_22(snapshot){
        let bytes = [];
        bytes.push(snapshot.errors.length, decoyTable.positionalSignals.length,
            decoyTable.keyedSignals.length, decoyTable.signals.length);
        bytes.push(config.depthBound, seed);
        return bytes;
    }
    return $THREAD_701_22;
}
return $THREAD_700_11;
}
host();"#;

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.outcomes.iter().any(|o| {
            o.applied && o.binding.name == target && o.proposed_name.as_deref() == Some(name)
        })
    })
}

/// A negative source must still parse and analyze; only the naming proof is
/// expected to fail. This keeps refusals from passing vacuously.
fn refused(source: &str, target: &str, name: &str) {
    let result = name_threads(source, true)
        .unwrap_or_else(|error| panic!("negative source must parse and analyze: {error}\n{source}"));
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes:\n{source}"));
    assert!(
        !(outcome.applied && outcome.proposed_name.as_deref() == Some(name)),
        "accepted {target} as {name}:\n{source}"
    );
}

fn refuses(template: &str, target: &str, name: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        refused(&template.replace(from, to), target, name);
    }
}

#[test]
fn serializer_factory_is_named_for_portable_and_spelling_variants() {
    assert!(
        named(POSITIVE, "$THREAD_700_11", "createSignalSerializer"),
        "did not name the portable serializer factory"
    );
    assert!(
        named(ALPHA, "$THREAD_900_777", "createSignalSerializer"),
        "did not name the alpha/spelling serializer factory"
    );
}

#[test]
fn serializer_factory_requires_the_exact_extension_configuration() {
    refuses(
        POSITIVE,
        "$THREAD_700_11",
        "createSignalSerializer",
        &[
            ("keysetsToOmit: keysetsToOmit", "keysetsToOmit: reservedSlot"),
            ("depthBound: 100", "depthBound: 101"),
            ("extensions: void 0", "extensions: null"),
            ("{ \"0\": capturedSentinelExtension }", "{ \"1\": capturedSentinelExtension }"),
            (
                "let depthExtension = capturedDepthExtension;",
                "let depthExtension = capturedSentinelExtension;",
            ),
            (
                "Object.defineProperty(extensions, \"3735928559\", depthExtension);",
                "Object.defineProperty(extensions, \"3735928558\", depthExtension);",
            ),
            (
                "extensions[3735928559] = depthExtension;",
                "extensions[3735928558] = depthExtension;",
            ),
            (
                "Object.defineProperty(extensions, \"3735928559\", depthExtension);",
                "Reflect.defineProperty(extensions, \"3735928559\", depthExtension);",
            ),
            ("enumerable: true", "enumerable: false"),
            ("writable: true", "writable: false"),
            ("configurable: true", "configurable: false"),
            (
                "if (\"3735928559\" in extensions)",
                "if (\"3735928559\" == extensions)",
            ),
            (
                "encodeConfig.extensions = extensions;",
                "encodeConfig.extensionsValue = extensions;",
            ),
            ("config = encodeConfig;", "config = extensions;"),
        ],
    );
}

#[test]
fn serializer_factory_requires_the_owned_nested_serializer() {
    assert!(
        !&RETURNED_CAPTURED_DECOY.is_empty(),
        "decoy fixture is present"
    );
    refused(
        RETURNED_CAPTURED_DECOY,
        "$THREAD_700_11",
        "createSignalSerializer",
    );
    refused(
        NESTED_CALLBACK_DECOY,
        "$THREAD_700_11",
        "createSignalSerializer",
    );
    refused(
        SHADOWED_DEPTH_CAPTURE,
        "$THREAD_700_11",
        "createSignalSerializer",
    );
    refused(RECEIVER_DECOY, "$THREAD_700_11", "createSignalSerializer");
}

#[test]
fn serializer_factory_requires_captured_config_and_seed_closure() {
    refuses(
        POSITIVE,
        "$THREAD_700_11",
        "createSignalSerializer",
        &[
            ("bytes.push(config.depthBound, seed);", "bytes.push(seed);"),
            ("bytes.push(config.depthBound, seed);", "bytes.push(config.depthBound);"),
            (
                "bytes.push(snapshot.errors.length, snapshot.positionalSignals.length,",
                "bytes.push(reservedSlot, snapshot.positionalSignals.length,",
            ),
            (
                "function $THREAD_700_11(keysetsToOmit, reservedSlot, seedKey){",
                "function $THREAD_700_11(keysetsToOmit, seedKey){",
            ),
        ],
    );
}

#[test]
fn serializer_factory_does_not_rename_readable_bindings() {
    let human = POSITIVE.replace("$THREAD_700_11", "createSignalSerializer");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
}

// Configuration placeholder precedes the seed alias; its value is assigned after the
// extension store.
const CONFIG_PLACEHOLDER_FIRST: &str = r#"function host(){
let $KEY_426, $KEY_193;
function $THREAD_1_43446($BH, $BI, $BJ) {
					let $KEY_4;
					let $KEY_6 = $BJ;
					let $BK = {
						keysetsToOmit: $BH,
						depthBound: 100,
						extensions: void 0
					};
					let $BL = { "0": $KEY_426 };
					let $BM = $KEY_193;
					if ("3735928559" in $BL) {
						$BM = {
							value: $BM,
							enumerable: true,
							configurable: true,
							writable: true
						};
						Object.defineProperty($BL, "3735928559", $BM);
					} else {
						$BL[3735928559] = $BM;
					}
					$BK.extensions = $BL;
					$KEY_4 = $BK;
					function $THREAD_2_85520($BN) {
						let $KEY_14 = [];
						$KEY_14 = $BN.errors.length === 0 ? [] : $KEY_321($BN.errors);
						let $BO = [];
						let $BP, $BQ, $BR, $BS;
						if ($KEY_14.length >>> 0 < 128) {
							$BQ = 0;
							$BR = $KEY_14.length >>> 0;
							$BS = void 0;
						} else {
							let $BT = $KEY_14.length >>> 0 >>> 7;
							let $BU = -1;
							let $BV = $KEY_14.length >>> 0 & 127;
							for (;;) {
								$BS = Number($BU) + 1;
								$BO[$BS] = $BV | 128;
								$BR = $BT & 127;
								if ($BT >>> 7 <= 0) break;
								$BT = $BT >>> 7;
								$BU = $BS;
								$BV = $BR;
							}
							$BQ = $BS + 1;
						}
						$BP = $BO;
						$BP[$BQ] = $BR;
						let i = 0;
						for (; i < $BO.length; i++) {
							$KEY_14.push($BO[i]);
						}
						let j = 0;
						for (; j < $KEY_14.length; j++) {
							$KEY_14.push($KEY_14[j]);
						}
						let j_5 = 0;
						let $BW;
						for (; j_5 < $BN.positionalSignals.length; j_5++) {
							let $BX;
							if ($BN.positionalSignals[j_5] === $KEY_169) {
								$KEY_14.push(0);
								$KEY_14.push(0);
								$BX = $BW;
							} else {
								const $BY = $BN.positionalSignals[j_5][0].length === 0 ? [] : $KEY_321($BN.positionalSignals[j_5][0]);
								let $BZ = [];
								let $CA, $CB, $CC, $CD;
								if ($BY.length >>> 0 < 128) {
									$CB = 0;
									$CC = $BY.length >>> 0;
									$CD = $BW;
								} else {
									let $CE = $BY.length >>> 0 >>> 7;
									let $CF = $BY.length >>> 0 & 127;
									let $CG = -1;
									let $CH;
									for (;;) {
										$BZ[++$CG] = $CF | 128;
										$CH = $CE;
										if ($CH >>> 7 <= 0) break;
										$CE = $CH >>> 7;
										$CF = $CH & 127;
									}
									$CB = $CG + 1;
									$CC = $CH & 127;
									$CD = $CH & 127;
								}
								$CA = $BZ;
								$CA[$CB] = $CC;
								let k = 0;
								for (; k < $BZ.length; k++) {
									$KEY_14.push($BZ[k]);
								}
								let i_2 = 0;
								for (; i_2 < $BY.length; i_2++) {
									$KEY_14.push($BY[i_2]);
								}
								let $CI = [];
								let $CJ, $CK, $CL;
								if ($BN.positionalSignals[j_5][1].length >>> 0 < 128) {
									$CK = 0;
									$CL = $BN.positionalSignals[j_5][1].length >>> 0;
									$BX = $CD;
								} else {
									let $CM = $BN.positionalSignals[j_5][1].length >>> 0 >>> 7;
									let $CN = -1;
									let $CO = $BN.positionalSignals[j_5][1].length >>> 0 & 127;
									for (;;) {
										$CI[++$CN] = $CO | 128;
										$CL = $CM & 127;
										if ($CM >>> 7 <= 0) break;
										$CM = $CM >>> 7;
										$CO = $CL;
									}
									$CK = $CN + 1;
									$BX = $CK;
								}
								$CJ = $CI;
								$CJ[$CK] = $CL;
								let j_2 = 0;
								for (; j_2 < $CI.length; j_2++) {
									$KEY_14.push($CI[j_2]);
								}
								let k_2 = 0;
								for (; k_2 < $BN.positionalSignals[j_5][1].length; k_2++) {
									$KEY_14.push($BN.positionalSignals[j_5][1][k_2]);
								}
								$BS = $BN.positionalSignals[j_5][1];
							}
							$BW = $BX;
						}
						const $CP = $BN.signals.length === 0 ? [] : $KEY_321($BN.signals, $KEY_4);
						let $CQ = [];
						let $CR, $CS, $CT, $CU, $CV;
						if ($CP.length >>> 0 < 128) {
							$CS = 0;
							$CT = $CP.length >>> 0;
							$CU = $CP.length >>> 0;
							$CV = $BW;
						} else {
							let $CW = $CP.length >>> 0 >>> 7;
							let $CX = $CP.length >>> 0 & 127;
							let $CY = -1;
							let $CZ;
							for (;;) {
								$CV = Number($CY) + 1;
								$CQ[$CV] = $CX | 128;
								$CZ = $CW;
								if ($CZ >>> 7 <= 0) break;
								$CW = $CZ >>> 7;
								$CX = $CZ & 127;
								$CY = $CV;
							}
							$CS = $CV + 1;
							$CT = $CZ & 127;
							$CU = $CZ >>> 7;
						}
						$CR = $CQ;
						$CR[$CS] = $CT;
						let i_3 = 0;
						for (; i_3 < $CQ.length; i_3++) {
							$KEY_14.push($CQ[i_3]);
						}
						let j_3 = 0;
						for (; j_3 < $CP.length; j_3++) {
							$KEY_14.push($CP[j_3]);
						}
						let $DA = 0;
						let $DB = $KEY_14;
						while ($DA < $BN.keyedSignals.length) {
							let $DC = [];
							let $DD, $DE, $DF, $DG, $DH, $DI;
							if ($BN.keyedSignals[$DA][0] >>> 0 < 128) {
								$DE = 0;
								$DF = $BN.keyedSignals[$DA][0] >>> 0;
								$DG = $CU;
								$DH = $DB;
								$DI = $CV;
							} else {
								let $DJ = -1;
								let $DK = $BN.keyedSignals[$DA][0] >>> 0 & 127;
								let $DL = $BN.keyedSignals[$DA][0] >>> 0 >>> 7;
								for (;;) {
									$DG = Number($DJ) + 1;
									$DC[$DG] = $DK | 128;
									if ($DL >>> 7 <= 0) break;
									$DJ = $DG;
									$DK = $DL & 127;
									$DL = $DL >>> 7;
								}
								$DE = $DG + 1;
								$DF = $DL & 127;
								$DH = $DL & 127;
								$DI = $DG;
							}
							$DD = $DC;
							$DD[$DE] = $DF;
							let k_3 = 0;
							for (; k_3 < $DC.length; k_3++) {
								$KEY_14.push($DC[k_3]);
							}
							if ($BN.keyedSignals[$DA][1] === $KEY_169) {
								$KEY_14.push(0);
								$KEY_14.push(0);
								$CU = $DG;
								$DB = $DH;
								$CV = $DI;
							} else {
								const $DM = $BN.keyedSignals[$DA][1][0].length === 0 ? [] : $KEY_321($BN.keyedSignals[$DA][1][0]);
								let $DN = [];
								let $DO, $DP, $DQ, $DR;
								if ($DM.length >>> 0 < 128) {
									$DP = 0;
									$DQ = $DM.length >>> 0;
									$DR = $DH;
								} else {
									let $DS = $DM.length >>> 0 & 127;
									let $DT = -1;
									let $DU = $DM.length >>> 0 >>> 7;
									for (;;) {
										$DN[++$DT] = $DS | 128;
										if ($DU >>> 7 <= 0) break;
										$DS = $DU & 127;
										$DU = $DU >>> 7;
									}
									$DP = $DT + 1;
									$DQ = $DU & 127;
									$DR = $DN;
								}
								$DO = $DN;
								$DO[$DP] = $DQ;
								let i_4 = 0;
								for (; i_4 < $DN.length; i_4++) {
									$KEY_14.push($DN[i_4]);
								}
								let j_4 = 0;
								for (; j_4 < $DM.length; j_4++) {
									$KEY_14.push($DM[j_4]);
								}
								$CU = [];
								let $DV, $DW, $DX;
								if ($BN.keyedSignals[$DA][1][1].length >>> 0 < 128) {
									$DW = 0;
									$DX = $BN.keyedSignals[$DA][1][1].length >>> 0;
									$DB = $DR;
								} else {
									let $DY = $BN.keyedSignals[$DA][1][1].length >>> 0 >>> 7;
									let $DZ = -1;
									let $EA = $BN.keyedSignals[$DA][1][1].length >>> 0 & 127;
									for (;;) {
										$CU[++$DZ] = $EA | 128;
										$DX = $DY & 127;
										if ($DY >>> 7 <= 0) break;
										$DY = $DY >>> 7;
										$EA = $DX;
									}
									$DW = $DZ + 1;
									$DB = $CU;
								}
								$DV = $CU;
								$DV[$DW] = $DX;
								let k_4 = 0;
								for (; k_4 < $CU.length; k_4++) {
									$KEY_14.push($CU[k_4]);
								}
								let i_5 = 0;
								for (; i_5 < $BN.keyedSignals[$DA][1][1].length; i_5++) {
									$KEY_14.push($BN.keyedSignals[$DA][1][1][i_5]);
								}
								$CV = $BN.keyedSignals[$DA][1][1];
							}
							++$DA;
						}
						if ($KEY_405 != null && $KEY_6 != null) {
							let $EB = Math.random() * 4294967296 >>> 0;
							let $EC = 0;
							try {
								$EC = performance.now() >>> 0;
							} catch (e) {}
							let $ED = Number(new Date()) >>> 0;
							let $EE = 0;
							try {
								$EE = crypto.getRandomValues(new Uint32Array(1))[0];
							} catch (e_2) {}
							$EB = $EB ^ 1103515245;
							$EC = $EC === $EC ? $EB ^ $EC : $EB;
							$ED = $ED === $ED ? $EC ^ $ED : $EC;
							++$KEY_400;
							$EE = ($ED ^ $EE ^ $KEY_400) >>> 0;
							let $EF = Math.random() * 4294967296 >>> 0;
							let $EG = 0;
							try {
								$EG = performance.now() >>> 0;
							} catch (e_3) {}
							let $EH = Number(new Date()) >>> 0;
							let $EI = 0;
							try {
								$EI = crypto.getRandomValues(new Uint32Array(1))[0];
							} catch (e_4) {}
							$EF = $EF ^ 1103515245;
							$EG = $EG === $EG ? $EF ^ $EG : $EF;
							$EH = $EH === $EH ? $EG ^ $EH : $EG;
							++$KEY_400;
							$EI = ($EH ^ $EI ^ $KEY_400) >>> 0;
							$KEY_14 = $KEY_405($KEY_14, $KEY_6, $EE, $EI);
							$KEY_14.unshift($EE >>> 24, $EE >>> 16 & 255, $EE >>> 8 & 255, $EE & 255, $EI >>> 24, $EI >>> 16 & 255, $EI >>> 8 & 255, $EI & 255);
						}
						return $KEY_14;
					}
					return $THREAD_2_85520;
				}
}
host();"#;

// Seed alias precedes configuration, declared by `let config = encodeConfig` after the
// extension store.
const CONFIG_DECLARATION_AFTER_STORE: &str = r#"function host(){
let $KEY_147, $KEY_296;
function $THREAD_9_22935($BR, $BS, $BT) {
					let $KEY_4 = $BT;
					let $BU = {
						keysetsToOmit: $BR,
						depthBound: 100,
						extensions: void 0
					};
					let $BV = { "0": $KEY_147 };
					let $BW = $KEY_296;
					if ("3735928559" in $BV) {
						$BW = {
							value: $BW,
							enumerable: true,
							configurable: true,
							writable: true
						};
						Object.defineProperty($BV, "3735928559", $BW);
					} else {
						$BV[3735928559] = $BW;
					}
					$BU.extensions = $BV;
					let $KEY_0 = $BU;
					function $THREAD_6_33697($BX) {
						let $BY = [];
						let $KEY_11 = $BX.errors.length === 0 ? [] : $KEY_252($BX.errors);
						let $BZ = [];
						let $CA, $CB, $CC, $CD;
						if ($KEY_11.length >>> 0 < 128) {
							$CB = 0;
							$CC = $KEY_11.length >>> 0;
							$CD = void 0;
						} else {
							let $CE = $KEY_11.length >>> 0 & 127;
							let $CF = $KEY_11.length >>> 0 >>> 7;
							let $CG = -1;
							for (;;) {
								$CD = Number($CG) + 1;
								$BZ[$CD] = $CE | 128;
								if ($CF >>> 7 <= 0) break;
								$CE = $CF & 127;
								$CF = $CF >>> 7;
								$CG = $CD;
							}
							$CB = $CD + 1;
							$CC = $CF & 127;
						}
						$CA = $BZ;
						$CA[$CB] = $CC;
						let i = 0;
						for (; i < $BZ.length; i++) {
							$BY.push($BZ[i]);
						}
						let j = 0;
						for (; j < $KEY_11.length; j++) {
							$BY.push($KEY_11[j]);
						}
						let j_5 = 0;
						let $CH;
						let $CI = $BY;
						for (; j_5 < $BX.positionalSignals.length; j_5++) {
							let $CJ, $CK;
							if ($BX.positionalSignals[j_5] === $KEY_17) {
								$BY.push(0);
								$BY.push(0);
								$CJ = $CH;
								$CK = $CI;
							} else {
								const $CL = $BX.positionalSignals[j_5][0].length === 0 ? [] : $KEY_252($BX.positionalSignals[j_5][0]);
								let $CM = [];
								let $CN, $CO, $CP;
								if ($CL.length >>> 0 < 128) {
									$CO = 0;
									$CP = $CL.length >>> 0;
									$CJ = $CH;
								} else {
									let $CQ = $CL.length >>> 0 >>> 7;
									let $CR = -1;
									let $CS = $CL.length >>> 0 & 127;
									for (;;) {
										$CM[++$CR] = $CS | 128;
										$CP = $CQ & 127;
										if ($CQ >>> 7 <= 0) break;
										$CQ = $CQ >>> 7;
										$CS = $CP;
									}
									$CO = $CR + 1;
									$CJ = $CM;
								}
								$CN = $CM;
								$CN[$CO] = $CP;
								let k = 0;
								for (; k < $CM.length; k++) {
									$BY.push($CM[k]);
								}
								let i_2 = 0;
								for (; i_2 < $CL.length; i_2++) {
									$BY.push($CL[i_2]);
								}
								let $CT = [];
								let $CU, $CV, $CW;
								if ($BX.positionalSignals[j_5][1].length >>> 0 < 128) {
									$CV = 0;
									$CW = $BX.positionalSignals[j_5][1].length >>> 0;
									$CK = $BX.positionalSignals[j_5][1].length >>> 0;
								} else {
									let $CX = -1;
									let $CY = $BX.positionalSignals[j_5][1].length >>> 0 >>> 7;
									let $CZ = $BX.positionalSignals[j_5][1].length >>> 0 & 127;
									for (;;) {
										$CT[++$CX] = $CZ | 128;
										$CW = $CY & 127;
										if ($CY >>> 7 <= 0) break;
										$CY = $CY >>> 7;
										$CZ = $CW;
									}
									$CV = $CX + 1;
									$CK = $CY >>> 7;
								}
								$CU = $CT;
								$CU[$CV] = $CW;
								for (let j_2 = 0; j_2 < $CT.length; j_2++) {
									$BY.push($CT[j_2]);
								}
								let k_2 = 0;
								for (; k_2 < $BX.positionalSignals[j_5][1].length; k_2++) {
									$BY.push($BX.positionalSignals[j_5][1][k_2]);
								}
								$CD = $BX.positionalSignals[j_5][1];
							}
							$CH = $CJ;
							$CI = $CK;
						}
						const $DA = $BX.signals.length === 0 ? [] : $KEY_252($BX.signals, $KEY_0);
						let $DB = [];
						let $DC, $DD, $DE;
						if ($DA.length >>> 0 < 128) {
							$DD = 0;
							$DE = $DA.length >>> 0;
						} else {
							let $DF = $DA.length >>> 0 >>> 7;
							let $DG = -1;
							let $DH = $DA.length >>> 0 & 127;
							for (;;) {
								$DB[++$DG] = $DH | 128;
								$DE = $DF & 127;
								if ($DF >>> 7 <= 0) break;
								$DF = $DF >>> 7;
								$DH = $DE;
							}
							$DD = $DG + 1;
						}
						$DC = $DB;
						$DC[$DD] = $DE;
						let i_3 = 0;
						for (; i_3 < $DB.length; i_3++) {
							$BY.push($DB[i_3]);
						}
						let j_3 = 0;
						for (; j_3 < $DA.length; j_3++) {
							$BY.push($DA[j_3]);
						}
						let $DI = 0;
						let $DJ = $DB;
						let $DK = $DA;
						while ($DI < $BX.keyedSignals.length) {
							let $DL = [];
							let $DM, $DN, $DO, $DP, $DQ;
							if ($BX.keyedSignals[$DI][0] >>> 0 < 128) {
								$DN = 0;
								$DO = $BX.keyedSignals[$DI][0] >>> 0;
								$DP = $DJ;
								$DQ = $DK;
							} else {
								let $DR = $BX.keyedSignals[$DI][0] >>> 0 & 127;
								let $DS = $BX.keyedSignals[$DI][0] >>> 0 >>> 7;
								let $DT = -1;
								for (;;) {
									$DP = Number($DT) + 1;
									$DL[$DP] = $DR | 128;
									if ($DS >>> 7 <= 0) break;
									$DR = $DS & 127;
									$DS = $DS >>> 7;
									$DT = $DP;
								}
								$DN = $DP + 1;
								$DO = $DS & 127;
								$DQ = $DP;
							}
							$DM = $DL;
							$DM[$DN] = $DO;
							for (let k_3 = 0; k_3 < $DL.length; k_3++) {
								$BY.push($DL[k_3]);
							}
							if ($BX.keyedSignals[$DI][1] === $KEY_17) {
								$BY.push(0);
								$BY.push(0);
								$DJ = $DP;
								$DK = $DQ;
							} else {
								const $DU = $BX.keyedSignals[$DI][1][0].length === 0 ? [] : $KEY_252($BX.keyedSignals[$DI][1][0]);
								let $DV = [];
								let $DW, $DX, $DY, $DZ, $EA;
								if ($DU.length >>> 0 < 128) {
									$DX = 0;
									$DY = $DU.length >>> 0;
									$DZ = $DP;
									$EA = $DU.length >>> 0;
								} else {
									let $EB = $DU.length >>> 0 & 127;
									let $EC = -1;
									let $ED = $DU.length >>> 0 >>> 7;
									for (;;) {
										$DV[++$EC] = $EB | 128;
										if ($ED >>> 7 <= 0) break;
										$EB = $ED & 127;
										$ED = $ED >>> 7;
									}
									$DX = $EC + 1;
									$DY = $ED & 127;
									$DZ = $DV;
									$EA = $ED >>> 7;
								}
								$DW = $DV;
								$DW[$DX] = $DY;
								let i_4 = 0;
								for (; i_4 < $DV.length; i_4++) {
									$BY.push($DV[i_4]);
								}
								let j_4 = 0;
								for (; j_4 < $DU.length; j_4++) {
									$BY.push($DU[j_4]);
								}
								let $EE = [];
								let $EF, $EG, $EH;
								if ($BX.keyedSignals[$DI][1][1].length >>> 0 < 128) {
									$EG = 0;
									$EH = $BX.keyedSignals[$DI][1][1].length >>> 0;
									$DJ = $DZ;
									$DK = $EA;
								} else {
									let $EI = $BX.keyedSignals[$DI][1][1].length >>> 0 >>> 7;
									let $EJ = $BX.keyedSignals[$DI][1][1].length >>> 0 & 127;
									let $EK = -1;
									for (;;) {
										$DK = Number($EK) + 1;
										$EE[$DK] = $EJ | 128;
										$EH = $EI & 127;
										if ($EI >>> 7 <= 0) break;
										$EI = $EI >>> 7;
										$EJ = $EH;
										$EK = $DK;
									}
									$EG = $DK + 1;
									$DJ = $EE;
								}
								$EF = $EE;
								$EF[$EG] = $EH;
								let k_4 = 0;
								for (; k_4 < $EE.length; k_4++) {
									$BY.push($EE[k_4]);
								}
								let i_5 = 0;
								for (; i_5 < $BX.keyedSignals[$DI][1][1].length; i_5++) {
									$BY.push($BX.keyedSignals[$DI][1][1][i_5]);
								}
								$CH = $BX.keyedSignals[$DI][1][1];
							}
							++$DI;
						}
						let $EL;
						if ($KEY_31 != null && $KEY_4 != null) {
							let $EM = Math.random() * 4294967296 >>> 0;
							let $EN = 0;
							try {
								$EN = performance.now() >>> 0;
							} catch (e) {}
							let $EO = Number(new Date()) >>> 0;
							let $EP = 0;
							try {
								$EP = crypto.getRandomValues(new Uint32Array(1))[0];
							} catch (e_2) {}
							$EM = $EM ^ 1103515245;
							$EN = $EN === $EN ? $EM ^ $EN : $EM;
							$EO = $EO === $EO ? $EN ^ $EO : $EN;
							++$KEY_24;
							$EP = ($EO ^ $EP ^ $KEY_24) >>> 0;
							let $EQ = Math.random() * 4294967296 >>> 0;
							let $ER = 0;
							try {
								$ER = performance.now() >>> 0;
							} catch (e_3) {}
							let $ES = Number(new Date()) >>> 0;
							let $ET = 0;
							try {
								$ET = crypto.getRandomValues(new Uint32Array(1))[0];
							} catch (e_4) {}
							$EQ = $EQ ^ 1103515245;
							$ER = $ER === $ER ? $EQ ^ $ER : $EQ;
							$ES = $ES === $ES ? $ER ^ $ES : $ER;
							++$KEY_24;
							$ET = ($ES ^ $ET ^ $KEY_24) >>> 0;
							$EL = $KEY_31($BY, $KEY_4, $EP, $ET);
							$EL.unshift($EP >>> 24, $EP >>> 16 & 255, $EP >>> 8 & 255, $EP & 255, $ET >>> 24, $ET >>> 16 & 255, $ET >>> 8 & 255, $ET & 255);
						} else {
							$EL = $BY;
						}
						return $EL;
					}
					return $THREAD_6_33697;
				}
}
host();"#;

// Seed alias comes first; configuration placeholder precedes the encode object and is
// assigned after the extension store.
const CONFIG_ASSIGNMENT_AFTER_STORE: &str = r#"function host(){
let $KEY_61, $KEY_276;
function $THREAD_3_20972($BX, $BY, $BZ) {
			let $KEY_2 = $BZ;
			let $KEY_7;
			let $CA = {
				keysetsToOmit: $BX,
				depthBound: 100,
				extensions: void 0
			};
			let $CB = { "0": $KEY_61 };
			let $CC = $KEY_276;
			if ("3735928559" in $CB) {
				$CC = {
					value: $CC,
					enumerable: true,
					configurable: true,
					writable: true
				};
				Object.defineProperty($CB, "3735928559", $CC);
			} else {
				$CB[3735928559] = $CC;
			}
			$CA.extensions = $CB;
			$KEY_7 = $CA;
			function $THREAD_2_134312($CD) {
				let $CE = [];
				const $CF = $CD.errors.length === 0 ? [] : $KEY_242($CD.errors);
				let $CG = [];
				let $CH, $CI;
				if ($CF.length >>> 0 < 128) {
					$CH = 0;
					$CI = $CF.length >>> 0;
				} else {
					let $CJ = $CF.length >>> 0 >>> 7;
					$CI = $CF.length >>> 0 & 127;
					let i = -1;
					let $CK;
					for (;; i = $CK) {
						$CK = Number(i) + 1;
						$CG[$CK] = $CI | 128;
						$CI = $CJ & 127;
						if ($CJ >>> 7 <= 0) break;
						$CJ = $CJ >>> 7;
					}
					$CH = $CK + 1;
				}
				$CG[$CH] = $CI;
				let j = 0;
				let $CL;
				for (;; j = Number($CL) + 1) {
					$CL = j;
					if (!($CL < $CG.length)) break;
					$CE.push($CG[$CL]);
				}
				let k = 0;
				for (; k < $CF.length; k++) {
					$CE.push($CF[k]);
				}
				let k_5 = 0;
				let $CM;
				let $CN = $CE;
				for (; k_5 < $CD.positionalSignals.length; k_5++) {
					let $CO, $CP;
					if ($CD.positionalSignals[k_5] === $KEY_140) {
						$CE.push(0);
						$CE.push(0);
						$CO = $CM;
						$CP = $CN;
						$CD.positionalSignals[k_5];
					} else {
						const $CQ = $CD.positionalSignals[k_5][0].length === 0 ? [] : $KEY_242($CD.positionalSignals[k_5][0]);
						let $CR = [];
						let $CS, $CT, $CU, $CV;
						if ($CQ.length >>> 0 < 128) {
							$CT = 0;
							$CU = $CQ.length >>> 0;
							$CO = $CM;
							$CV = $CN;
						} else {
							let $CW = $CQ.length >>> 0 >>> 7;
							let $CX = -1;
							let $CY = $CQ.length >>> 0 & 127;
							for (;;) {
								$CV = Number($CX) + 1;
								$CR[$CV] = $CY | 128;
								$CU = $CW & 127;
								if ($CW >>> 7 <= 0) break;
								$CW = $CW >>> 7;
								$CX = $CV;
								$CY = $CU;
							}
							$CT = $CV + 1;
							$CO = $CR;
						}
						$CS = $CR;
						$CS[$CT] = $CU;
						for (let i_2 = 0; i_2 < $CR.length; i_2++) {
							$CE.push($CR[i_2]);
						}
						let j_2 = 0;
						for (; j_2 < $CQ.length; j_2++) {
							$CE.push($CQ[j_2]);
						}
						let $CZ = [];
						let $DA, $DB, $DC;
						if ($CD.positionalSignals[k_5][1].length >>> 0 < 128) {
							$DB = 0;
							$DC = $CD.positionalSignals[k_5][1].length >>> 0;
							$CP = $CV;
							$CD.positionalSignals[k_5];
						} else {
							let $DD = $CD.positionalSignals[k_5][1].length >>> 0 >>> 7;
							let $DE = $CD.positionalSignals[k_5][1].length >>> 0 & 127;
							let $DF = -1;
							for (;;) {
								$CZ[++$DF] = $DE | 128;
								$CP = $DD & 127;
								if ($DD >>> 7 <= 0) break;
								$DD = $DD >>> 7;
								$DE = $CP;
							}
							$DB = $DF + 1;
							$DC = $CP;
						}
						$DA = $CZ;
						$DA[$DB] = $DC;
						let k_2 = 0;
						for (; k_2 < $CZ.length; k_2++) {
							$CE.push($CZ[k_2]);
						}
						let i_3 = 0;
						for (; i_3 < $CD.positionalSignals[k_5][1].length; i_3++) {
							$CE.push($CD.positionalSignals[k_5][1][i_3]);
						}
						$CL = $CD.positionalSignals[k_5][1];
					}
					$CM = $CO;
					$CN = $CP;
				}
				const $DG = $CD.signals.length === 0 ? [] : $KEY_242($CD.signals, $KEY_7);
				let $DH = [];
				let $DI, $DJ, $DK;
				if ($DG.length >>> 0 < 128) {
					$DJ = 0;
					$DK = $DG.length >>> 0;
				} else {
					let $DL = $DG.length >>> 0 >>> 7;
					let $DM = -1;
					let $DN = $DG.length >>> 0 & 127;
					for (;;) {
						$DH[++$DM] = $DN | 128;
						$DK = $DL & 127;
						if ($DL >>> 7 <= 0) break;
						$DL = $DL >>> 7;
						$DN = $DK;
					}
					$DJ = $DM + 1;
				}
				$DI = $DH;
				$DI[$DJ] = $DK;
				let j_3 = 0;
				for (; j_3 < $DH.length; j_3++) {
					$CE.push($DH[j_3]);
				}
				for (let k_3 = 0; k_3 < $DG.length; k_3++) {
					$CE.push($DG[k_3]);
				}
				let $DO = 0;
				let $DP = $DG;
				while ($DO < $CD.keyedSignals.length) {
					let $DQ = [];
					let $DR, $DS, $DT;
					if ($CD.keyedSignals[$DO][0] >>> 0 < 128) {
						$DS = 0;
						$DT = $CD.keyedSignals[$DO][0] >>> 0;
					} else {
						let $DU = $CD.keyedSignals[$DO][0] >>> 0 & 127;
						let $DV = $CD.keyedSignals[$DO][0] >>> 0 >>> 7;
						let $DW = -1;
						for (;;) {
							$DQ[++$DW] = $DU | 128;
							if ($DV >>> 7 <= 0) break;
							$DU = $DV & 127;
							$DV = $DV >>> 7;
						}
						$DS = $DW + 1;
						$DT = $DV & 127;
					}
					$DR = $DQ;
					$DR[$DS] = $DT;
					let i_4 = 0;
					for (; i_4 < $DQ.length; i_4++) {
						$CE.push($DQ[i_4]);
					}
					let $DX;
					if ($CD.keyedSignals[$DO][1] === $KEY_140) {
						$CE.push(0);
						$CE.push(0);
						$DX = $DP;
					} else {
						const $DY = $CD.keyedSignals[$DO][1][0].length === 0 ? [] : $KEY_242($CD.keyedSignals[$DO][1][0]);
						let $DZ = [];
						let $EA, $EB, $EC, $ED;
						if ($DY.length >>> 0 < 128) {
							$EB = 0;
							$EC = $DY.length >>> 0;
							$ED = $DP;
						} else {
							let $EE = -1;
							let $EF = $DY.length >>> 0 >>> 7;
							let $EG = $DY.length >>> 0 & 127;
							for (;;) {
								$DZ[++$EE] = $EG | 128;
								$ED = $EF & 127;
								if ($EF >>> 7 <= 0) break;
								$EF = $EF >>> 7;
								$EG = $ED;
							}
							$EB = $EE + 1;
							$EC = $ED;
						}
						$EA = $DZ;
						$EA[$EB] = $EC;
						let j_4 = 0;
						for (; j_4 < $DZ.length; j_4++) {
							$CE.push($DZ[j_4]);
						}
						let k_4 = 0;
						for (; k_4 < $DY.length; k_4++) {
							$CE.push($DY[k_4]);
						}
						let $EH = [];
						let $EI, $EJ, $EK;
						if ($CD.keyedSignals[$DO][1][1].length >>> 0 < 128) {
							$EJ = 0;
							$EK = $CD.keyedSignals[$DO][1][1].length >>> 0;
							$DX = $ED;
						} else {
							let $EL = $CD.keyedSignals[$DO][1][1].length >>> 0 >>> 7;
							let $EM = $CD.keyedSignals[$DO][1][1].length >>> 0 & 127;
							let $EN = -1;
							let $EO;
							for (;;) {
								$DX = Number($EN) + 1;
								$EH[$DX] = $EM | 128;
								$EO = $EL;
								if ($EO >>> 7 <= 0) break;
								$EL = $EO >>> 7;
								$EM = $EO & 127;
								$EN = $DX;
							}
							$EJ = $DX + 1;
							$EK = $EO & 127;
						}
						$EI = $EH;
						$EI[$EJ] = $EK;
						let i_5 = 0;
						for (; i_5 < $EH.length; i_5++) {
							$CE.push($EH[i_5]);
						}
						let j_5 = 0;
						for (; j_5 < $CD.keyedSignals[$DO][1][1].length; j_5++) {
							$CE.push($CD.keyedSignals[$DO][1][1][j_5]);
						}
					}
					++$DO;
					$DP = $DX;
				}
				let $EP;
				if ($KEY_312 != null && $KEY_2 != null) {
					let $EQ = Math.random() * 4294967296 >>> 0;
					let $ER = 0;
					try {
						$ER = performance.now() >>> 0;
					} catch (e) {}
					let $ES = Number(new Date()) >>> 0;
					let $ET = 0;
					try {
						$ET = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_2) {}
					$EQ = $EQ ^ 1103515245;
					$ER = $ER === $ER ? $EQ ^ $ER : $EQ;
					$ES = $ES === $ES ? $ER ^ $ES : $ER;
					++$KEY_67;
					$ET = ($ES ^ $ET ^ $KEY_67) >>> 0;
					let $EU = Math.random() * 4294967296 >>> 0;
					let $EV = 0;
					try {
						$EV = performance.now() >>> 0;
					} catch (e_3) {}
					let $EW = Number(new Date()) >>> 0;
					let $EX = 0;
					try {
						$EX = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_4) {}
					$EU = $EU ^ 1103515245;
					$EV = $EV === $EV ? $EU ^ $EV : $EU;
					$EW = $EW === $EW ? $EV ^ $EW : $EV;
					++$KEY_67;
					$EX = ($EW ^ $EX ^ $KEY_67) >>> 0;
					$EP = $KEY_312($CE, $KEY_2, $ET, $EX);
					$EP.unshift($ET >>> 24, $ET >>> 16 & 255, $ET >>> 8 & 255, $ET & 255, $EX >>> 24, $EX >>> 16 & 255, $EX >>> 8 & 255, $EX & 255);
				} else {
					$EP = $CE;
				}
				return $EP;
			}
			return $THREAD_2_134312;
		}
}
host();"#;


#[test]
fn serializer_factory_is_named_for_cached_vendor_variants() {
    for (source, target) in [
        (CONFIG_PLACEHOLDER_FIRST, "$THREAD_1_43446"),
        (CONFIG_DECLARATION_AFTER_STORE, "$THREAD_9_22935"),
        (CONFIG_ASSIGNMENT_AFTER_STORE, "$THREAD_3_20972"),
    ] {
        assert!(
            named(source, target, "createSignalSerializer"),
            "did not name cached vendor serializer factory {target}"
        );
    }
}

#[test]
fn serializer_factory_refuses_cached_variant_operation_changes() {
    refuses(
        CONFIG_PLACEHOLDER_FIRST,
        "$THREAD_1_43446",
        "createSignalSerializer",
        &[
            ("keysetsToOmit: $BH", "keysetsToOmit: $BI"),
            ("depthBound: 100", "depthBound: 64"),
            ("extensions: void 0", "extensions: null"),
            ("{ \"0\": $KEY_426 }", "{ \"2\": $KEY_426 }"),
            ("let $BM = $KEY_193;", "let $BM = $KEY_426;"),
            (
                "Object.defineProperty($BL, \"3735928559\", $BM);",
                "Object.defineProperty($BL, \"3735928558\", $BM);",
            ),
            ("$BL[3735928559] = $BM;", "$BL[3735928558] = $BM;"),
            (
                "Object.defineProperty($BL, \"3735928559\", $BM);",
                "Reflect.defineProperty($BL, \"3735928559\", $BM);",
            ),
            ("$BK.extensions = $BL;", "$BK.extension = $BL;"),
            ("$KEY_4 = $BK;", "$KEY_4 = $BL;"),
        ],
    );
}
