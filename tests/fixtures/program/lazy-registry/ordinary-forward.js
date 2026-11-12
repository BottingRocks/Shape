function bootstrap() {
				let $KEY_62, $KEY_166, $KEY_172, $KEY_173, $KEY_198, $KEY_212, $KEY_243, $KEY_344, $KEY_359, $KEY_366;
				$KEY_172 = false;
				let $KEY_18, $KEY_85, $KEY_93, $KEY_120, $KEY_174, $KEY_275;
				$KEY_120 = Object.create(null);
function $THREAD_7_64770($AH, $AI, $AJ) {
					let $KEY_17 = $AJ;
					let $KEY_19;
					if ($AJ === void 0) {
						$KEY_17 = false;
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
					let $KEY_10 = $AL.value;
					if (!$KEY_275(Object.hasOwnProperty, $AL, "value")) {
						return null;
					}
					let $AQ = Object.create(null);
					$AQ.value = $KEY_10;
					if ($AO === false && $AP === false || typeof $KEY_10 !== "function") {
						return { originals: $AQ };
					} else {
						let $AR;
						if ($KEY_120[$AI] == null) {
							$AR = null;
						} else {
							let $AS = 0;
							for (;;) {
								if ($AS >= $KEY_120[$AI].length) {
									$AR = null;
									break;
								}
								$AR = $KEY_120[$AI][$AS];
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
							$KEY_10 = $AR.original;
							$AQ.value = $KEY_10;
						}
						let $KEY_7 = $KEY_93();
						$KEY_19 = $KEY_93();
						function $THREAD_2_17235() {
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
								$BC = $KEY_17 && this instanceof $THREAD_2_17235 ? $KEY_154($KEY_10, $BA) : $KEY_332($KEY_10, this, $BA);
								$BB = {
									args: arguments,
									thisObj: this,
									threw: false,
									result: $BC
								};
							} finally {
								let $BE = $KEY_19.notify($BB);
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
						$AL.value = $THREAD_2_17235;
						let $AT = $AL.value;
						if (typeof $KEY_10 === "function") {
							try {
								let $AU = $KEY_18.toString.call($KEY_10);
								$KEY_85.push($AU);
								$KEY_174.push($AT);
								if (Function.prototype.toString !== $KEY_18.toString) {
									Function.prototype.toString = $KEY_18.toString;
								}
							} catch (e) {}
						}
						let $AV = Object.getOwnPropertyNames($KEY_10);
						let i = 0;
						for (; i < $AV.length; i++) {
							if ($KEY_275(Array.prototype.indexOf, $KEY_241, $AV[i]) === -1) {
								let $AW = Object.getOwnPropertyDescriptor($AT, $AV[i]);
								if ($AW == null || $AW.configurable === true || $AW.writable === true) {
									let $AX = Object.getOwnPropertyDescriptor($KEY_10, $AV[i]);
									if ($AX != null) {
										if ($AW == null || $AW.configurable !== false || $AX.configurable !== true) {
											Object.defineProperty($AT, $AV[i], $AX);
										}
									}
								}
							}
						}
						try {
							if (!$KEY_275(Object.hasOwnProperty, $KEY_10, "prototype")) {
								$AT.prototype = void 0;
							}
						} catch (e_2) {}
						Object.defineProperty($AN, $AI, $AL);
						$AQ = {
							onBeforeInvoke: $KEY_7,
							onAfterInvoke: $KEY_19,
							originals: $AQ
						};
						$AN = {
							object: $AN,
							result: $AQ,
							original: $KEY_10
						};
						let $AY = $KEY_120[$AI];
						if ($AY == null) {
							$AY = [];
							$KEY_120[$AI] = $AY;
						}
						$KEY_275(Array.prototype.push, $AY, $AN);
						return $AQ;
					}
				}
let $KEY_25 = $THREAD_7_64770;
function $THREAD_9_23667() {
					let $KEY_0, $KEY_1, $KEY_6;
					function $THREAD_9_29430($AH) {
						let $KEY_4 = $AH.target;
						if ($KEY_4 == null) {
							return void 0;
						}
						if ($KEY_4 != null && $KEY_4.nodeType === 1 && $KEY_4.nodeName === "FORM") {
							let $AI;
							if ($AH.event != null && !$AH.event.isTrusted && "originalTarget" in $AH.event) {
								$AI = $KEY_4;
							} else {
								let $AJ = $KEY_4;
								let $AK = $KEY_4;
								for (;;) {
									if ($AJ.parentNode != null) {
										$AK = $AJ.parentNode;
										$AK = $AK == null || $AK.nodeType !== 1 || $AK.nodeName !== "FORM";
									} else {
										$AK = false;
									}
									if (!$AK || $AJ.parentNode === $KEY_4) break;
									$AJ = $AJ.parentNode;
								}
								$AI = $AJ;
							}
							let $KEY_5 = $AI;
							let $KEY_7;
							function $THREAD_6_39068($AL) {
								try {
									$KEY_5.removeEventListener("submit", $KEY_7);
									if ($AL != null && $AL.defaultPrevented) {
										return void 0;
									} else {
										$KEY_212.notify({
											form: $KEY_4,
											event: $AL
										});
										return void 0;
									}
								} catch (e) {
									$KEY_62.notify("2.27:" + e);
									return void 0;
								}
							}
							$KEY_7 = $THREAD_6_39068;
							if ($AH.event == null) {
								$KEY_7();
								$KEY_0 = null;
								$KEY_1 = null;
								$KEY_6 = null;
								$KEY_5.removeEventListener("submit", $KEY_7);
								return void 0;
							} else {
								function $THREAD_9_124192($AL) {
									if ($AL.type !== "submit" || $AL.target !== $KEY_4) {
										return void 0;
									} else {
										$KEY_7($AL);
										$KEY_1 = null;
										return void 0;
									}
								}
								function $THREAD_1_16008($AL) {
									if ($AL.type !== "submit" || $AL.target !== $KEY_4) {
										return void 0;
									} else {
										$KEY_7($AL);
										$KEY_0 = null;
										return void 0;
									}
								}
								$KEY_0 = $THREAD_1_16008;
								$KEY_1 = $THREAD_9_124192;
								function $THREAD_5_47115($AL) {
									try {
										if (!$AL.isTrusted && !$AL.cancelable) {
											return void 0;
										}
										if ($AL.type !== "submit" || $AL.target !== $KEY_4) {
											return void 0;
										} else {
											$KEY_5.removeEventListener("submit", $KEY_7);
											$KEY_1 = null;
											$KEY_0 = null;
											$KEY_198.notify($KEY_4);
											return void 0;
										}
									} catch (e) {
										$KEY_62.notify("2.28:" + e);
										return void 0;
									}
								}
								$KEY_6 = $THREAD_5_47115;
								$KEY_5.addEventListener("submit", $KEY_7);
								return void 0;
							}
						} else {
							return void 0;
						}
					}
					let $KEY_5 = $THREAD_9_29430;
					if ($KEY_172) {
						return void 0;
					} else {
						try {
							function $THREAD_6_126216($AH) {
								let $AI = {
									event: void 0,
									target: void 0
								};
								$AI.event = $AH;
								$AI.target = $AH.target;
								return $KEY_5($AI);
							}
							$KEY_344 = $THREAD_6_126216;
							window.document.addEventListener("submit", $THREAD_6_126216, true);
						} catch (e) {
							$KEY_62.notify("2.21:" + e);
							return void 0;
						}
						if ($KEY_166 != null && $KEY_166.onBeforeInvoke != null) {
							$KEY_166.onBeforeInvoke.register(($AH, $AI) => {
								try {
									if ($AI != null && $AI.bypassResult != null) {
										return $AI;
									} else {
										$KEY_5({ target: $AH.thisObj });
										return $AI;
									}
								} catch (e) {
									$KEY_62.notify("2.22:" + e);
									return $AI;
								}
							});
						}
						if ($KEY_366 != null && $KEY_366.onBeforeInvoke != null) {
							$KEY_366.onBeforeInvoke.register(($AH, $AI) => {
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
									$KEY_62.notify("2.23:" + e);
									return $AI;
								}
							});
						}
						if ($KEY_173 != null && $KEY_173.onBeforeInvoke != null) {
							$KEY_173.onBeforeInvoke.register(($AH, $AI) => {
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
									$KEY_62.notify("2.24:" + e);
									return $AI;
								}
							});
						}
						if ($KEY_359 != null && $KEY_359.onBeforeInvoke != null) {
							$KEY_359.onBeforeInvoke.register(($AH, $AI) => {
								try {
									if ($AI != null && $AI.bypassResult != null) {
										return $AI;
									} else {
										if ($KEY_6 != null) {
											$KEY_6($AH.thisObj);
										}
										return $AI;
									}
								} catch (e) {
									$KEY_62.notify("2.25:" + e);
									return $AI;
								}
							});
						}
						if ($KEY_243 != null && $KEY_243.onBeforeSet != null) {
							$KEY_243.onBeforeSet.register(($AH, $AI) => {
								try {
									if ($AI != null && $AI.bypassResult != null) {
										return $AI;
									}
									if ($AH.param == null) {
										return $AI;
									} else {
										window.Event.prototype.stopPropagation.call($AH.thisObj);
										return $AI;
									}
								} catch (e) {
									$KEY_62.notify("2.26:" + e);
									return $AI;
								}
							});
						}
						$KEY_172 = true;
						return void 0;
					}
				}
let $KEY_279 = $THREAD_9_23667;
function $THREAD_5_68382() {
					"use strict";
					this.callbacks = [];
				}
function $THREAD_2_112022($AH) {
					"use strict";
					this.callbacks.push($AH);
					return this;
				}
				$THREAD_5_68382.prototype.register = $THREAD_2_112022;
				function $THREAD_1_86713($AH) {
					"use strict";
					let i = 0;
					for (; i < this.callbacks.length; i++) {
						if (this.callbacks[i] === $AH) {
							this.callbacks[i] = void 0;
						}
					}
					return this;
				}
				function $THREAD_8_124288() {
					"use strict";
					this.callbacks = [];
					return this;
				}
				$THREAD_5_68382.prototype.unregister = $THREAD_1_86713;
				$THREAD_5_68382.prototype.clear = $THREAD_8_124288;
				function $THREAD_5_11594($AH) {
					"use strict";
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
				$THREAD_5_68382.prototype.notify = $THREAD_5_11594;
$KEY_166 = $KEY_25(window.HTMLFormElement.prototype, "submit");
function $THREAD_6_103742($AH) {
					class $THREAD_4_126638 extends $AH {
						constructor() {
							super(...arguments);
						}
						register($AI) {
							$KEY_279();
							super.register($AI);
							return this;
						}
					}
					return $THREAD_4_126638;
				}
let $AF = $THREAD_6_103742($THREAD_5_68382);
				$KEY_212 = new $AF();
				$KEY_198 = new $AF();
				$KEY_62 = new $AF();

}
bootstrap();
