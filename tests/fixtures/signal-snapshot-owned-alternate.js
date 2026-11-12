//                   Exact snapshot body in a synthetic context wrapper; never executed.
function main(){
		let $KEY_261 = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76];
		let $KEY_178, $KEY_201, $KEY_220;
		let $KEY_362;
		let $KEY_0, $KEY_26, $KEY_153, $KEY_199, $KEY_254, $KEY_262, $KEY_339, $KEY_355, $KEY_374, $KEY_378, $KEY_382;
		let $KEY_117;
		let $KEY_183, $KEY_206, $KEY_306, $KEY_389;
		$KEY_153 = [
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
				ctor: $O,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
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
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: $T,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
				priority: false
			},
			{
				ctor: null,
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
		$KEY_117 = new $KEY_169(2);
function $THREAD_3_93415($BK, $BL, $BM, $BN, $BO, $BP, $BQ, $BR, $BS, $BT, $BU) {
			let $KEY_7 = $BN;
			let $KEY_10, $KEY_17;
			let $KEY_20 = $BL;
			let $KEY_23;
			let $KEY_24 = $BM;
			function $THREAD_3_111589($CB) {
				const $CC = $CB === void 0 ? {
					signalFilter: $KEY_261,
					includeDebugNames: false,
					notifyLastChance: true
				} : $CB;
				if ($CC.notifyLastChance == null || $CC.notifyLastChance) {
					$CC.signalFilter.forEach($CM => {
						if ($KEY_10[$CM] != null) {
							$KEY_10[$CM].call(void 0);
						}
					});
				}
				let $CD;
				if ($CC.includeDebugNames) {
					$CD = $KEY_220.map(($CM, $CN) => {
						return {
							value: $CM,
							signal: $KEY_362[$CN]
						};
					});
				} else {
					$CD = $CC.signalFilter.map($CM => $KEY_220[$CM]);
				}
				let $CE = [];
				let $CF = [];
				let $CG = [];
				let i = 0;
				for (; i < $CD.length; i++) {
					let $CH = $KEY_153[$CC.signalFilter[i]];
					let $CI, $CJ, $CK;
					if (Object.prototype.hasOwnProperty.call($CH, "name")) {
						$CI = typeof $CH.name === "number" ? $CE : $CG;
						$CJ = $CI.push;
						$CK = [$CH.name, $CD[i]];
					} else {
						$CJ = $CF.push;
						$CI = $CF;
						$CK = $CD[i];
					}
					$CJ.call($CI, $CK);
				}
				return {
					signals: $CG,
					positionalSignals: $CF,
					keyedSignals: $CE,
					errors: $KEY_117.toArray()
				};
			}
			let $KEY_4;
			function $THREAD_8_107082() {
				let $CB = $KEY_23.persistentId.key;
				try {
					if ($KEY_214 != null) {
						if ($KEY_214.getItem($CB) == null) {
							let $CC = $KEY_214.getItem("f");
							if ($CC != null && typeof $CC === "string") {
								let $CD = {};
								let i = 0;
								for (; i < "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=".length; i++) {
									$CD["ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_="[i]] = i;
								}
								let $CE = [];
								let $CF = 0;
								while ($CF < $CC.length) {
									if ($CD[$CC[$CF + 2]] === 64 && $CD[$CC[$CF + 3]] !== 64) {
										throw new Error("The string to be decoded is not correctly padded.");
									} else {
										$CE.push($CD[$CC[$CF]] << 2 | $CD[$CC[$CF + 1]] >> 4);
										if ($CD[$CC[$CF + 2]] !== 64 && $CF + 2 < $CC.length) {
											$CE.push(($CD[$CC[$CF + 1]] & 15) << 4 | $CD[$CC[$CF + 2]] >> 2);
										}
										if ($CD[$CC[$CF + 3]] !== 64 && $CF + 3 < $CC.length) {
											$CE.push(($CD[$CC[$CF + 2]] & 3) << 6 | $CD[$CC[$CF + 3]]);
										}
										$CF += 4;
										continue;
									}
								}
								if ($CE[5] >= 90 && $CE[5] <= 190 && $CE[6] === 1 && $CE[7] === 0 && $CE[8] === 0) {
									$KEY_214.removeItem("f");
									try {
										$KEY_175 = false;
										if ($KEY_214) {
											if (!$KEY_214.getItem($CB)) {
												$KEY_214.setItem($CB, $CC);
											}
											$KEY_175 = true;
										}
									} catch (e) {}
								}
							}
						}
					}
				} catch (e_2) {}
				let $CG = $KEY_23.persistentId.key;
				let $CH = $KEY_23.transactionToken;
				try {
					$KEY_175 = false;
					if ($KEY_214) {
						if (typeof $KEY_23.persistentId.resetKey !== "undefined") {
							$KEY_214.removeItem($KEY_23.persistentId.resetKey);
						}
						if (!$KEY_214.getItem($CG)) {
							$KEY_214.setItem($CG, $CH);
						}
						$KEY_175 = true;
					}
				} catch (e_3) {}
				try {
					if ($KEY_214) {
						$KEY_97 = "l_" + $KEY_23.persistentId.key;
						$KEY_276 = "" + Date.now() + $KEY_397();
						if (!$KEY_214.getItem($KEY_97)) {
							$KEY_214.setItem($KEY_97, $KEY_276);
						}
					}
				} catch (e_4) {}
				$KEY_241 = $KEY_269($KEY_23.transactionToken);
				$KEY_101();
				try {
					$KEY_4.forEach($CJ => {
						$CJ.init(new $KEY_14($CK => {
							let $KEY_0 = $CK;
							$KEY_392.push($CL => $KEY_0.next($CL));
						}));
						if ($CJ.afterReady != null) {
							if (typeof setTimeout !== "function") {
								$CJ.afterReady.call(void 0);
							} else if ($KEY_216) {
								setTimeout($CJ.afterReady, 0);
							} else if ($KEY_140) {
								$KEY_140.push($CJ.afterReady);
							}
						}
					});
				} catch (e_5) {
					let $CI = {};
					$CI.message = "3.1:" + e_5;
					$KEY_158($CI);
				}
			}
			function $THREAD_1_55306() {
				$KEY_374.forEach($CB => $CB());
				$KEY_199.empty();
				$KEY_117.clear();
				$KEY_4.forEach($CB => $CB.teardown());
			}
			if ($KEY_290 == null) {
				let $BV = {
					init: $KEY_364,
					getData: $KEY_364
				};
				function $THREAD_3_70411() {
				}
				$BV.teardown = $THREAD_3_70411;
				return $BV;
			} else {
				let $BW = {
					global: $KEY_290,
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
				let $BX = {
					scriptNonce: $BO,
					styleNonce: $BP
				};
				$BW.csp = $BX;
				$BW.transactionToken = $BK;
				$BW.persistentId = {
					key: "FTzyeq12rr",
					resetKey: "pi4RY2ilhv"
				};
				$BW.outerArgs = $BQ;
				$BW.forwardedData = $BR;
				$BW.timeElapsedSinceInit = $KEY_377($KEY_290);
				$BW.instrumented = Object.create(null);
				$BW.platformIntegratedHooksCalled = false;
				$BW.previousBundleTokens = $BS;
				$BW.previousTransactionTokens = $BT;
				$BW.previousBundlesTimeOnPage = $BU;
				$BW.scrBytes = {
					sidx: [],
					data: []
				};
				$KEY_23 = $BW;
				let $KEY_15 = $KEY_326($KEY_20, $KEY_131.map($CB => $KEY_63[$CB]), $KEY_24, $KEY_7);
				$KEY_306 = $BK;
				$KEY_183 = $THREAD_3_111589;
				$KEY_389 = $KEY_15;
				$KEY_17 = Object.create(null);
				function $THREAD_1_60329($CB) {
					$KEY_23.platformIntegratedHooksCalled = true;
					$KEY_17 = $CB.instrumented;
				}
				let $BY = { exchange: $THREAD_1_60329 };
				let $BZ = 0;
				while ($BZ < $KEY_164.length) {
					let $CA = $KEY_290.document.createEvent("CustomEvent");
					$CA.initCustomEvent($KEY_164[$BZ], false, true, $BY);
					$KEY_290.dispatchEvent($CA);
					++$BZ;
				}
				if (!$KEY_23.platformIntegratedHooksCalled) {
					$KEY_390.forEach($CB => {
						if ($CB.type === "function") {
							let $KEY_7 = $KEY_17;
							let $CC = $KEY_96;
							let $CD = 0;
							let $CE = $KEY_290;
							let $CF;
							for (;;) {
								$CF = void 0;
								if ($CD >= $CB.location.length) {
									$CF = $CE;
									break;
								}
								if (!!Object.prototype.hasOwnProperty.call($CE, $CB.location[$CD])) {
									$CE = $CE[$CB.location[$CD]];
									++$CD;
									continue;
								}
								$CF = void 0;
								break;
							}
							$KEY_7[$CB.name] = $CC($CF, $CB.property, !!$CB.isConstructor);
						} else if ($CB.type === "accessor") {
							let $CG = $KEY_17;
							let $CH = $KEY_65;
							let $CI = 0;
							let $CJ = $KEY_290;
							let $CK;
							for (;;) {
								$CK = void 0;
								if ($CI >= $CB.location.length) {
									$CK = $CJ;
									break;
								}
								if (!!Object.prototype.hasOwnProperty.call($CJ, $CB.location[$CI])) {
									$CJ = $CJ[$CB.location[$CI]];
									++$CI;
									continue;
								}
								$CK = void 0;
								break;
							}
							$CG[$CB.name] = $CH($CK, $CB.property);
						}
					});
				}
				$KEY_23.instrumented = $KEY_17;
				$KEY_4 = Array.prototype.map.call($KEY_247, $CB => $CB($KEY_23, $KEY_20, $KEY_24, $KEY_7));
				$KEY_10 = Object.create(null);
				function $THREAD_9_46126($CB, $CC) {
					$KEY_10[$CC] = $CB;
				}
				$KEY_355 = $KEY_23;
				$KEY_262 = $THREAD_9_46126;
				$KEY_220 = $KEY_153.map(() => $KEY_29);
				$KEY_362 = $KEY_153.map($CB => $CB.debugName);
				$KEY_178 = $KEY_153.map($CB => {
					return {
						debugName: $CB.debugName,
						hasCollected: false,
						hasErrored: false
					};
				});
				$KEY_374 = [];
				$KEY_392 = [];
				return {
					init: $THREAD_8_107082,
					teardown: $THREAD_1_55306
				};
			}
		}
function consumeSnapshot(){
let $BX = {
										notifyLastChance: true,
										signalFilter: $KEY_261,
										descriptor: "ADaAhIDBCKGBgQGAAYIQgISigaIAwBGAzPpCxg_33ocx3sD_CACK2jzbwghFXv____-9phlpAzyL9TAY_xiK3MAqaxby8OA"
									};
									let $BY = typeof $BX.encode === "function" ? $BX.encode : $KEY_389;
									let $BZ = Object.create(null);
									let $CA = {
										signalFilter: $BX.signalFilter,
										includeDebugNames: false
									};
									$CA.notifyLastChance = $BX.notifyLastChance;
									$BY = $BY($KEY_183($CA));
									$BZ.f = $KEY_306;
									$BZ.b = $KEY_269($KEY_306 + $BY).toString(36);
									$BZ.c = "AMDQkIahAQAA8dri1o18rdk410bMDnNf5jq6Ms86HFstDnks9YraPNvCCEVe";
									$BZ.d = $BX.descriptor;
									$BZ.z = "q";
									$BZ.a = $BY;
return $BZ;
}
return consumeSnapshot;
}
main();
