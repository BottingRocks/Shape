//                   Exact snapshot body in a synthetic context wrapper; never executed.
function main(){
				let $KEY_210 = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63];
				let $KEY_94, $KEY_225, $KEY_342, $KEY_368;
				let $KEY_26, $KEY_340;
				let $KEY_31, $KEY_150, $KEY_162;
				let $KEY_283;
						let $KEY_105 = $KEY_114 && $KEY_30($KEY_114.get);
				$KEY_31 = [
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{ ctor: null, priority: false },
					{
						ctor: $S,
						priority: false
					},
					{
						ctor: null,
						priority: false
					},
					{
						ctor: null,
						priority: false
					}
				];
				$KEY_283 = new $KEY_110(2);
function $THREAD_6_80458($AH, $AI, $AJ, $AK, $AL, $AM, $AN, $AO, $AP, $AQ, $AR) {
					let $KEY_9 = $AK;
					let $KEY_13;
					let $KEY_14 = $AJ;
					let $KEY_17, $KEY_21;
					let $KEY_22 = $AI;
					function $THREAD_4_1922() {
						let $AY = $KEY_13.persistentId.key;
						try {
							if ($KEY_50 != null) {
								if ($KEY_50.getItem($AY) == null) {
									let $AZ = $KEY_50.getItem("f");
									if ($AZ != null && typeof $AZ === "string") {
										let $BA = {};
										let i = 0;
										for (; i < "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=".length; i++) {
											$BA["ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_="[i]] = i;
										}
										let $BB = [];
										let $BC = 0;
										while ($BC < $AZ.length) {
											if ($BA[$AZ[$BC + 2]] === 64 && $BA[$AZ[$BC + 3]] !== 64) {
												throw new Error("The string to be decoded is not correctly padded.");
											} else {
												$BB.push($BA[$AZ[$BC]] << 2 | $BA[$AZ[$BC + 1]] >> 4);
												if ($BA[$AZ[$BC + 2]] !== 64 && $BC + 2 < $AZ.length) {
													$BB.push(($BA[$AZ[$BC + 1]] & 15) << 4 | $BA[$AZ[$BC + 2]] >> 2);
												}
												if ($BA[$AZ[$BC + 3]] !== 64 && $BC + 3 < $AZ.length) {
													$BB.push(($BA[$AZ[$BC + 2]] & 3) << 6 | $BA[$AZ[$BC + 3]]);
												}
												$BC += 4;
												continue;
											}
										}
										if ($BB[5] >= 90 && $BB[5] <= 190 && $BB[6] === 1 && $BB[7] === 0 && $BB[8] === 0) {
											$KEY_50.removeItem("f");
											try {
												$KEY_313 = false;
												if ($KEY_50) {
													if (!$KEY_50.getItem($AY)) {
														$KEY_50.setItem($AY, $AZ);
													}
													$KEY_313 = true;
												}
											} catch (e) {}
										}
									}
								}
							}
						} catch (e_2) {}
						let $BD = $KEY_13.persistentId.key;
						let $BE = $KEY_13.transactionToken;
						try {
							$KEY_313 = false;
							if ($KEY_50) {
								if (typeof $KEY_13.persistentId.resetKey !== "undefined") {
									$KEY_50.removeItem($KEY_13.persistentId.resetKey);
								}
								if (!$KEY_50.getItem($BD)) {
									$KEY_50.setItem($BD, $BE);
								}
								$KEY_313 = true;
							}
						} catch (e_3) {}
						try {
							if ($KEY_50) {
								$KEY_88 = "l_" + $KEY_13.persistentId.key;
								$KEY_5 = "" + Date.now() + $KEY_28();
								if (!$KEY_50.getItem($KEY_88)) {
									$KEY_50.setItem($KEY_88, $KEY_5);
								}
							}
						} catch (e_4) {}
						$KEY_159 = $KEY_177($KEY_13.transactionToken);
						$KEY_318();
						try {
							$KEY_17.forEach($BG => {
								$BG.init(new $KEY_268($BH => {
									let $KEY_0 = $BH;
									$KEY_162.push($BI => $KEY_0.next($BI));
								}));
								if ($BG.afterReady != null) {
									if (typeof setTimeout !== "function") {
										$BG.afterReady.call(void 0);
									} else if ($KEY_29) {
										setTimeout($BG.afterReady, 0);
									} else if ($KEY_86) {
										$KEY_86.push($BG.afterReady);
									}
								}
							});
						} catch (e_5) {
							let $BF = {};
							$BF.message = "3.1:" + e_5;
							$KEY_201($BF);
						}
					}
					let $KEY_0;
					function $THREAD_8_86926($AY) {
						const $AZ = $AY === void 0 ? {
							signalFilter: $KEY_210,
							includeDebugNames: false,
							notifyLastChance: true
						} : $AY;
						if ($AZ.notifyLastChance == null || $AZ.notifyLastChance) {
							$AZ.signalFilter.forEach($BJ => {
								if ($KEY_0[$BJ] != null) {
									$KEY_0[$BJ].call(void 0);
								}
							});
						}
						let $BA;
						if ($AZ.includeDebugNames) {
							$BA = $KEY_225.map(($BJ, $BK) => {
								return {
									value: $BJ,
									signal: $KEY_340[$BK]
								};
							});
						} else {
							$BA = $AZ.signalFilter.map($BJ => $KEY_225[$BJ]);
						}
						let $BB = [];
						let $BC = [];
						let $BD = [];
						let i = 0;
						for (; i < $BA.length; i++) {
							let $BE = $KEY_31[$AZ.signalFilter[i]];
							let $BF, $BG, $BH;
							if (Object.prototype.hasOwnProperty.call($BE, "name")) {
								$BG = (typeof $BE.name === "number" ? $BC : $BD).push;
								$BH = [$BE.name, $BA[i]];
								$BF = typeof $BE.name === "number" ? $BC : $BD;
							} else {
								$BF = $BB;
								$BG = $BB.push;
								$BH = $BA[i];
							}
							$BG.call($BF, $BH);
						}
						return {
							signals: $BD,
							positionalSignals: $BB,
							keyedSignals: $BC,
							errors: $KEY_283.toArray()
						};
					}
					function $THREAD_9_126051() {
						$KEY_314.forEach($AY => $AY());
						$KEY_238.empty();
						$KEY_283.clear();
						$KEY_17.forEach($AY => $AY.teardown());
					}
					if ($KEY_96 == null) {
						let $AS = {
							init: $KEY_72,
							getData: $KEY_72
						};
						function $THREAD_7_67229() {
						}
						$AS.teardown = $THREAD_7_67229;
						return $AS;
					} else {
						let $AT = {
							global: $KEY_96,
							native: null,
							csp: void 0,
							transactionToken: void 0,
							persistentId: void 0,
							outerArgs: void 0,
							forwardedData: void 0,
							timeElapsedSinceInit: void 0,
							instrumented: void 0,
							platformIntegratedHooksCalled: void 0,
							previousBundleTokens: void 0,
							previousTransactionTokens: void 0,
							previousBundlesTimeOnPage: void 0,
							scrBytes: void 0
						};
						let $AU = {
							scriptNonce: $AL,
							styleNonce: $AM
						};
						$AT.csp = $AU;
						$AT.transactionToken = $AH;
						$AT.persistentId = {
							key: "FTzyeq12rr",
							resetKey: "pi4RY2ilhv"
						};
						$AT.outerArgs = $AN;
						$AT.forwardedData = $AO;
						$AT.timeElapsedSinceInit = $KEY_95($KEY_96);
						$AT.instrumented = Object.create(null);
						$AT.platformIntegratedHooksCalled = false;
						$AT.previousBundleTokens = $AP;
						$AT.previousTransactionTokens = $AQ;
						$AT.previousBundlesTimeOnPage = $AR;
						$AT.scrBytes = {
							sidx: [],
							data: []
						};
						$KEY_13 = $AT;
						let $KEY_23 = $KEY_255($KEY_22, $KEY_190.map($AY => $KEY_339[$AY]), $KEY_14, $KEY_9);
						$KEY_135 = $AH;
						$KEY_105 = $THREAD_8_86926;
						$KEY_317 = $KEY_23;
						$KEY_21 = Object.create(null);
						function $THREAD_6_17828($AY) {
							$KEY_13.platformIntegratedHooksCalled = true;
							$KEY_21 = $AY.instrumented;
						}
						let $AV = { exchange: $THREAD_6_17828 };
						let $AW = 0;
						while ($AW < $KEY_328.length) {
							let $AX = $KEY_96.document.createEvent("CustomEvent");
							$AX.initCustomEvent($KEY_328[$AW], false, true, $AV);
							$KEY_96.dispatchEvent($AX);
							++$AW;
						}
						if (!$KEY_13.platformIntegratedHooksCalled) {
							$KEY_123.forEach($AY => {
								if ($AY.type === "function") {
									let $KEY_2 = $KEY_21;
									let $AZ = $KEY_25;
									let $BA = 0;
									let $BB = $KEY_96;
									let $BC;
									for (;;) {
										$BC = void 0;
										if ($BA >= $AY.location.length) {
											$BC = $BB;
											break;
										}
										if (!!Object.prototype.hasOwnProperty.call($BB, $AY.location[$BA])) {
											$BB = $BB[$AY.location[$BA]];
											++$BA;
											continue;
										}
										$BC = void 0;
										break;
									}
									$KEY_2[$AY.name] = $AZ($BC, $AY.property, !!$AY.isConstructor);
								} else if ($AY.type === "accessor") {
									let $BD = $KEY_21;
									let $BE = $KEY_4;
									let $BF = 0;
									let $BG = $KEY_96;
									let $BH, $BI;
									for (;;) {
										$BH = void 0;
										$BI = void 0;
										if ($BF >= $AY.location.length) {
											$BH = $BG;
											$BI = $AY.property;
											break;
										}
										if (!!Object.prototype.hasOwnProperty.call($BG, $AY.location[$BF])) {
											$BG = $BG[$AY.location[$BF]];
											++$BF;
											continue;
										}
										$BH = void 0;
										$BI = $AY.property;
										break;
									}
									$BD[$AY.name] = $BE($BH, $BI);
								}
							});
						}
						$KEY_13.instrumented = $KEY_21;
						$KEY_17 = Array.prototype.map.call($KEY_26, $AY => $AY($KEY_13, $KEY_22, $KEY_14, $KEY_9));
						$KEY_0 = Object.create(null);
						function $THREAD_6_91408($AY, $AZ) {
							$KEY_0[$AZ] = $AY;
						}
						$KEY_342 = $KEY_13;
						$KEY_374 = $THREAD_6_91408;
						$KEY_225 = $KEY_31.map(() => $KEY_56);
						$KEY_340 = $KEY_31.map($AY => $AY.debugName);
						$KEY_150 = $KEY_31.map($AY => {
							return {
								debugName: $AY.debugName,
								hasCollected: false,
								hasErrored: false
							};
						});
						$KEY_314 = [];
						$KEY_162 = [];
						return {
							init: $THREAD_4_1922,
							teardown: $THREAD_9_126051
						};
					}
				}
function consumeSnapshot(){
let $AP = {
											notifyLastChance: true,
											signalFilter: $KEY_210,
											descriptor: "ABaAhIDBCKGFgQGAAYIQgISigaIAwBGAzvpCzi_33wcHb9zCXKm1JQAAAABN__k2APwttohOMT9fydqFYtZXS64"
										};
										let $AQ = typeof $AP.encode === "function" ? $AP.encode : $KEY_317;
										let $AR = Object.create(null);
										let $AS = {
											signalFilter: $AP.signalFilter,
											includeDebugNames: false
										};
										$AS.notifyLastChance = $AP.notifyLastChance;
										$AQ = $AQ($KEY_105($AS));
										$AR.f = $KEY_135;
										$AR.b = $KEY_177($KEY_135 + $AQ).toString(36);
										$AR.c = "AECdLXChAQAALg_6ZXwQPyhFWnteUKYRK-XWkzqoPoQ0w8k0TQdv3MJcqbUl";
										$AR.d = $AP.descriptor;
										$AR.z = "q";
										$AR.a = $AQ;
return $AR;
}
return consumeSnapshot;
}
main();
