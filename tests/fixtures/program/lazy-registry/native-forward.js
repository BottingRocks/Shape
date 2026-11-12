function bootstrap() {
				let $KEY_11, $KEY_23, $KEY_25, $KEY_31, $KEY_97, $KEY_115, $KEY_153, $KEY_180, $KEY_275, $KEY_298, $KEY_301, $KEY_364;
					$KEY_115 = false;
				let $KEY_41, $KEY_67, $KEY_123, $KEY_199, $KEY_303, $KEY_342;
				$KEY_199 = Object.create(null);
function $THREAD_5_62505($AH, $AI, $AJ) {
					let $KEY_8 = $AJ;
					let $KEY_14, $KEY_15;
					if ($AJ === void 0) {
						$KEY_8 = false;
					}
					let $AK = $AH;
					let $AL;
					for (;;) {
						$AL = void 0;
						if ($AK == null) {
							$AL = null;
							break;
						}
						let $AM = Object.getOwnPropertyDescriptor($AK, $AI);
						if ($AM == null) {
							$AK = Object.getPrototypeOf($AK);
							continue;
						}
						$AL = {
							containingObj: $AK,
							desc: $AM
						};
						break;
					}
					if ($AL == null) {
						return null;
					}
					let $AN = $AL.containingObj;
					$AL = $AL.desc;
					let $AO = $AL.configurable;
					let $AP = $AL.writable;
					$KEY_14 = $AL.value;
					if (!$KEY_123(Object.hasOwnProperty, $AL, "value")) {
						return null;
					}
					let $AQ = Object.create(null);
					$AQ.value = $KEY_14;
					if ($AO === false && $AP === false || typeof $KEY_14 !== "function") {
						$AQ = { originals: $AQ };
						return $AQ;
					} else {
						let $AR;
						if ($KEY_199[$AI] == null) {
							$AR = null;
						} else {
							let $AS = 0;
							for (;;) {
								if ($AS >= $KEY_199[$AI].length) {
									$AR = null;
									break;
								}
								$AR = $KEY_199[$AI][$AS];
								if ($AN !== $AR.object) {
									++$AS;
									continue;
								}
								break;
							}
						}
						if ($AR != null) {
							if ($AR.result != null) {
								return $AR.result;
							}
							$KEY_14 = $AR.original;
							$AQ.value = $KEY_14;
						}
						let $KEY_7 = $KEY_41();
						$KEY_15 = $KEY_41();
						function $THREAD_8_36359() {
							"use strict";
							let $KEY_1 = arguments;
							let $AZ = $KEY_7.notify({
								args: $KEY_1,
								thisObj: this
							});
							let $BA;
							if ($AZ) {
								if ($AZ.bypassResult != null) {
									if ($AZ.bypassResult.throw) {
										throw $AZ.bypassResult.value;
									} else {
										return $AZ.bypassResult.value;
									}
								} else {
									$BA = $AZ.args != null ? $AZ.args : $KEY_1;
								}
							} else {
								$BA = $KEY_1;
							}
							$KEY_1 = arguments;
							let $BB = {
								args: $KEY_1,
								thisObj: this,
								threw: true,
								result: null
							};
							let $BC, $BD;
							try {
								$BC = $KEY_8 && this instanceof $THREAD_8_36359 ? $KEY_170($KEY_14, $BA) : $KEY_158($KEY_14, this, $BA);
								$BB = {
									args: arguments,
									thisObj: this,
									threw: false,
									result: $BC
								};
							} finally {
								let $BE = $KEY_15.notify($BB);
								if ($BE && $BE.bypassResult != null) {
									if ($BE.bypassResult.throw) {
										throw $BE.bypassResult.value;
									} else {
										return $BE.bypassResult.value;
									}
								} else {
									$BD = $BC;
								}
							}
							return $BD;
						}
						$AL.value = $THREAD_8_36359;
						let $AT = $AL.value;
						if (typeof $KEY_14 === "function") {
							try {
								let $AU = $KEY_67.toString.call($KEY_14);
								$KEY_303.push($AU);
								$KEY_342.push($AT);
								if (Function.prototype.toString !== $KEY_67.toString) {
									Function.prototype.toString = $KEY_67.toString;
								}
							} catch (e) {}
						}
						let $AV = Object.getOwnPropertyNames($KEY_14);
						let i = 0;
						for (; i < $AV.length; i++) {
							if ($KEY_123(Array.prototype.indexOf, $KEY_73, $AV[i]) === -1) {
								let $AW = Object.getOwnPropertyDescriptor($AT, $AV[i]);
								if ($AW == null || $AW.configurable === true || $AW.writable === true) {
									let $AX = Object.getOwnPropertyDescriptor($KEY_14, $AV[i]);
									if ($AX != null) {
										if ($AW == null || $AW.configurable !== false || $AX.configurable !== true) {
											Object.defineProperty($AT, $AV[i], $AX);
										}
									}
								}
							}
						}
						try {
							if (!$KEY_123(Object.hasOwnProperty, $KEY_14, "prototype")) {
								$AT.prototype = void 0;
							}
						} catch (e_2) {}
						Object.defineProperty($AN, $AI, $AL);
						$AQ = {
							onBeforeInvoke: $KEY_7,
							onAfterInvoke: $KEY_15,
							originals: $AQ
						};
						$AN = {
							object: $AN,
							result: $AQ,
							original: $KEY_14
						};
						let $AY = $KEY_199[$AI];
						if ($AY == null) {
							$AY = [];
							$KEY_199[$AI] = $AY;
						}
						$KEY_123(Array.prototype.push, $AY, $AN);
						return $AQ;
					}
				}
let $KEY_178 = $THREAD_5_62505;
function $THREAD_9_125208() {
					let $KEY_0, $KEY_1, $KEY_3;
					function $THREAD_6_63399($AH) {
						let $KEY_8 = $AH.target;
						if ($KEY_8 == null) {
							return void 0;
						}
						if ($KEY_8 != null && $KEY_8.nodeType === 1 && $KEY_8.nodeName === "FORM") {
							let $AI;
							if ($AH.event != null && !$AH.event.isTrusted && "originalTarget" in $AH.event) {
								$AI = $KEY_8;
							} else {
								let $AJ = $KEY_8;
								let $AK;
								for (;;) {
									if ($AJ.parentNode != null) {
										$AK = $AJ.parentNode;
										$AK = $AK == null || $AK.nodeType !== 1 || $AK.nodeName !== "FORM";
									} else {
										$AK = false;
									}
									if (!$AK || $AJ.parentNode === $KEY_8) break;
									$AJ = $AJ.parentNode;
								}
								$AI = $AJ;
							}
							let $KEY_7 = $AI;
							let $KEY_6;
							function $THREAD_8_118371($AL) {
								try {
									$KEY_7.removeEventListener("submit", $KEY_6);
									if ($AL != null && $AL.defaultPrevented) {
										return void 0;
									} else {
										$KEY_31.notify({
											form: $KEY_8,
											event: $AL
										});
										return void 0;
									}
								} catch (e) {
									$KEY_298.notify("2.27:" + e);
									return void 0;
								}
							}
							$KEY_6 = $THREAD_8_118371;
							if ($AH.event == null) {
								$KEY_6();
								$KEY_0 = null;
								$KEY_3 = null;
								$KEY_1 = null;
								$KEY_7.removeEventListener("submit", $KEY_6);
								return void 0;
							} else {
								function $THREAD_0_117030($AL) {
									if ($AL.type !== "submit" || $AL.target !== $KEY_8) {
										return void 0;
									} else {
										$KEY_6($AL);
										$KEY_0 = null;
										return void 0;
									}
								}
								$KEY_0 = $THREAD_0_117030;
								function $THREAD_4_72650($AL) {
									if ($AL.type !== "submit" || $AL.target !== $KEY_8) {
										return void 0;
									} else {
										$KEY_6($AL);
										$KEY_3 = null;
										return void 0;
									}
								}
								$KEY_3 = $THREAD_4_72650;
								function $THREAD_0_127813($AL) {
									try {
										if (!$AL.isTrusted && !$AL.cancelable) {
											return void 0;
										}
										if ($AL.type !== "submit" || $AL.target !== $KEY_8) {
											return void 0;
										} else {
											$KEY_7.removeEventListener("submit", $KEY_6);
											$KEY_3 = null;
											$KEY_0 = null;
											$KEY_11.notify($KEY_8);
											return void 0;
										}
									} catch (e) {
										$KEY_298.notify("2.28:" + e);
										return void 0;
									}
								}
								$KEY_1 = $THREAD_0_127813;
								$KEY_7.addEventListener("submit", $KEY_6);
								return void 0;
							}
						} else {
							return void 0;
						}
					}
					let $KEY_2 = $THREAD_6_63399;
					if ($KEY_115) {
						return void 0;
					} else {
						try {
							function $THREAD_2_63812($AH) {
								let $AI = {
									event: void 0,
									target: void 0
								};
								$AI.event = $AH;
								$AI.target = $AH.target;
								return $KEY_2($AI);
							}
							$KEY_23 = $THREAD_2_63812;
							$KEY_153.addEventListener("submit", $THREAD_2_63812, true);
						} catch (e) {
							$KEY_298.notify("2.21:" + e);
							return void 0;
						}
						if ($KEY_301 != null && $KEY_301.onBeforeInvoke != null) {
							$KEY_301.onBeforeInvoke.register(($AH, $AI) => {
								try {
									if ($AI != null && $AI.bypassResult != null) {
										return $AI;
									} else {
										$KEY_2({ target: $AH.thisObj });
										return $AI;
									}
								} catch (e) {
									$KEY_298.notify("2.22:" + e);
									return $AI;
								}
							});
						}
						if ($KEY_97 != null && $KEY_97.onBeforeInvoke != null) {
							$KEY_97.onBeforeInvoke.register(($AH, $AI) => {
								try {
									if ($AI != null && $AI.bypassResult != null) {
										return $AI;
									} else {
										if ($KEY_0 != null) {
											$KEY_0($AH.thisObj);
										}
										return $AI;
									}
								} catch (e) {
									$KEY_298.notify("2.23:" + e);
									return $AI;
								}
							});
						}
						if ($KEY_180 != null && $KEY_180.onBeforeInvoke != null) {
							$KEY_180.onBeforeInvoke.register(($AH, $AI) => {
								try {
									if ($AI != null && $AI.bypassResult != null) {
										return $AI;
									} else {
										if ($KEY_3 != null) {
											$KEY_3($AH.thisObj);
										}
										return $AI;
									}
								} catch (e) {
									$KEY_298.notify("2.24:" + e);
									return $AI;
								}
							});
						}
						if ($KEY_275 != null && $KEY_275.onBeforeInvoke != null) {
							$KEY_275.onBeforeInvoke.register(($AH, $AI) => {
								try {
									if ($AI != null && $AI.bypassResult != null) {
										return $AI;
									} else {
										if ($KEY_1 != null) {
											$KEY_1($AH.thisObj);
										}
										return $AI;
									}
								} catch (e) {
									$KEY_298.notify("2.25:" + e);
									return $AI;
								}
							});
						}
						if ($KEY_364 != null && $KEY_364.onBeforeSet != null) {
							$KEY_364.onBeforeSet.register(($AH, $AI) => {
								try {
									if ($AI != null && $AI.bypassResult != null) {
										return $AI;
									}
									if ($AH.param == null) {
										return $AI;
									} else {
										$KEY_25.Event.prototype.stopPropagation.call($AH.thisObj);
										return $AI;
									}
								} catch (e) {
									$KEY_298.notify("2.26:" + e);
									return $AI;
								}
							});
						}
						$KEY_115 = true;
						return void 0;
					}
				}
let $KEY_218 = $THREAD_9_125208;
class $THREAD_3_37776 {
						constructor() {
							this.callbacks = [];
						}
						register($AH) {
							this.callbacks.push($AH);
							return this;
						}
						unregister($AH) {
							let i = 0;
							for (; i < this.callbacks.length; i++) {
								if (this.callbacks[i] === $AH) {
									this.callbacks[i] = void 0;
								}
							}
							return this;
						}
						clear() {
							this.callbacks = [];
							return this;
						}
						notify($AH) {
							let i = 0;
							let $AI = null;
							for (; i < this.callbacks.length; i++) {
								if (this.callbacks[i] != null) {
									try {
										$AI = this.callbacks[i].call(void 0, $AH, $AI);
									} catch (e) {}
								}
							}
							return $AI;
						}
					}

$KEY_301 = $KEY_178($KEY_25.HTMLFormElement.prototype, "submit");
function $THREAD_2_95998($AH) {
						class $THREAD_5_72622 extends $AH {
							constructor() {
								super(...arguments);
							}
							register($AI) {
								$KEY_218();
								super.register($AI);
								return this;
							}
						}
						return $THREAD_5_72622;
					}
let $AF = $THREAD_2_95998($THREAD_3_37776);
					$KEY_31 = new $AF();
					$KEY_11 = new $AF();
					$KEY_298 = new $AF();

}
bootstrap();
