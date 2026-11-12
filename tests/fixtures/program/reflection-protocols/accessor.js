function main(){
function $THREAD_1_104589($AF, $AG) {
					let $AH = $AF;
					let $AI;
					for (;;) {
						$AI = void 0;
						if ($AH == null) {
							$AI = null;
							break;
						}
						let $AJ = Object.getOwnPropertyDescriptor($AH, $AG);
						if ($AJ == null) {
							$AH = Object.getPrototypeOf($AH);
							continue;
						}
						$AI = {
							containingObj: $AH,
							desc: $AJ
						};
						break;
					}
					if ($AI == null) {
						return null;
					}
					let $AK = $AI.containingObj;
					$AI = $AI.desc;
					let $AL = $AI.configurable;
					let $KEY_13 = $AI.get;
					let $KEY_7 = $AI.set;
					let $AM = $KEY_194(Object.hasOwnProperty, $AI, "value");
					let $AN = Object.create(null);
					if ($AL === false || $AM) {
						if ($KEY_13 != null) {
							$AN.get = $KEY_13;
						}
						if ($KEY_7 != null) {
							$AN.set = $KEY_7;
						}
						$AN = { originals: $AN };
						return $AN;
					} else {
						let $AO, $AP;
						if ($KEY_133[$AG] == null) {
							$AO = null;
							$AP = null;
						} else {
							let $AQ = 0;
							for (;;) {
								if ($AQ >= $KEY_133[$AG].length) {
									$AO = null;
									$AP = null;
									break;
								}
								$AO = $KEY_133[$AG][$AQ];
								if ($AK !== $AO.object) {
									++$AQ;
									continue;
								}
								$AP = $AO;
								break;
							}
						}
						if ($AO != null) {
							if ($AP.result != null) {
								return $AP.result;
							}
							$KEY_13 = $AP.original.get;
							$KEY_7 = $AP.original.set;
						}
						let $AR = {
							onAfterGet: void 0,
							onBeforeGet: void 0,
							onAfterSet: void 0,
							onBeforeSet: void 0,
							originals: {}
						};
						if ($KEY_13 != null) {
							$AN.get = $KEY_13;
							let $KEY_11 = $KEY_204();
							let $KEY_14 = $KEY_204();
							function $THREAD_8_63239() {
								"use strict";
								let $AW = $KEY_11.notify({ thisObj: this });
								if (!$AW || $AW.bypassResult == null) {
									let $AY = {
										thisObj: this,
										result: null,
										threw: true
									};
									let $AZ, $BA;
									try {
										$AZ = $KEY_194($KEY_13, this);
										$AY = {
											thisObj: this,
											result: $AZ,
											threw: false
										};
									} finally {
										let $BC = $KEY_14.notify($AY);
										if ($BC && $BC.bypassResult != null) {
											if ($BC.bypassResult.throw) {
												throw $BC.bypassResult.value;
											} else {
												return $BC.bypassResult.value;
											}
										} else {
											$BA = $AZ;
										}
									}
									return $BA;
								}
								if ($AW.bypassResult.throw) {
									throw $AW.bypassResult.value;
								} else {
									return $AW.bypassResult.value;
								}
							}
							$AI.get = $THREAD_8_63239;
							if (typeof $KEY_13 === "function") {
								try {
									let $AS = $KEY_118.toString.call($KEY_13);
									$KEY_142.push($AS);
									$KEY_112.push($AI.get);
									if (Function.prototype.toString !== $KEY_118.toString) {
										Function.prototype.toString = $KEY_118.toString;
									}
								} catch (e) {}
							}
							try {
								if (!$KEY_194(Object.hasOwnProperty, $KEY_13, "prototype")) {
									$AI.get.prototype = void 0;
								}
							} catch (e_2) {}
							$AR.onBeforeGet = $KEY_11;
							$AR.onAfterGet = $KEY_14;
						}
						if ($KEY_7 != null) {
							$AN.set = $KEY_7;
							let $KEY_16 = $KEY_204();
							let $KEY_15 = $KEY_204();
							function $THREAD_8_21725($AV) {
								"use strict";
								let $AX = $KEY_16.notify({
									param: $AV,
									thisObj: this
								});
								if ($AX) {
									if ($AX.bypassResult != null) {
										if ($AX.bypassResult.throw) {
											throw $AX.bypassResult.value;
										} else {
											return $AX.bypassResult.value;
										}
									} else {
										$AX = $KEY_194(Object.hasOwnProperty, $AX, "param") ? $AX.param : $AV;
									}
								} else {
									$AX = $AV;
								}
								let $AY = {
									param: $AV,
									thisObj: this,
									result: null,
									threw: true
								};
								let $AZ;
								let $BA = $AY;
								let $BB;
								try {
									$AZ = $KEY_194($KEY_7, this, $AX);
									$BA = {
										param: $AV,
										thisObj: this,
										result: $AZ,
										threw: false
									};
								} finally {
									let $BD = $KEY_15.notify($BA);
									if ($BD && $BD.bypassResult != null) {
										if ($BD.bypassResult.throw) {
											throw $BD.bypassResult.value;
										} else {
											return $BD.bypassResult.value;
										}
									} else {
										$BB = $AZ;
									}
								}
								return $BB;
							}
							$AI.set = $THREAD_8_21725;
							if (typeof $KEY_7 === "function") {
								try {
									let $AT = $KEY_118.toString.call($KEY_7);
									$KEY_142.push($AT);
									$KEY_112.push($AI.set);
									if (Function.prototype.toString !== $KEY_118.toString) {
										Function.prototype.toString = $KEY_118.toString;
									}
								} catch (e_3) {}
							}
							try {
								if (!$KEY_194(Object.hasOwnProperty, $KEY_7, "prototype")) {
									$AI.set.prototype = void 0;
								}
							} catch (e_4) {}
							$AR.onBeforeSet = $KEY_16;
							$AR.onAfterSet = $KEY_15;
						}
						Object.defineProperty($AK, $AG, $AI);
						$AR.originals = $AN;
						let $AU = $KEY_133[$AG];
						if ($AU == null) {
							$AU = [];
							$KEY_133[$AG] = $AU;
						}
						$KEY_194(Array.prototype.push, $AU, {
							object: $AK,
							result: $AR,
							original: $AN
						});
						return $AR;
					}
				}
return $THREAD_1_104589;
}
main();
