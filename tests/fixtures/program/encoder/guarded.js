function main(){
let $KEY_374={STRLUT:0,STRREF:1,STR5_BASE:2,CSTRING:3,STR8:4,STR_:5,EXTENSION:6,FALSE:7,TRUE:8,NULL:9,UNDEFINED:10,NINT4_BASE:11,NINT8:12,NINT16:13,NINT32:14,NINT64:15,FLOAT32:16,TIMESTAMP:17,BINARY_:18,BARRAY4_BASE:19,BARRAY8:20,BARRAY_:21,ARRAY5_BASE:22,ARRAY8:23,ARRAY_:24,BMAP_:25,MAP_:26,UINT14_BASE:27,UINT16:28,UINT24:29,UINT32:30,UINT64:31,DOUBLE64:32};
function $KEY_294(a,b){if(a.length===b.length){return a.every((v,i)=>b[i]===v);}return false;}
function registryFactory(){
 function R(){if(!(this instanceof R)){throw new TypeError();}this.extensions=Object.create(null);}
 function extend(k,d,s,u){this.extensions[k]={detector:d,serialiser:s,deserialiser:u};}
 R.prototype.extend=extend;
 return R;
}
let $BM=registryFactory();
function $THREAD_0_146772($BP) {
					let $KEY_0;
					class $THREAD_5_148898 extends $BP {
						constructor($BQ, $BR) {
							if ($BQ != null && $BR == null || $BQ == null && $BR != null) {
								throw new TypeError("depthBound must be specified if and only if depthBoundExtensionPoint is");
							}
							super();
							this.keysets = [];
							this.stringHist = {};
							this.stringPlaceholders = true;
							this.remainingDepth = $BQ == null ? null : $BQ;
							this.depthBoundExtensionPoint = $BR ?? null;
						}
						encode() {
							let $KEY_6 = arguments;
							let $BQ;
							if ($KEY_6.length <= 1) {
								$BQ = true;
							} else {
								$KEY_6 = arguments;
								$BQ = $KEY_6[1] === undefined;
							}
							if ($BQ) {
								$KEY_6 = {};
							} else {
								$KEY_6 = arguments[1];
							}
							let $KEY_2 = [];
							if ($KEY_6.keysetsToOmit != null) {
								Array.prototype.push.apply(this.keysets, $KEY_6.keysetsToOmit);
							}
							let $BR = this.encodeValue(arguments[0], []);
							if ($KEY_6.keysetsToOmit != null) {
								this.keysets = this.keysets.slice($KEY_6.keysetsToOmit.length);
							}
							let $BS = this.encodeValue(this.keysets, []);
							let $KEY_4 = $KEY_305(this.stringHist);
							this.stringPlaceholders = false;
							if ($KEY_4.length > 0 || this.keysets.length > 0) {
								$KEY_2.push($KEY_374.STRLUT);
								$KEY_2.push($KEY_4.length);
								this.pushArrayElements($KEY_4, $KEY_2);
								$BR = $BS.concat($BR);
							}
							$BR.forEach($BU => {
								if (typeof $BU === "string") {
									let $BV = $KEY_2;
									const $BW = $KEY_4 ? $KEY_4.indexOf($BU) : -1;
									if ($BW !== -1) {
										$BV.push($KEY_374.STRREF, $BW);
									} else {
										let $BX = [];
										let $BY = unescape(encodeURIComponent($BU));
										let j = 0;
										let $BZ = false;
										for (; j < $BY.length; j++) {
											$BX.push($BY.charCodeAt(j));
											if ($BX[j] === 0) {
												$BZ = true;
											}
										}
										let $CA = $BX.length;
										if ($CA < 32) {
											$BV.push($KEY_374.STR5_BASE | $CA);
										} else if (!$BZ) {
											$BV.push($KEY_374.CSTRING);
											$BX.push(0);
										} else if ($CA <= 255) {
											$BV.push($KEY_374.STR8, $CA);
										} else {
											$BV.push($KEY_374.STR_);
											$KEY_14($CA, $BV);
										}
										if ($BX.length > 65535) {
											for (let i = 0; i < $BX.length; i = i + 65535) {
												Array.prototype.push.apply($BV, $BX.slice(i, i + 65535));
											}
										} else {
											Array.prototype.push.apply($BV, $BX);
										}
									}
								} else {
									$KEY_2.push($BU);
								}
							});
							if ($KEY_2) {
								return $KEY_2;
							} else {
								throw new TypeError("Cannot read properties of undefined (reading 'value')");
							}
						}
						findKeysetIndex($BQ) {
							let $KEY_1 = $BQ;
							function $THREAD_1_86030($BU) {
								return $KEY_294($BU, $KEY_1);
							}
							let $BR = this.keysets;
							let $BS = 0;
							let $BT;
							for (;;) {
								$BT = void 0;
								if ($BS >= $BR.length) {
									$BT = -1;
									break;
								}
								if (!$THREAD_1_86030($BR[$BS])) {
									++$BS;
									continue;
								}
								$BT = $BS;
								break;
							}
							if ($BT < 0) {
								return this.keysets.push($KEY_1) - 1;
							} else {
								return $BT;
							}
						}
						pushArrayElements($BQ, $BR) {
							let $KEY_3 = $BR;
							let $KEY_2 = this;
							Array.prototype.forEach.call($BQ, $BS => {
								$KEY_2.encodeValue($BS, $KEY_3);
							});
						}
						encodeValue($BQ, $BR) {
							let $KEY_5 = $BR;
							let $KEY_9 = $BQ;
							let $KEY_10 = this;
							let $BS = Object.keys(this.extensions);
							function $THREAD_7_71658($CM) {
								return $KEY_10.extensions[$CM].detector($KEY_9);
							}
							let $BT = 0;
							let $BU;
							for (;;) {
								$BU = void 0;
								if ($BT >= $BS.length) {
									$BU = null;
									break;
								}
								$BU = $BS[$BT];
								if (!$THREAD_7_71658($BU)) {
									++$BT;
									continue;
								}
								break;
							}
							if ($BU != null) {
								$KEY_5.push($KEY_374.EXTENSION);
								$KEY_14(Number($BU), $KEY_5);
								this.encodeValue(this.extensions[Number($BU)].serialiser($KEY_9), $KEY_5);
							} else if ($KEY_9 === false) {
								$KEY_5.push($KEY_374.FALSE);
							} else if ($KEY_9 === true) {
								$KEY_5.push($KEY_374.TRUE);
							} else if ($KEY_9 === null) {
								$KEY_5.push($KEY_374.NULL);
							} else if (typeof $KEY_9 === "undefined") {
								$KEY_5.push($KEY_374.UNDEFINED);
							} else if (typeof $KEY_9 === "number") {
								if (isFinite($KEY_9)) {
									let $BV = Math.abs($KEY_9);
									if (Math.floor($BV) === $BV && $BV < 0xfffffffffffff800 && ($BV !== 0 || 1 / $KEY_9 > 0)) {
										let $BW = $KEY_9;
										if ($BW === 0 && 1 / $BW === -Infinity) {
											$KEY_5.push($KEY_374.NINT4_BASE);
										} else if ($BW >= 0) {
											$KEY_14($BW, $KEY_5);
										} else if (-$BW <= 15) {
											$KEY_5.push($KEY_374.NINT4_BASE | -$BW);
										} else if (-$BW <= 255) {
											$KEY_5.push($KEY_374.NINT8, -$BW);
										} else if (-$BW <= 65535) {
											$KEY_5.push($KEY_374.NINT16, -$BW >> 8, -$BW & 255);
										} else if (-$BW <= 4294967295) {
											$KEY_5.push($KEY_374.NINT32);
											$KEY_5.push(-$BW >>> 24, -$BW >> 16 & 255, -$BW >> 8 & 255, -$BW & 255);
										} else {
											$KEY_5.push($KEY_374.NINT64);
											$KEY_5.push((-$BW / 4294967296 & 4294967295) >>> 24, (-$BW / 4294967296 & 4294967295) >> 16 & 255, (-$BW / 4294967296 & 4294967295) >> 8 & 255, -$BW / 4294967296 & 4294967295 & 255);
											$KEY_5.push((-$BW & 4294967295) >>> 24, (-$BW & 4294967295) >> 16 & 255, (-$BW & 4294967295) >> 8 & 255, -$BW & 4294967295 & 255);
										}
									} else {
										$KEY_200($KEY_9, $KEY_5);
									}
								} else if ($KEY_9 === 1 / 0) {
									$KEY_5.push($KEY_374.FLOAT32, 127, 128, 0, 0);
								} else if ($KEY_9 === -1 / 0) {
									$KEY_5.push($KEY_374.FLOAT32, 255, 128, 0, 0);
								} else if ($KEY_9 !== $KEY_9) {
									$KEY_5.push($KEY_374.FLOAT32, 127, 192, 0, 0);
								}
							} else if (typeof $KEY_9 === "string") {
								if (this.stringPlaceholders) {
									this.stringHist[$KEY_9] = (this.stringHist[$KEY_9] || 0) + 1;
									$KEY_5.push($KEY_9);
								} else {
									let $BY = $KEY_5;
									let $BZ = [];
									let $CA = unescape(encodeURIComponent($KEY_9));
									let i = 0;
									let $CB = false;
									for (; i < $CA.length; i++) {
										$BZ.push($CA.charCodeAt(i));
										if ($BZ[i] === 0) {
											$CB = true;
										}
									}
									let $CC = $BZ.length;
									if ($CC < 32) {
										$BY.push($KEY_374.STR5_BASE | $CC);
									} else if (!$CB) {
										$BY.push($KEY_374.CSTRING);
										$BZ.push(0);
									} else if ($CC <= 255) {
										$BY.push($KEY_374.STR8, $CC);
									} else {
										$BY.push($KEY_374.STR_);
										$KEY_14($CC, $BY);
									}
									if ($BZ.length > 65535) {
										for (let j = 0; j < $BZ.length; j = j + 65535) {
											Array.prototype.push.apply($BY, $BZ.slice(j, j + 65535));
										}
									} else {
										Array.prototype.push.apply($BY, $BZ);
									}
								}
							} else if (Object.prototype.toString.call($KEY_9) === "[object Date]") {
								let $CD = $KEY_5;
								let $CE = Date.prototype.getTime.call($KEY_9);
								let $CF = $CE / 4294967296;
								$CF = $CE < 0 ? Number($CF) - 1 : $CF;
								$CD.push($KEY_374.TIMESTAMP, $CF >>> 8 & 255, $CF & 255);
								$CD.push($CE >>> 0 >>> 24, $CE >>> 0 >> 16 & 255, $CE >>> 0 >> 8 & 255, $CE >>> 0 & 255);
							} else if (typeof ArrayBuffer !== "undefined" && $KEY_9 instanceof ArrayBuffer) {
								$KEY_5.push($KEY_374.BINARY_);
								this.encodeValue($KEY_9.byteLength, $KEY_5);
								this.pushArrayElements(new Uint8Array($KEY_9), $KEY_5);
							} else {
								if (this.remainingDepth !== null) {
									if (this.remainingDepth <= 0) {
										$KEY_5.push($KEY_374.EXTENSION);
										if (this.depthBoundExtensionPoint == null) {
											throw new Error("if remainingDepth != null, depthBoundExtensionPoint must not be either");
										} else {
											$KEY_14(this.depthBoundExtensionPoint, $KEY_5);
											$KEY_5.push($KEY_374.NULL);
											return $KEY_5;
										}
									} else {
										this.remainingDepth = Number(this.remainingDepth) - 1;
									}
								}
								if (Array.isArray($KEY_9)) {
									let $CG = $KEY_9.length;
									if ($KEY_9.every($CM => typeof $CM === "boolean") && $CG > 0) {
										if ($CG <= 15) {
											$KEY_5.push($KEY_374.BARRAY4_BASE | $CG);
										} else if ($CG <= 255) {
											$KEY_5.push($KEY_374.BARRAY8, $CG);
										} else {
											$KEY_5.push($KEY_374.BARRAY_);
											$KEY_14($CG, $KEY_5);
										}
										for (let k = 0; k < $CG; k = k + 8) {
											$KEY_5.push($KEY_9[k] << 7 | $KEY_9[k + 1] << 6 | $KEY_9[k + 2] << 5 | $KEY_9[k + 3] << 4 | $KEY_9[k + 4] << 3 | $KEY_9[k + 5] << 2 | $KEY_9[k + 6] << 1 | $KEY_9[k + 7]);
										}
									} else {
										if ($CG <= 31) {
											$KEY_5.push($KEY_374.ARRAY5_BASE | $CG);
										} else if ($CG <= 255) {
											$KEY_5.push($KEY_374.ARRAY8, $CG);
										} else {
											$KEY_5.push($KEY_374.ARRAY_);
											$KEY_14($CG, $KEY_5);
										}
										this.pushArrayElements($KEY_9, $KEY_5);
									}
								} else {
									let $CH = Object.keys($KEY_9).sort();
									let $CI = $CH.length;
									let $CJ = this.findKeysetIndex($CH);
									if ($CH.every($CM => typeof $KEY_9[$CM] === "boolean")) {
										$KEY_5.push($KEY_374.BMAP_);
										$KEY_14($CJ, $KEY_5);
										let $CK = [0, 0, 0, 0, 0, 0, 0, 0];
										let $CL = 0;
										while ($CL < $CI) {
											let i_2 = 0;
											for (; i_2 < 8; i_2++) {
												$BS = i_2;
												$CK[$BS] = $CL + i_2 < $CI ? $KEY_9[$CH[$CL + i_2]] : $CL + i_2 < $CI;
											}
											$KEY_5.push($CK[0] << 7 | $CK[1] << 6 | $CK[2] << 5 | $CK[3] << 4 | $CK[4] << 3 | $CK[5] << 2 | $CK[6] << 1 | $CK[7]);
											$CL += 8;
										}
									} else {
										$KEY_5.push($KEY_374.MAP_);
										$KEY_14($CJ, $KEY_5);
										$CH.forEach($CM => $KEY_10.encodeValue($KEY_9[$CM], $KEY_5));
									}
								}
								if (this.remainingDepth !== null) {
									this.remainingDepth = Number(this.remainingDepth) + 1;
								}
							}
							return $KEY_5;
						}
						static encode() {
							let $KEY_6;
							let $KEY_3 = arguments;
							let $BQ;
							if ($KEY_3.length <= 1) {
								$BQ = true;
							} else {
								$KEY_3 = arguments;
								$BQ = $KEY_3[1] === undefined;
							}
							let $KEY_4;
							if ($BQ) {
								$KEY_4 = {};
							} else {
								$KEY_3 = arguments;
								$KEY_4 = $KEY_3[1];
							}
							let $KEY_2 = null;
							if ($KEY_4.depthBound != null) {
								let $BR = $KEY_4.depthBound;
								if ($BR < 0 || Math.floor($BR) !== $BR) {
									throw new RangeError("depthBound, if specified, must be a non-negative integer");
								}
								if ($KEY_4.extensions != null) {
									Object.keys($KEY_4.extensions).forEach($BT => {
										let $KEY_1 = Number($BT);
										if ($KEY_4.extensions[$KEY_1] === $KEY_95) {
											$KEY_2 = $KEY_1;
										}
									});
								}
								if ($KEY_2 == null) {
									throw new Error("if depthBound is used, its corresponding extension must be provided");
								}
							} else {
								let $BS;
								if ($KEY_4.extensions != null) {
									$BS = Object.keys($KEY_4.extensions).some($BT => $KEY_4.extensions[$BT] === $KEY_95);
								} else {
									$BS = false;
								}
								if ($BS) {
									throw new Error("if the depthBound extension is used, a depth bound must be specified");
								}
							}
							$KEY_6 = new $KEY_0($KEY_4.depthBound, $KEY_2);
							if ($KEY_4.extensions != null) {
								Object.keys($KEY_4.extensions).forEach($BT => {
									$KEY_6.extend($BT, $KEY_4.extensions[$BT].detector, $KEY_4.extensions[$BT].serialiser, $KEY_4.extensions[$BT].deserialiser);
								});
							}
							return $KEY_6.encode(arguments[0], $KEY_4);
						}
					}
					$KEY_0 = $THREAD_5_148898;
					return $KEY_0;
				}
let encoder=$THREAD_0_146772($BM).encode;return encoder;}main();
