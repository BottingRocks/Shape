use shape::naming::classes::encoder::identify_value_encoder_classes;

// Verbatim class-encoder factory with a small, inert helper carrier. The recovered
// JavaScript is never executed.
const ORIGINAL: &str = r###"function main(){
let $KEY_236={STRLUT:0,STRREF:1,STR5_BASE:2,CSTRING:3,STR8:4,STR_:5,EXTENSION:6,FALSE:7,TRUE:8,NULL:9,UNDEFINED:10,NINT4_BASE:11,NINT8:12,NINT16:13,NINT32:14,NINT64:15,FLOAT32:16,TIMESTAMP:17,BINARY_:18,BARRAY4_BASE:19,BARRAY8:20,BARRAY_:21,ARRAY5_BASE:22,ARRAY8:23,ARRAY_:24,BMAP_:25,MAP_:26,UINT14_BASE:27,UINT16:28,UINT24:29,UINT32:30,UINT64:31,DOUBLE64:32};
function $KEY_329(a,b){if(a.length===b.length){return a.every((v,i)=>b[i]===v);}return false;}
function registryFactory(){
 function R(){if(!(this instanceof R)){throw new TypeError();}this.extensions=Object.create(null);}
 function extend(k,d,s,u){this.extensions[k]={detector:d,serialiser:s,deserialiser:u};}
 R.prototype.extend=extend;
 return R;
}
let $BM=registryFactory();
function $THREAD_4_80734($BR) {
			let $KEY_3;
			class $THREAD_3_16462 extends $BR {
				constructor($BS, $BT) {
					if ($BS != null && $BT == null || $BS == null && $BT != null) {
						throw new TypeError("depthBound must be specified if and only if depthBoundExtensionPoint is");
					}
					super();
					this.keysets = [];
					this.stringHist = {};
					this.stringPlaceholders = true;
					this.remainingDepth = $BS == null ? null : $BS;
					this.depthBoundExtensionPoint = $BT ?? null;
				}
				encode() {
					let $KEY_1, $KEY_4;
					let $KEY_2 = arguments;
					let $BS;
					if ($KEY_2.length <= 1) {
						$BS = true;
					} else {
						$KEY_2 = arguments;
						$BS = $KEY_2[1] === undefined;
					}
					if ($BS) {
						$KEY_2 = {};
					} else {
						$KEY_2 = arguments[1];
					}
					$KEY_1 = [];
					if ($KEY_2.keysetsToOmit != null) {
						Array.prototype.push.apply(this.keysets, $KEY_2.keysetsToOmit);
					}
					let $BT = this.encodeValue(arguments[0], []);
					if ($KEY_2.keysetsToOmit != null) {
						this.keysets = this.keysets.slice($KEY_2.keysetsToOmit.length);
					}
					let $BU = this.encodeValue(this.keysets, []);
					$KEY_4 = $KEY_256(this.stringHist);
					this.stringPlaceholders = false;
					if ($KEY_4.length > 0 || this.keysets.length > 0) {
						$KEY_1.push($KEY_236.STRLUT);
						$KEY_1.push($KEY_4.length);
						this.pushArrayElements($KEY_4, $KEY_1);
						$BU = [$BU, $BU.concat];
						$BT = $BU[1].call($BU[0], $BT);
					}
					$BT.forEach($BV => {
						if (typeof $BV === "string") {
							let $BW = $KEY_1;
							let $BX;
							if ($KEY_4) {
								let $BY = [$KEY_4, $KEY_4.indexOf];
								$BX = $BY[1].call($BY[0], $BV);
							} else {
								$BX = -1;
							}
							if ($BX !== -1) {
								$BW.push($KEY_236.STRREF, $BX);
							} else {
								let $BZ = [];
								let $CA = unescape(encodeURIComponent($BV));
								let j = 0;
								let $CB = false;
								for (; j < $CA.length; j++) {
									let $CC = [$CA, $CA.charCodeAt];
									$BZ.push($CC[1].call($CC[0], j));
									if ($BZ[j] === 0) {
										$CB = true;
									}
								}
								let $CD = $BZ.length;
								if ($CD < 32) {
									$BW.push($KEY_236.STR5_BASE | $CD);
								} else if (!$CB) {
									$BW.push($KEY_236.CSTRING);
									$BZ.push(0);
								} else if ($CD <= 255) {
									$BW.push($KEY_236.STR8, $CD);
								} else {
									$BW.push($KEY_236.STR_);
									$KEY_436($CD, $BW);
								}
								if ($BZ.length > 65535) {
									for (let i = 0; i < $BZ.length; i = i + 65535) {
										Array.prototype.push.apply($BW, $BZ.slice(i, i + 65535));
									}
								} else {
									Array.prototype.push.apply($BW, $BZ);
								}
							}
						} else {
							$KEY_1.push($BV);
						}
					});
					return $KEY_1;
				}
				findKeysetIndex($BS) {
					let $KEY_4 = $BS;
					let $BT = this.keysets;
					function $THREAD_9_118913($BW) {
						return $KEY_329($BW, $KEY_4);
					}
					let $BU = 0;
					let $BV;
					for (;;) {
						$BV = void 0;
						if ($BU >= $BT.length) {
							$BV = -1;
							break;
						}
						if (!$THREAD_9_118913($BT[$BU])) {
							++$BU;
							continue;
						}
						$BV = $BU;
						break;
					}
					if ($BV < 0) {
						return this.keysets.push($KEY_4) - 1;
					} else {
						return $BV;
					}
				}
				pushArrayElements($BS, $BT) {
					let $KEY_1;
					let $KEY_3 = $BT;
					$KEY_1 = this;
					Array.prototype.forEach.call($BS, $BU => {
						$KEY_1.encodeValue($BU, $KEY_3);
					});
				}
				encodeValue($BS, $BT) {
					let $KEY_8 = $BT;
					let $KEY_9 = $BS;
					let $KEY_3 = this;
					let $BU = Object.keys(this.extensions);
					function $THREAD_4_150675($CQ) {
						return $KEY_3.extensions[$CQ].detector($KEY_9);
					}
					let $BV = 0;
					let $BW;
					for (;;) {
						$BW = void 0;
						if ($BV >= $BU.length) {
							$BW = null;
							break;
						}
						$BW = $BU[$BV];
						if (!$THREAD_4_150675($BW)) {
							++$BV;
							continue;
						}
						break;
					}
					if ($BW != null) {
						$KEY_8.push($KEY_236.EXTENSION);
						$KEY_436(Number($BW), $KEY_8);
						this.encodeValue(this.extensions[Number($BW)].serialiser($KEY_9), $KEY_8);
					} else if ($KEY_9 === false) {
						$KEY_8.push($KEY_236.FALSE);
					} else if ($KEY_9 === true) {
						$KEY_8.push($KEY_236.TRUE);
					} else if ($KEY_9 === null) {
						$KEY_8.push($KEY_236.NULL);
					} else if (typeof $KEY_9 === "undefined") {
						$KEY_8.push($KEY_236.UNDEFINED);
					} else if (typeof $KEY_9 === "number") {
						if (isFinite($KEY_9)) {
							let $BX = Math.abs($KEY_9);
							if (Math.floor($BX) === $BX && $BX < 0xfffffffffffff800 && ($BX !== 0 || 1 / $KEY_9 > 0)) {
								let $BY = $KEY_9;
								if ($BY === 0 && 1 / $BY === -Infinity) {
									$KEY_8.push($KEY_236.NINT4_BASE);
								} else if ($BY >= 0) {
									$KEY_436($BY, $KEY_8);
								} else if (-$BY <= 15) {
									$KEY_8.push($KEY_236.NINT4_BASE | -$BY);
								} else if (-$BY <= 255) {
									$KEY_8.push($KEY_236.NINT8, -$BY);
								} else if (-$BY <= 65535) {
									$KEY_8.push($KEY_236.NINT16, -$BY >> 8, -$BY & 255);
								} else if (-$BY <= 4294967295) {
									$KEY_8.push($KEY_236.NINT32);
									$KEY_8.push(-$BY >>> 24, -$BY >> 16 & 255, -$BY >> 8 & 255, -$BY & 255);
								} else {
									$KEY_8.push($KEY_236.NINT64);
									$KEY_8.push((-$BY / 4294967296 & 4294967295) >>> 24, (-$BY / 4294967296 & 4294967295) >> 16 & 255, (-$BY / 4294967296 & 4294967295) >> 8 & 255, -$BY / 4294967296 & 4294967295 & 255);
									$KEY_8.push((-$BY & 4294967295) >>> 24, (-$BY & 4294967295) >> 16 & 255, (-$BY & 4294967295) >> 8 & 255, -$BY & 4294967295 & 255);
								}
							} else {
								$KEY_187($KEY_9, $KEY_8);
							}
						} else if ($KEY_9 === 1 / 0) {
							$KEY_8.push($KEY_236.FLOAT32, 127, 128, 0, 0);
						} else if ($KEY_9 === -1 / 0) {
							$KEY_8.push($KEY_236.FLOAT32, 255, 128, 0, 0);
						} else if ($KEY_9 !== $KEY_9) {
							$KEY_8.push($KEY_236.FLOAT32, 127, 192, 0, 0);
						}
					} else if (typeof $KEY_9 === "string") {
						if (this.stringPlaceholders) {
							this.stringHist[$KEY_9] = (this.stringHist[$KEY_9] || 0) + 1;
							$KEY_8.push($KEY_9);
						} else {
							let $CA = $KEY_8;
							let $CB = [];
							let $CC = unescape(encodeURIComponent($KEY_9));
							let i = 0;
							let $CD = false;
							for (; i < $CC.length; i++) {
								let $CE = [$CC, $CC.charCodeAt];
								$CB.push($CE[1].call($CE[0], i));
								if ($CB[i] === 0) {
									$CD = true;
								}
							}
							let $CF = $CB.length;
							if ($CF < 32) {
								$CA.push($KEY_236.STR5_BASE | $CF);
							} else if (!$CD) {
								$CA.push($KEY_236.CSTRING);
								$CB.push(0);
							} else if ($CF <= 255) {
								$CA.push($KEY_236.STR8, $CF);
							} else {
								$CA.push($KEY_236.STR_);
								$KEY_436($CF, $CA);
							}
							if ($CB.length > 65535) {
								let j = 0;
								let $CG;
								for (;; j = $CG + 65535) {
									$CG = j;
									if (!(j < $CB.length)) break;
									Array.prototype.push.apply($CA, $CB.slice($CG, $CG + 65535));
								}
							} else {
								Array.prototype.push.apply($CA, $CB);
							}
						}
					} else if (Object.prototype.toString.call($KEY_9) === "[object Date]") {
						let $CH = $KEY_8;
						let $CI = Date.prototype.getTime.call($KEY_9);
						let $CJ = $CI / 4294967296;
						$CJ = $CI < 0 ? Number($CJ) - 1 : $CJ;
						$CH.push($KEY_236.TIMESTAMP, $CJ >>> 8 & 255, $CJ & 255);
						$CH.push($CI >>> 0 >>> 24, $CI >>> 0 >> 16 & 255, $CI >>> 0 >> 8 & 255, $CI >>> 0 & 255);
					} else if (typeof ArrayBuffer !== "undefined" && $KEY_9 instanceof ArrayBuffer) {
						$KEY_8.push($KEY_236.BINARY_);
						this.encodeValue($KEY_9.byteLength, $KEY_8);
						this.pushArrayElements(new Uint8Array($KEY_9), $KEY_8);
					} else {
						if (this.remainingDepth !== null) {
							if (this.remainingDepth <= 0) {
								$KEY_8.push($KEY_236.EXTENSION);
								if (this.depthBoundExtensionPoint == null) {
									throw new Error("if remainingDepth != null, depthBoundExtensionPoint must not be either");
								} else {
									$KEY_436(this.depthBoundExtensionPoint, $KEY_8);
									$KEY_8.push($KEY_236.NULL);
									return $KEY_8;
								}
							} else {
								this.remainingDepth = Number(this.remainingDepth) - 1;
							}
						}
						if (Array.isArray($KEY_9)) {
							let $CK = $KEY_9.length;
							if ($KEY_9.every($CQ => typeof $CQ === "boolean") && $CK > 0) {
								if ($CK <= 15) {
									$KEY_8.push($KEY_236.BARRAY4_BASE | $CK);
								} else if ($CK <= 255) {
									$KEY_8.push($KEY_236.BARRAY8, $CK);
								} else {
									$KEY_8.push($KEY_236.BARRAY_);
									$KEY_436($CK, $KEY_8);
								}
								for (let k = 0; k < $CK; k = k + 8) {
									$KEY_8.push($KEY_9[k] << 7 | $KEY_9[k + 1] << 6 | $KEY_9[k + 2] << 5 | $KEY_9[k + 3] << 4 | $KEY_9[k + 4] << 3 | $KEY_9[k + 5] << 2 | $KEY_9[k + 6] << 1 | $KEY_9[k + 7]);
								}
							} else {
								if ($CK <= 31) {
									$KEY_8.push($KEY_236.ARRAY5_BASE | $CK);
								} else if ($CK <= 255) {
									$KEY_8.push($KEY_236.ARRAY8, $CK);
								} else {
									$KEY_8.push($KEY_236.ARRAY_);
									$KEY_436($CK, $KEY_8);
								}
								this.pushArrayElements($KEY_9, $KEY_8);
							}
						} else {
							let $CL = Object.keys($KEY_9).sort();
							let $CM = $CL.length;
							let $CN = this.findKeysetIndex($CL);
							if ($CL.every($CQ => typeof $KEY_9[$CQ] === "boolean")) {
								$KEY_8.push($KEY_236.BMAP_);
								$KEY_436($CN, $KEY_8);
								$BU = [0, 0, 0, 0, 0, 0, 0, 0];
								let $CO = 0;
								let $CP = 6;
								while ($CO < $CM) {
									let i_2 = 0;
									for (; i_2 < 8; i_2++) {
										$CP = i_2;
										$BU[$CP] = $CO + i_2 < $CM && $KEY_9[$CL[$CO + i_2]];
									}
									$KEY_8.push($BU[0] << 7 | $BU[1] << 6 | $BU[2] << 5 | $BU[3] << 4 | $BU[4] << 3 | $BU[5] << 2 | $BU[6] << 1 | $BU[7]);
									$CO += 8;
								}
							} else {
								$KEY_8.push($KEY_236.MAP_);
								$KEY_436($CN, $KEY_8);
								$CL.forEach($CQ => $KEY_3.encodeValue($KEY_9[$CQ], $KEY_8));
							}
						}
						if (this.remainingDepth !== null) {
							this.remainingDepth = Number(this.remainingDepth) + 1;
						}
					}
					return $KEY_8;
				}
				static encode() {
					let $KEY_4, $KEY_7;
					let $KEY_1 = arguments;
					let $BS;
					if ($KEY_1.length <= 1) {
						$BS = true;
					} else {
						$KEY_1 = arguments;
						$BS = $KEY_1[1] === undefined;
					}
					if ($BS) {
						$KEY_7 = {};
					} else {
						$KEY_1 = arguments;
						$KEY_7 = $KEY_1[1];
					}
					let $KEY_0 = null;
					if ($KEY_7.depthBound != null) {
						let $BT = $KEY_7.depthBound;
						if ($BT < 0) {
							$BT = $BT < 0;
						} else {
							$BT = Math.floor($BT) !== $BT;
						}
						if ($BT) {
							throw new RangeError("depthBound, if specified, must be a non-negative integer");
						}
						if ($KEY_7.extensions != null) {
							Object.keys($KEY_7.extensions).forEach($BV => {
								let $KEY_2 = Number($BV);
								if ($KEY_7.extensions[$KEY_2] === $KEY_132) {
									$KEY_0 = $KEY_2;
								}
							});
						}
						if ($KEY_0 == null) {
							throw new Error("if depthBound is used, its corresponding extension must be provided");
						}
					} else {
						let $BU;
						if ($KEY_7.extensions != null) {
							$BU = Object.keys($KEY_7.extensions).some($BV => $KEY_7.extensions[$BV] === $KEY_132);
						} else {
							$BU = false;
						}
						if ($BU) {
							throw new Error("if the depthBound extension is used, a depth bound must be specified");
						}
					}
					$KEY_4 = new $KEY_3($KEY_7.depthBound, $KEY_0);
					if ($KEY_7.extensions != null) {
						Object.keys($KEY_7.extensions).forEach($BV => {
							$KEY_4.extend($BV, $KEY_7.extensions[$BV].detector, $KEY_7.extensions[$BV].serialiser, $KEY_7.extensions[$BV].deserialiser);
						});
					}
					return $KEY_4.encode(arguments[0], $KEY_7);
				}
			}
			$KEY_3 = $THREAD_3_16462;
			return $KEY_3;
		}
let encoder=$THREAD_4_80734($BM).encode; return encoder;
}main();"###;

fn count(source: &str) -> usize {
    identify_value_encoder_classes(source).unwrap().len()
}

#[test]
fn original_shared_shape_and_scoped_renames() {
    let proofs = identify_value_encoder_classes(ORIGINAL).unwrap();
    assert_eq!(proofs.len(), 1);
    assert_eq!(proofs[0].canonical_name, "ValueEncoder");
    assert!(
        ORIGINAL[proofs[0].class_span.start as usize..proofs[0].class_span.end as usize]
            .starts_with("class $THREAD_3_16462")
    );
    let renamed = ORIGINAL
        .replace("$THREAD_3_16462", "$THREAD_8_901")
        .replace("$THREAD_4_80734", "makeCodec")
        .replace("$BR", "Base")
        .replace("$BS", "item")
        .replace("$BT", "output")
        .replace("$KEY_3", "identity");
    assert_eq!(count(&renamed), 1);
    // Human-readable classes remain valid identity proofs; the route alone
    // decides whether their binding is eligible for renaming.
    assert_eq!(
        count(&ORIGINAL.replace("$THREAD_3_16462", "ReadableEncoder")),
        1
    );
}

#[test]
fn ordered_return_alias_chains_and_direct_return() {
    let chain = ORIGINAL.replace(
        "return $KEY_3;",
        "const alias1=$KEY_3; const alias2=alias1; return alias2;",
    );
    assert_eq!(count(&chain), 1);
    assert_eq!(
        count(&ORIGINAL.replace("return $KEY_3;", "return $THREAD_3_16462;")),
        1
    );
}

#[test]
fn refuses_other_return_ambiguous_return_and_mutated_identity_or_base() {
    for (from, to) in [
        ("return $KEY_3;", "class Other {} return Other;"),
        ("return $KEY_3;", "if (host) return null; return $KEY_3;"),
        ("return $KEY_3;", "$KEY_3=Other; return $KEY_3;"),
        (
            "return $KEY_3;",
            "const alias=$KEY_3; alias=Other; return alias;",
        ),
        ("extends $BR", "extends Other"),
        ("let $KEY_3;", "$BR=Other; let $KEY_3;"),
    ] {
        assert!(ORIGINAL.contains(from), "missing mutation anchor {from}");
        assert_eq!(count(&ORIGINAL.replace(from, to)), 0, "accepted {to}");
    }
}

#[test]
fn refuses_donor_functions_classes_and_wrong_receivers() {
    for (from, to) in [
        ("this.keysets = [];", "foreign.keysets = [];"),
        ("this.stringHist = {};", "(()=>{this.stringHist={};})();"),
        (
            "this.stringHist = {};",
            "function donor(){this.stringHist={};}",
        ),
        (
            "this.stringHist = {};",
            "class Donor {constructor(){this.stringHist={};}}",
        ),
        ("$KEY_1 = this;", "$KEY_1 = foreign;"),
        (
            "new $KEY_3($KEY_7.depthBound, $KEY_0)",
            "new Other($KEY_7.depthBound, $KEY_0)",
        ),
        (
            "return $KEY_4.encode(arguments[0], $KEY_7);",
            "return Other.encode(arguments[0], $KEY_7);",
        ),
        ("super();", "foreign();"),
        (
            "this.stringPlaceholders = true;",
            "this.stringPlaceholders = false;",
        ),
        (
            "this.remainingDepth = $BS == null ? null : $BS;",
            "this.remainingDepth = null;",
        ),
        (
            "this.depthBoundExtensionPoint = $BT ?? null;",
            "this.depthBoundExtensionPoint = null;",
        ),
    ] {
        assert!(ORIGINAL.contains(from), "missing mutation anchor {from}");
        assert_eq!(count(&ORIGINAL.replace(from, to)), 0, "accepted {to}");
    }
}

#[test]
fn refuses_duplicate_methods_fields_and_nonfunction_overwrites() {
    for extra in [
        "encodeValue() {}",
        "encodeValue=0;",
        "get encodeValue(){return 0;}",
        "static {this.encode=0;}",
    ] {
        let changed = ORIGINAL.to_string();
        // Insert in the class immediately before the final static encode.
        let changed = changed.replace("static encode() {", &format!("{extra} static encode() {{"));
        assert_eq!(count(&changed), 0, "accepted {extra}");
    }
}

// Alternate class-encoder factory. Captured tag and comparator bindings use the same static
// harness.
const NONUNIFORM: &str = r###"function main(){
let $KEY_236; $KEY_236={STRLUT:0,STRREF:1,STR5_BASE:2,CSTRING:3,STR8:4,STR_:5,EXTENSION:6,FALSE:7,TRUE:8,NULL:9,UNDEFINED:10,NINT4_BASE:11,NINT8:12,NINT16:13,NINT32:14,NINT64:15,FLOAT32:16,TIMESTAMP:17,BINARY_:18,BARRAY4_BASE:19,BARRAY8:20,BARRAY_:21,ARRAY5_BASE:22,ARRAY8:23,ARRAY_:24,BMAP_:25,MAP_:26,UINT14_BASE:27,UINT16:28,UINT24:29,UINT32:30,UINT64:31,DOUBLE64:32};
function $KEY_329(a,b){if(a.length===b.length){return a.every((v,i)=>b[i]===v);}return false;}
function registryFactory(){
 function R(){if(!(this instanceof R)){throw new TypeError();}this.extensions=Object.create(null);}
 function extend(k,d,s,u){this.extensions[k]={detector:d,serialiser:s,deserialiser:u};}
 R.prototype.extend=extend;
 return R;
}
let $BM=registryFactory();
function $THREAD_1_36695($BT) {
			let $KEY_0;
			class $THREAD_2_98508 extends $BT {
				constructor($BU, $BV) {
					if ($BU != null && $BV == null || $BU == null && $BV != null) {
						throw new TypeError("depthBound must be specified if and only if depthBoundExtensionPoint is");
					}
					super();
					this.keysets = [];
					this.stringHist = {};
					this.stringPlaceholders = true;
					this.remainingDepth = $BU == null ? null : $BU;
					this.depthBoundExtensionPoint = $BV ?? null;
				}
				encode() {
					let $KEY_4;
					let $KEY_5 = arguments;
					let $BU;
					if ($KEY_5.length <= 1) {
						$BU = true;
					} else {
						$KEY_5 = arguments;
						$BU = $KEY_5[1] === undefined;
					}
					if ($BU) {
						$KEY_5 = {};
					} else {
						$KEY_5 = arguments[1];
					}
					$KEY_4 = [];
					if ($KEY_5.keysetsToOmit != null) {
						Array.prototype.push.apply(this.keysets, $KEY_5.keysetsToOmit);
					}
					let $BV = this.encodeValue(arguments[0], []);
					if ($KEY_5.keysetsToOmit != null) {
						this.keysets = this.keysets.slice($KEY_5.keysetsToOmit.length);
					}
					let $BW = this.encodeValue(this.keysets, []);
					let $KEY_0 = $KEY_180(this.stringHist);
					this.stringPlaceholders = false;
					if ($KEY_0.length > 0 || this.keysets.length > 0) {
						$KEY_4.push($KEY_236.STRLUT);
						$KEY_4.push($KEY_0.length);
						this.pushArrayElements($KEY_0, $KEY_4);
						$BV = $BW.concat($BV);
					}
					$BV.forEach($BX => {
						if (typeof $BX === "string") {
							let $BY = $KEY_4;
							const $BZ = $KEY_0 ? $KEY_0.indexOf($BX) : -1;
							if ($BZ !== -1) {
								$BY.push($KEY_236.STRREF, $BZ);
							} else {
								let $CA = [];
								let $CB = unescape(encodeURIComponent($BX));
								let j = 0;
								let $CC = false;
								for (; j < $CB.length; j++) {
									$CA.push($CB.charCodeAt(j));
									if ($CA[j] === 0) {
										$CC = true;
									}
								}
								let $CD = $CA.length;
								if ($CD < 32) {
									$BY.push($KEY_236.STR5_BASE | $CD);
								} else if (!$CC) {
									$BY.push($KEY_236.CSTRING);
									$CA.push(0);
								} else if ($CD <= 255) {
									$BY.push($KEY_236.STR8, $CD);
								} else {
									$BY.push($KEY_236.STR_);
									$KEY_168($CD, $BY);
								}
								if ($CA.length > 65535) {
									for (let i = 0; i < $CA.length; i = i + 65535) {
										Array.prototype.push.apply($BY, $CA.slice(i, i + 65535));
									}
								} else {
									Array.prototype.push.apply($BY, $CA);
								}
							}
						} else {
							$KEY_4.push($BX);
						}
					});
					return $KEY_4;
				}
				findKeysetIndex($BU) {
					let $KEY_4 = $BU;
					let $BV = this.keysets;
					function $THREAD_2_22982($BZ) {
						return $KEY_329($BZ, $KEY_4);
					}
					let $BW = 0;
					let $BX, $BY;
					for (;;) {
						$BX = void 0;
						$BY = void 0;
						if ($BW >= $BV.length) {
							$BX = -1;
							$BY = -1;
							break;
						}
						if (!$THREAD_2_22982($BV[$BW])) {
							++$BW;
							continue;
						}
						$BX = $BW;
						$BY = $BW;
						break;
					}
					if ($BX < 0) {
						return this.keysets.push($KEY_4) - 1;
					} else {
						return $BY;
					}
				}
				pushArrayElements($BU, $BV) {
					let $KEY_1 = $BV;
					let $KEY_3 = this;
					Array.prototype.forEach.call($BU, $BW => {
						$KEY_3.encodeValue($BW, $KEY_1);
					});
				}
				encodeValue($BU, $BV) {
					let $KEY_8 = $BV;
					let $KEY_9 = $BU;
					let $KEY_12 = this;
					let $BW = Object.keys(this.extensions);
					function $THREAD_9_32383($CQ) {
						return $KEY_12.extensions[$CQ].detector($KEY_9);
					}
					let $BX = 0;
					let $BY;
					for (;;) {
						$BY = void 0;
						if ($BX >= $BW.length) {
							$BY = null;
							break;
						}
						$BY = $BW[$BX];
						if (!$THREAD_9_32383($BY)) {
							++$BX;
							continue;
						}
						break;
					}
					if ($BY != null) {
						$KEY_8.push($KEY_236.EXTENSION);
						$KEY_168(Number($BY), $KEY_8);
						this.encodeValue(this.extensions[Number($BY)].serialiser($KEY_9), $KEY_8);
					} else if ($KEY_9 === false) {
						$KEY_8.push($KEY_236.FALSE);
					} else if ($KEY_9 === true) {
						$KEY_8.push($KEY_236.TRUE);
					} else if ($KEY_9 === null) {
						$KEY_8.push($KEY_236.NULL);
					} else if (typeof $KEY_9 === "undefined") {
						$KEY_8.push($KEY_236.UNDEFINED);
					} else if (typeof $KEY_9 === "number") {
						if (isFinite($KEY_9)) {
							let $BZ = Math.abs($KEY_9);
							if (Math.floor($BZ) === $BZ && $BZ < 0xfffffffffffff800 && ($BZ !== 0 || 1 / $KEY_9 > 0)) {
								let $CA = $KEY_9;
								if ($CA === 0 && 1 / $CA === -Infinity) {
									$KEY_8.push($KEY_236.NINT4_BASE);
								} else if ($CA >= 0) {
									$KEY_168($CA, $KEY_8);
								} else if (-$CA <= 15) {
									$KEY_8.push($KEY_236.NINT4_BASE | -$CA);
								} else if (-$CA <= 255) {
									$KEY_8.push($KEY_236.NINT8, -$CA);
								} else if (-$CA <= 65535) {
									$KEY_8.push($KEY_236.NINT16, -$CA >> 8, -$CA & 255);
								} else if (-$CA <= 4294967295) {
									$KEY_8.push($KEY_236.NINT32);
									$KEY_8.push(-$CA >>> 24, -$CA >> 16 & 255, -$CA >> 8 & 255, -$CA & 255);
								} else {
									$KEY_8.push($KEY_236.NINT64);
									$KEY_8.push((-$CA / 4294967296 & 4294967295) >>> 24, (-$CA / 4294967296 & 4294967295) >> 16 & 255, (-$CA / 4294967296 & 4294967295) >> 8 & 255, -$CA / 4294967296 & 4294967295 & 255);
									$KEY_8.push((-$CA & 4294967295) >>> 24, (-$CA & 4294967295) >> 16 & 255, (-$CA & 4294967295) >> 8 & 255, -$CA & 4294967295 & 255);
								}
							} else {
								$KEY_449($KEY_9, $KEY_8);
							}
						} else if ($KEY_9 === 1 / 0) {
							$KEY_8.push($KEY_236.FLOAT32, 127, 128, 0, 0);
						} else if ($KEY_9 === -1 / 0) {
							$KEY_8.push($KEY_236.FLOAT32, 255, 128, 0, 0);
						} else if ($KEY_9 !== $KEY_9) {
							$KEY_8.push($KEY_236.FLOAT32, 127, 192, 0, 0);
						}
					} else if (typeof $KEY_9 === "string") {
						if (this.stringPlaceholders) {
							this.stringHist[$KEY_9] = (this.stringHist[$KEY_9] || 0) + 1;
							$KEY_8.push($KEY_9);
						} else {
							let $CC = $KEY_8;
							let $CD = [];
							let $CE = unescape(encodeURIComponent($KEY_9));
							let i = 0;
							let $CF = false;
							for (; i < $CE.length; i++) {
								$CD.push($CE.charCodeAt(i));
								if ($CD[i] === 0) {
									$CF = true;
								}
							}
							let $CG = $CD.length;
							if ($CG < 32) {
								$CC.push($KEY_236.STR5_BASE | $CG);
							} else if (!$CF) {
								$CC.push($KEY_236.CSTRING);
								$CD.push(0);
							} else if ($CG <= 255) {
								$CC.push($KEY_236.STR8, $CG);
							} else {
								$CC.push($KEY_236.STR_);
								$KEY_168($CG, $CC);
							}
							if ($CD.length > 65535) {
								for (let j = 0; j < $CD.length; j = j + 65535) {
									Array.prototype.push.apply($CC, $CD.slice(j, j + 65535));
								}
							} else {
								Array.prototype.push.apply($CC, $CD);
							}
						}
					} else if (Object.prototype.toString.call($KEY_9) === "[object Date]") {
						let $CH = $KEY_8;
						let $CI = Date.prototype.getTime.call($KEY_9);
						let $CJ = $CI / 4294967296;
						$CJ = $CI < 0 ? Number($CJ) - 1 : $CJ;
						$CH.push($KEY_236.TIMESTAMP, $CJ >>> 8 & 255, $CJ & 255);
						$CH.push($CI >>> 0 >>> 24, $CI >>> 0 >> 16 & 255, $CI >>> 0 >> 8 & 255, $CI >>> 0 & 255);
					} else if (typeof ArrayBuffer !== "undefined" && $KEY_9 instanceof ArrayBuffer) {
						$KEY_8.push($KEY_236.BINARY_);
						this.encodeValue($KEY_9.byteLength, $KEY_8);
						this.pushArrayElements(new Uint8Array($KEY_9), $KEY_8);
					} else {
						if (this.remainingDepth !== null) {
							if (this.remainingDepth <= 0) {
								$KEY_8.push($KEY_236.EXTENSION);
								if (this.depthBoundExtensionPoint == null) {
									throw new Error("if remainingDepth != null, depthBoundExtensionPoint must not be either");
								} else {
									$KEY_168(this.depthBoundExtensionPoint, $KEY_8);
									$KEY_8.push($KEY_236.NULL);
									return $KEY_8;
								}
							} else {
								this.remainingDepth = Number(this.remainingDepth) - 1;
							}
						}
						if (Array.isArray($KEY_9)) {
							let $CK = $KEY_9.length;
							if ($KEY_9.every($CQ => typeof $CQ === "boolean") && $CK > 0) {
								if ($CK <= 15) {
									$KEY_8.push($KEY_236.BARRAY4_BASE | $CK);
								} else if ($CK <= 255) {
									$KEY_8.push($KEY_236.BARRAY8, $CK);
								} else {
									$KEY_8.push($KEY_236.BARRAY_);
									$KEY_168($CK, $KEY_8);
								}
								for (let k = 0; k < $CK; k = k + 8) {
									$KEY_8.push($KEY_9[k] << 7 | $KEY_9[k + 1] << 6 | $KEY_9[k + 2] << 5 | $KEY_9[k + 3] << 4 | $KEY_9[k + 4] << 3 | $KEY_9[k + 5] << 2 | $KEY_9[k + 6] << 1 | $KEY_9[k + 7]);
								}
							} else {
								if ($CK <= 31) {
									$KEY_8.push($KEY_236.ARRAY5_BASE | $CK);
								} else if ($CK <= 255) {
									$KEY_8.push($KEY_236.ARRAY8, $CK);
								} else {
									$KEY_8.push($KEY_236.ARRAY_);
									$KEY_168($CK, $KEY_8);
								}
								this.pushArrayElements($KEY_9, $KEY_8);
							}
						} else {
							let $CL = Object.keys($KEY_9).sort();
							let $CM = $CL.length;
							let $CN = this.findKeysetIndex($CL);
							if ($CL.every($CQ => typeof $KEY_9[$CQ] === "boolean")) {
								$KEY_8.push($KEY_236.BMAP_);
								$KEY_168($CN, $KEY_8);
								let $CO = [0, 0, 0, 0, 0, 0, 0, 0];
								let $CP = 0;
								while ($CP < $CM) {
									let i_2 = 0;
									for (; i_2 < 8; i_2++) {
										$CO[i_2] = $CP + i_2 < $CM && $KEY_9[$CL[$CP + i_2]];
									}
									$KEY_8.push($CO[0] << 7 | $CO[1] << 6 | $CO[2] << 5 | $CO[3] << 4 | $CO[4] << 3 | $CO[5] << 2 | $CO[6] << 1 | $CO[7]);
									$CP += 8;
								}
							} else {
								$KEY_8.push($KEY_236.MAP_);
								$KEY_168($CN, $KEY_8);
								$CL.forEach($CQ => $KEY_12.encodeValue($KEY_9[$CQ], $KEY_8));
							}
						}
						if (this.remainingDepth !== null) {
							this.remainingDepth = Number(this.remainingDepth) + 1;
						}
					}
					return $KEY_8;
				}
				static encode() {
					let $KEY_7, $KEY_8, $KEY_9;
					let $KEY_3 = arguments;
					let $BU;
					if ($KEY_3.length <= 1) {
						$BU = true;
					} else {
						$KEY_3 = arguments;
						$BU = $KEY_3[1] === undefined;
					}
					if ($BU) {
						$KEY_9 = {};
					} else {
						$KEY_3 = arguments;
						$KEY_9 = $KEY_3[1];
					}
					$KEY_8 = null;
					if ($KEY_9.depthBound != null) {
						let $BV = $KEY_9.depthBound;
						if ($BV < 0 || Math.floor($BV) !== $BV) {
							throw new RangeError("depthBound, if specified, must be a non-negative integer");
						}
						if ($KEY_9.extensions != null) {
							Object.keys($KEY_9.extensions).forEach($BX => {
								let $KEY_1 = Number($BX);
								if ($KEY_9.extensions[$KEY_1] === $KEY_327) {
									$KEY_8 = $KEY_1;
								}
							});
						}
						if ($KEY_8 == null) {
							throw new Error("if depthBound is used, its corresponding extension must be provided");
						} else {
							$KEY_7 = new $KEY_0($KEY_9.depthBound, $KEY_8);
							if ($KEY_9.extensions != null) {
								Object.keys($KEY_9.extensions).forEach($BX => {
									$KEY_7.extend($BX, $KEY_9.extensions[$BX].detector, $KEY_9.extensions[$BX].serialiser, $KEY_9.extensions[$BX].deserialiser);
								});
							}
							return $KEY_7.encode(arguments[0], $KEY_9);
						}
					} else {
						let $BW;
						if ($KEY_9.extensions != null) {
							$BW = Object.keys($KEY_9.extensions).some($BX => $KEY_9.extensions[$BX] === $KEY_327);
						} else {
							$BW = false;
						}
						if ($BW) {
							throw new Error("if the depthBound extension is used, a depth bound must be specified");
						} else {
							$KEY_7 = new $KEY_0($KEY_9.depthBound, $KEY_8);
							if ($KEY_9.extensions != null) {
								Object.keys($KEY_9.extensions).forEach($BX => {
									$KEY_7.extend($BX, $KEY_9.extensions[$BX].detector, $KEY_9.extensions[$BX].serialiser, $KEY_9.extensions[$BX].deserialiser);
								});
							}
							return $KEY_7.encode(arguments[0], $KEY_9);
						}
					}
				}
			}
			$KEY_0 = $THREAD_2_98508;
			return $KEY_0;
		}
let encoder=$THREAD_1_36695($BM).encode;return encoder;}main();"###;
#[test]
fn nonuniform_seed_packet_and_mutated_table_or_comparator() {
    assert_eq!(count(NONUNIFORM), 1);
    assert_eq!(
        count(&NONUNIFORM.replace("return encoder;", "$KEY_236=Other;return encoder;")),
        0
    );
    assert_eq!(
        count(&ORIGINAL.replace("return encoder;", "$KEY_329=Other;return encoder;")),
        0
    );
    assert_eq!(
        count(&ORIGINAL.replace(
            "Number(this.remainingDepth) - 1",
            "Number(this.remainingDepth) + 1"
        )),
        0
    );
}

#[test]
fn checked_route_applies_both_owned_encoder_variants_and_exact_reference_edits() {
    use shape::naming::name_threads;
    for (source, binding) in [(ORIGINAL, "$THREAD_3_16462"), (NONUNIFORM, "$THREAD_2_98508")] {
        let result = name_threads(source, true).unwrap();
        let class = result.report.class_outcomes.iter().find(|c| c.binding.name == binding).unwrap();
        assert!(class.applied && class.reference_edits > 0, "{class:?}");
        assert_eq!(class.proposed_name.as_deref(), Some("ValueEncoder"));
        assert_eq!(class.evaluator.as_deref(), Some("owned_value_encoder_class"));
        assert!(!result.source.contains(binding));
        assert!(result.source.contains("encodeValue("));
        assert!(result.report.outcomes.iter().all(|o| o.binding.name != binding));
        let mut replay = source.to_string();
        for e in result.report.edits.iter().rev() { replay.replace_range(e.start as usize..e.end as usize, &e.replacement); }
        assert_eq!(replay, result.source);
        name_threads(&result.source, false).unwrap();
    }
}

#[test]
fn encoder_route_dryrun_readable_collision_capture_and_dynamic_scope() {
    use shape::naming::name_threads;
    let dry = name_threads(ORIGINAL, false).unwrap();
    assert_eq!(dry.source, ORIGINAL);
    assert!(dry.report.edits.is_empty());
    let human = ORIGINAL.replace("$THREAD_3_16462", "UserEncoder");
    let result = name_threads(&human, true).unwrap();
    assert!(result.source.contains("class UserEncoder"));
    assert!(result.report.class_outcomes.is_empty());
    for source in [
        ORIGINAL.replace("let $KEY_3;", "let $KEY_3; let ValueEncoder = 1; consume(ValueEncoder);"),
        ORIGINAL.replace("let $KEY_3;", "let $KEY_3; consume(ValueEncoder);"),
        format!("eval('');{ORIGINAL}"),
        format!("with (context) {{ {ORIGINAL} }}"),
    ] {
        let result = name_threads(&source, true).unwrap();
        let class = result.report.class_outcomes.iter().find(|c| c.binding.name == "$THREAD_3_16462").unwrap();
        assert!(!class.applied && class.reason.is_some(), "{class:?}");
    }
}

// The original factory bytes and UTF8 spans are pinned in the sibling provenance
// file. Recognition parses and builds Model even for every refusal below.
const GUARDED_CURRENT: &str = include_str!("fixtures/program/encoder/guarded.js");

fn checked_encoder_count(source: &str) -> usize {
    identify_value_encoder_classes(source)
        .expect("fixture must parse and construct a valid Model before checking refusal")
        .len()
}

#[test]
fn current_tuple_header_fresh_array_guard_and_exact_tuple_header() {
    assert_eq!(checked_encoder_count(GUARDED_CURRENT), 1);
    let tuple = GUARDED_CURRENT
        .replace("$KEY_2.push($KEY_374.STRLUT);", "const headerTag=[$KEY_2,$KEY_2.push];headerTag[1].call(headerTag[0],$KEY_374.STRLUT);")
        .replace("$KEY_2.push($KEY_4.length);", "const headerLength=[$KEY_2,$KEY_2.push];headerLength[1].call(headerLength[0],$KEY_4.length);");
    assert_ne!(tuple, GUARDED_CURRENT);
    assert_eq!(checked_encoder_count(&tuple), 1);
    // Renaming is independent of the identity proof, including human names and
    // non-ASCII text before the class's original byte spans.
    let renamed = format!(
        "/* π */{}",
        tuple.replace("$THREAD_5_148898", "ReadableEncoder")
    );
    let proofs = identify_value_encoder_classes(&renamed).unwrap();
    assert_eq!(proofs.len(), 1);
    assert!(
        renamed[proofs[0].class_span.start as usize..proofs[0].class_span.end as usize]
            .starts_with("class ReadableEncoder")
    );
}

#[test]
fn guarded_encoder_refuses_foreign_outputs_origins_and_dormant_headers() {
    for (from, to) in [
        ("if ($KEY_2) {", "if (foreign) {"),
        ("return $KEY_2;", "return foreign;"),
        ("return $KEY_5;", "return foreign;"),
        ("return $BT;", "return foreign;"),
        ("let $KEY_2 = [];", "let $KEY_2 = foreign;"),
        ("return $KEY_2;", "$KEY_2=foreign;return $KEY_2;"),
        ("let $KEY_2 = [];", "let $KEY_2 = [];leak($KEY_2);"),
        ("let $BV = $KEY_2;", "let $BV = $KEY_2;leak($BV);"),
        (
            "$KEY_2.push($KEY_374.STRLUT);",
            "$KEY_2.push(foreign.STRLUT);",
        ),
        (
            "$KEY_2.push($KEY_374.STRLUT);",
            "$KEY_374.STRLUT=0;$KEY_2.push($KEY_374.STRLUT);",
        ),
        (
            "$KEY_2.push($KEY_374.STRLUT);",
            "leak($KEY_374);$KEY_2.push($KEY_374.STRLUT);",
        ),
        (
            "$KEY_2.push($KEY_4.length);",
            "$KEY_2.push(foreign.length);",
        ),
        (
            "this.pushArrayElements($KEY_4, $KEY_2);",
            "foreign.pushArrayElements($KEY_4, $KEY_2);",
        ),
        (
            "this.pushArrayElements($KEY_4, $KEY_2);",
            "this.pushArrayElements($KEY_4, foreign);",
        ),
        (
            "$KEY_4.length > 0 || this.keysets.length > 0",
            "foreign || this.keysets.length > 0",
        ),
        (
            "$KEY_2.push($KEY_374.STRLUT);",
            "function dormant(){$KEY_2.push($KEY_374.STRLUT);}",
        ),
        (
            "$KEY_2.push($KEY_374.STRLUT);",
            "$KEY_2.push($KEY_4.length);",
        ),
        (
            "return $KEY_294($BU, $KEY_1);",
            "return foreign($BU, $KEY_1);",
        ),
    ] {
        let changed = GUARDED_CURRENT.replace(from, to);
        assert_ne!(changed, GUARDED_CURRENT, "missing mutation anchor: {from}");
        assert_eq!(
            checked_encoder_count(&changed),
            0,
            "accepted mutation: {to}"
        );
    }
}

#[test]
fn tuple_header_requires_exact_call_receiver_slots_and_no_escape() {
    let tuple = GUARDED_CURRENT.replace(
        "$KEY_2.push($KEY_374.STRLUT);",
        "const headerTag=[$KEY_2,$KEY_2.push];headerTag[1].call(headerTag[0],$KEY_374.STRLUT);",
    );
    for (from, to) in [
        ("[$KEY_2,$KEY_2.push]", "[foreign,$KEY_2.push]"),
        ("[$KEY_2,$KEY_2.push]", "[$KEY_2,foreign.push]"),
        ("[$KEY_2,$KEY_2.push]", "[$KEY_2,$KEY_2.pop]"),
        ("headerTag[1].call", "headerTag[0].call"),
        ("call(headerTag[0],", "call(headerTag[1],"),
        ("call(headerTag[0],", "call(foreign,"),
        (".call(headerTag[0],", ".apply(headerTag[0],"),
        ("headerTag[1].call", "leak(headerTag);headerTag[1].call"),
        (
            "headerTag[1].call",
            "headerTag[1]=foreign;headerTag[1].call",
        ),
        ("headerTag[1].call", "if (foreign) headerTag[1].call"),
    ] {
        let changed = tuple.replace(from, to);
        assert_ne!(changed, tuple);
        assert_eq!(
            checked_encoder_count(&changed),
            0,
            "accepted carrier mutation: {to}"
        );
    }
}

const REVERSED_CURRENT: &str = include_str!("fixtures/program/encoder/reversed-assembly.js");

#[test]
fn current_guarded_header_assembles_keysets_before_payload_and_consumes_it() {
    // Same retained current witness; no second seed. Every refusal below invokes
    // the parser and Model constructor through checked_encoder_count first.
    assert_eq!(checked_encoder_count(GUARDED_CURRENT), 1);
    assert_eq!(REVERSED_CURRENT, GUARDED_CURRENT.replace("$BR = $BS.concat($BR);", "$BR = $BR.concat($BS);"));
    assert_eq!(checked_encoder_count(REVERSED_CURRENT), 0);
    for (from, to) in [
        ("$BR = $BS.concat($BR);", "$BR = $BS.concat($BS);"),
        ("$BR = $BS.concat($BR);", "$BS = $BS.concat($BR);"),
        ("$BR = $BS.concat($BR);", "$BR = foreign.concat($BR);"),
        ("$BR = $BS.concat($BR);", "$BR = $BS.slice($BR);"),
        ("$BR = $BS.concat($BR);", "$BR = $BS.concat(...foreign,$BR);"),
        ("$BR = $BS.concat($BR);", "$BR = $BS.concat($BR);leak($KEY_4);"),
        ("$BR = $BS.concat($BR);", "$BR = $BS.concat($BR);foreign=1;"),
        ("$BR = $BS.concat($BR);", "$BR = foreign;"),
        ("$BR = $BS.concat($BR);", "function dormant(){$BR = $BS.concat($BR);}"),
        ("$BR = $BS.concat($BR);", "leak($BS);$BR = $BS.concat($BR);"),
        ("$BR.forEach($BU => {", "leak($BR);$BR.forEach($BU => {"),
        ("$BR.forEach($BU => {", "foreign.forEach($BU => {"),
        ("$BR.forEach($BU => {", "$BS.forEach($BU => {"),
        ("$BR.forEach($BU => {", "$BR.forEach($BU => {});$BR.forEach($BU => {"),
        ("$BR.forEach($BU => {", "$BR.forEach($BU => { leak($BU);"),
        ("$BR.forEach($BU => {", "$BR.forEach(...foreign,$BU => {"),
        ("$KEY_2.push($BU);", "$KEY_2.push(0);"),
        ("$KEY_2.push($BU);", "foreign.push($BU);"),
        ("$BV.push($KEY_374.STRREF, $BW);", "$BV.push($BW, $KEY_374.STRREF);"),
        ("$BV.push($KEY_374.STRREF, $BW);", "$BV.push($KEY_374.STRREF, 0);"),
        ("$BV.push($KEY_374.STRREF, $BW);", "$BV.push($KEY_374.STRREF, $BW);leak($BV);"),
        ("$KEY_4.indexOf($BU)", "$KEY_4.indexOf(foreign)"),
        ("unescape(encodeURIComponent($BU))", "encodeURIComponent($BU)"),
        ("$BX.push($BY.charCodeAt(j));", "$BX.push(0);"),
        ("$KEY_14($CA, $BV);", "foreign($CA, $BV);"),
        ("$BX.slice(i, i + 65535)", "$BX.slice(i + 65535, i)"),
        ("Array.prototype.push.apply($BV, $BX);", "Array.prototype.push.apply($BX, $BV);"),
    ] {
        let changed = GUARDED_CURRENT.replace(from, to);
        assert_ne!(changed, GUARDED_CURRENT, "missing mutation anchor: {from}");
        assert_eq!(checked_encoder_count(&changed), 0, "accepted assembly/consumer mutation: {to}");
    }
}

#[test]
fn guarded_complete_consumer_contract_is_alpha_and_format_independent() {
    let renamed = GUARDED_CURRENT
        .replace("$BU", "elementOrKey")
        .replace("$BV", "byteOutput")
        .replace("$BW", "stringIndex")
        .replace("$BX", "utf8Bytes")
        .replace("$BY", "utf8String")
        .replace("$BZ", "containsZero")
        .replace("$CA", "byteLength")
        .replace("Array.prototype.push.apply($BV, $BX);", "/* preserve grammar, ignore comment */ Array.prototype.push.apply( $BV , $BX );")
        .replace("$BR = $BS.concat($BR);", "$BR = ( /* keysets precede payload */ $BS.concat( $BR ) );");
    assert_eq!(checked_encoder_count(&renamed), 1);
}

#[test]
fn guarded_encoder_requires_complete_reachable_owned_prefix() {
    let unreachable = include_str!("fixtures/program/encoder/unreachable-header.js");
    assert_eq!(unreachable, GUARDED_CURRENT.replacen("encode() {\n", "encode() {\nthrow new Error(\"stop\");\n", 1));
    // Do not replace parse/Model errors with zero. Check this nearest negative
    // first so the receipt records it even if the retained positive still fails.
    assert_eq!(checked_encoder_count(unreachable), 0);
    assert_eq!(checked_encoder_count(GUARDED_CURRENT), 1);
}

#[test]
fn owned_consumer_length_writer_joins_output_use_proof_without_allowing_escape() {
    // This STR_ writer call is inside the complete proved callback, and shares
    // its actual unsigned-writer identity with encodeValue's EXTENSION branch.
    assert!(GUARDED_CURRENT.contains("$BV.push($KEY_374.STR_);"));
    assert!(GUARDED_CURRENT.contains("$KEY_14($CA, $BV);"));
    assert_eq!(checked_encoder_count(GUARDED_CURRENT), 1);
    assert_eq!(checked_encoder_count(&GUARDED_CURRENT.replace("$KEY_14", "renamedUnsignedWriter")), 1);
    let foreign_writer = include_str!("fixtures/program/encoder/foreign-consumer-length-writer.js");
    assert_eq!(foreign_writer, GUARDED_CURRENT.replace("$KEY_14($CA, $BV);", "foreignLengthWriter($CA, $BV);"));
    assert_eq!(checked_encoder_count(foreign_writer), 0);
    for (from, to) in [
        ("$KEY_14($CA, $BV);", "$KEY_14($CA, $BV);leak($BV);"),
        ("$KEY_14($CA, $BV);", "$KEY_14($CA, $KEY_2);"),
        ("$KEY_14($CA, $BV);", "$KEY_14($BV, $CA);"),
        ("$KEY_14($CA, $BV);", "$KEY_14($CA, $BV);leak($KEY_2);"),
        ("let $KEY_2 = [];", "let $KEY_2 = [];leak($KEY_2);"),
        ("$BR.forEach($BU => {", "leak($KEY_2);$BR.forEach($BU => {"),
        ("return $KEY_2;", "leak($KEY_2);return $KEY_2;"),
    ] {
        let changed = GUARDED_CURRENT.replace(from, to);
        assert_ne!(changed, GUARDED_CURRENT, "missing mutation anchor: {from}");
        assert_eq!(checked_encoder_count(&changed), 0, "accepted writer/output escape: {to}");
    }
}
