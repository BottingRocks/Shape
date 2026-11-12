function bootstrap() {
let $KEY_0, $KEY_1, $KEY_11, $KEY_122, $KEY_140, $KEY_145, $KEY_147, $KEY_186, $KEY_2, $KEY_210, $KEY_212, $KEY_213, $KEY_222, $KEY_223, $KEY_229, $KEY_235, $KEY_24, $KEY_269, $KEY_299, $KEY_3, $KEY_351, $KEY_358, $KEY_6, $KEY_7, $KEY_73, $KEY_8, $KEY_80, $KEY_9;
function $THREAD_2_110707($AK, $AL, $AM) {
			let $KEY_11 = $AM;
			if ($AM === void 0) {
				$KEY_11 = false;
			}
			let $AN = $AK;
			let $AO, $AP;
			for (;;) {
				$AO = void 0;
				$AP = void 0;
				if ($AN == null) {
					$AO = null;
					$AP = null;
					break;
				}
				let $AQ = Object.getOwnPropertyDescriptor($AN, $AL);
				if ($AQ == null) {
					$AN = Object.getPrototypeOf($AN);
					continue;
				}
				$AO = {
					containingObj: $AN,
					desc: $AQ
				};
				$AP = $AO;
				break;
			}
			if ($AO == null) {
				return null;
			}
			let $AR = $AP.containingObj;
			$AP = $AP.desc;
			let $AS = $AP.configurable;
			let $AT = $AP.writable;
			let $KEY_7 = $AP.value;
			if (!$KEY_235(Object.hasOwnProperty, $AP, "value")) {
				return null;
			}
			let $AU = Object.create(null);
			$AU.value = $KEY_7;
			if ($AS === false && $AT === false || typeof $KEY_7 !== "function") {
				$AU = { originals: $AU };
				return $AU;
			} else {
				let $AV, $AW;
				if ($KEY_145[$AL] == null) {
					$AV = null;
					$AW = null;
				} else {
					let $AX = 0;
					for (;;) {
						if ($AX >= $KEY_145[$AL].length) {
							$AV = null;
							$AW = null;
							break;
						}
						$AV = $KEY_145[$AL][$AX];
						if ($AR !== $AV.object) {
							++$AX;
							continue;
						}
						$AW = $AV;
						break;
					}
				}
				if ($AV != null) {
					if ($AW.result != null) {
						return $AW.result;
					}
					$KEY_7 = $AW.original;
					$AU.value = $KEY_7;
				}
				let $KEY_0 = $KEY_24();
				let $KEY_6 = $KEY_24();
				function $THREAD_7_124281() {
					"use strict";
					let $KEY_1 = arguments;
					let $BE = $KEY_0.notify({
						args: $KEY_1,
						thisObj: this
					});
					let $BF;
					if ($BE) {
						if ($BE.bypassResult != null) {
							if ($BE.bypassResult.throw) {
								throw $BE.bypassResult.value;
							} else {
								return $BE.bypassResult.value;
							}
						} else {
							$BF = $BE.args != null ? $BE.args : $KEY_1;
						}
					} else {
						$BF = $KEY_1;
					}
					$KEY_1 = arguments;
					let $BG = {
						args: $KEY_1,
						thisObj: this,
						threw: true,
						result: null
					};
					let $BH, $BI;
					try {
						$BH = $KEY_11 && this instanceof $THREAD_7_124281 ? $KEY_229($KEY_7, $BF) : $KEY_222($KEY_7, this, $BF);
						$BG = {
							args: arguments,
							thisObj: this,
							threw: false,
							result: $BH
						};
					} finally {
						let $BJ = $KEY_6.notify($BG);
						if ($BJ && $BJ.bypassResult != null) {
							if ($BJ.bypassResult.throw) {
								throw $BJ.bypassResult.value;
							} else {
								return $BJ.bypassResult.value;
							}
						} else {
							$BI = $BH;
						}
					}
					return $BI;
				}
				$AP.value = $THREAD_7_124281;
				let $AY = $AP.value;
				if (typeof $KEY_7 === "function") {
					try {
						let $AZ = $KEY_210.toString.call($KEY_7);
						$KEY_358.push($AZ);
						$KEY_269.push($AY);
						if (Function.prototype.toString !== $KEY_210.toString) {
							Function.prototype.toString = $KEY_210.toString;
						}
					} catch (e) {}
				}
				let $BA = Object.getOwnPropertyNames($KEY_7);
				let i = 0;
				for (; i < $BA.length; i++) {
					if ($KEY_235(Array.prototype.indexOf, $KEY_147, $BA[i]) === -1) {
						let $BB = Object.getOwnPropertyDescriptor($AY, $BA[i]);
						if ($BB == null || $BB.configurable === true || $BB.writable === true) {
							let $BC = Object.getOwnPropertyDescriptor($KEY_7, $BA[i]);
							if ($BC != null) {
								if ($BB == null || $BB.configurable !== false || $BC.configurable !== true) {
									Object.defineProperty($AY, $BA[i], $BC);
								}
							}
						}
					}
				}
				try {
					if (!$KEY_235(Object.hasOwnProperty, $KEY_7, "prototype")) {
						$AY.prototype = void 0;
					}
				} catch (e_2) {}
				Object.defineProperty($AR, $AL, $AP);
				$AU = {
					onBeforeInvoke: $KEY_0,
					onAfterInvoke: $KEY_6,
					originals: $AU
				};
				$AR = {
					object: $AR,
					result: $AU,
					original: $KEY_7
				};
				let $BD = $KEY_145[$AL];
				if ($BD == null) {
					$BD = [];
					$KEY_145[$AL] = $BD;
				}
				$KEY_235(Array.prototype.push, $BD, $AR);
				return $AU;
			}
		}
function $THREAD_7_6923() {
			let $KEY_1, $KEY_2, $KEY_3;
			function $THREAD_3_39371($AK) {
				let $KEY_9 = $AK.target;
				if ($KEY_9 == null) {
					return void 0;
				}
				if ($KEY_9 != null && $KEY_9.nodeType === 1 && $KEY_9.nodeName === "FORM") {
					let $AL, $AM;
					if ($AK.event != null && !$AK.event.isTrusted) {
						$AM = $AK.event;
						$AL = "originalTarget" in $AM;
					} else {
						$AL = false;
						$AM = void 0;
					}
					let $AN;
					if ($AL) {
						$AN = $KEY_9;
					} else {
						let $AO = $KEY_9;
						for (;;) {
							if ($AO.parentNode != null) {
								$AM = $AO.parentNode;
								$AM = $AM == null || $AM.nodeType !== 1 || $AM.nodeName !== "FORM";
							} else {
								$AM = false;
							}
							if (!$AM || $AO.parentNode === $KEY_9) break;
							$AO = $AO.parentNode;
						}
						$AN = $AO;
					}
					let $KEY_8 = $AN;
					let $KEY_7;
					function $THREAD_4_72533($AP) {
						try {
							$KEY_8.removeEventListener("submit", $KEY_7);
							if ($AP != null && $AP.defaultPrevented) {
								return void 0;
							} else {
								$KEY_73.notify({
									form: $KEY_9,
									event: $AP
								});
								return void 0;
							}
						} catch (e) {
							$KEY_80.notify("2.27:" + e);
							return void 0;
						}
					}
					$KEY_7 = $THREAD_4_72533;
					if ($AK.event == null) {
						$KEY_7();
						$KEY_3 = null;
						$KEY_2 = null;
						$KEY_1 = null;
						$KEY_8.removeEventListener("submit", $KEY_7);
						return void 0;
					} else {
						function $THREAD_0_36560($AP) {
							if ($AP.type !== "submit" || $AP.target !== $KEY_9) {
								return void 0;
							} else {
								$KEY_7($AP);
								$KEY_3 = null;
								return void 0;
							}
						}
						$KEY_3 = $THREAD_0_36560;
						function $THREAD_8_99856($AP) {
							if ($AP.type !== "submit" || $AP.target !== $KEY_9) {
								return void 0;
							} else {
								$KEY_7($AP);
								$KEY_2 = null;
								return void 0;
							}
						}
						$KEY_2 = $THREAD_8_99856;
						function $THREAD_5_182($AP) {
							try {
								if (!$AP.isTrusted && !$AP.cancelable) {
									return void 0;
								}
								if ($AP.type !== "submit" || $AP.target !== $KEY_9) {
									return void 0;
								} else {
									$KEY_8.removeEventListener("submit", $KEY_7);
									$KEY_2 = null;
									$KEY_3 = null;
									$KEY_299.notify($KEY_9);
									return void 0;
								}
							} catch (e) {
								$KEY_80.notify("2.28:" + e);
								return void 0;
							}
						}
						$KEY_1 = $THREAD_5_182;
						$KEY_8.addEventListener("submit", $KEY_7);
						return void 0;
					}
				} else {
					return void 0;
				}
			}
			let $KEY_0 = $THREAD_3_39371;
			if ($KEY_223) {
				return void 0;
			} else {
				try {
					function $THREAD_7_30951($AK) {
						let $AL = { event: void 0 };
						$AL.event = $AK;
						$AL.target = $AK.target;
						return $KEY_0($AL);
					}
					$KEY_213 = $THREAD_7_30951;
					window.document.addEventListener("submit", $THREAD_7_30951, true);
				} catch (e) {
					$KEY_80.notify("2.21:" + e);
					return void 0;
				}
				if ($KEY_186 != null && $KEY_186.onBeforeInvoke != null) {
					$KEY_186.onBeforeInvoke.register(($AK, $AL) => {
						try {
							if ($AL != null && $AL.bypassResult != null) {
								return $AL;
							} else {
								$KEY_0({ target: $AK.thisObj });
								return $AL;
							}
						} catch (e) {
							$KEY_80.notify("2.22:" + e);
							return $AL;
						}
					});
				}
				if ($KEY_122 != null && $KEY_122.onBeforeInvoke != null) {
					$KEY_122.onBeforeInvoke.register(($AK, $AL) => {
						try {
							if ($AL != null && $AL.bypassResult != null) {
								return $AL;
							} else {
								if ($KEY_3 != null) {
									$KEY_3($AK.thisObj);
								}
								return $AL;
							}
						} catch (e) {
							$KEY_80.notify("2.23:" + e);
							return $AL;
						}
					});
				}
				if ($KEY_212 != null && $KEY_212.onBeforeInvoke != null) {
					$KEY_212.onBeforeInvoke.register(($AK, $AL) => {
						try {
							if ($AL != null && $AL.bypassResult != null) {
								return $AL;
							} else {
								if ($KEY_2 != null) {
									$KEY_2($AK.thisObj);
								}
								return $AL;
							}
						} catch (e) {
							$KEY_80.notify("2.24:" + e);
							return $AL;
						}
					});
				}
				if ($KEY_140 != null && $KEY_140.onBeforeInvoke != null) {
					$KEY_140.onBeforeInvoke.register(($AK, $AL) => {
						try {
							if ($AL != null && $AL.bypassResult != null) {
								return $AL;
							} else {
								if ($KEY_1 != null) {
									$KEY_1($AK.thisObj);
								}
								return $AL;
							}
						} catch (e) {
							$KEY_80.notify("2.25:" + e);
							return $AL;
						}
					});
				}
				if ($KEY_351 != null && $KEY_351.onBeforeSet != null) {
					$KEY_351.onBeforeSet.register(($AK, $AL) => {
						try {
							if ($AL != null && $AL.bypassResult != null) {
								return $AL;
							}
							if ($AK.param == null) {
								return $AL;
							} else {
								window.Event.prototype.stopPropagation.call($AK.thisObj);
								return $AL;
							}
						} catch (e) {
							$KEY_80.notify("2.26:" + e);
							return $AL;
						}
					});
				}
				$KEY_223 = true;
				return void 0;
			}
		}
class $THREAD_1_54839 {
			constructor() {
				this.callbacks = [];
			}
			register($AK) {
				this.callbacks.push($AK);
				return this;
			}
			unregister($AK) {
				let i = 0;
				for (; i < this.callbacks.length; i++) {
					if (this.callbacks[i] === $AK) {
						this.callbacks[i] = void 0;
					}
				}
				return this;
			}
			clear() {
				this.callbacks = [];
				return this;
			}
			notify($AK) {
				let i = 0;
				let $AL = null;
				for (; i < this.callbacks.length; i++) {
					if (this.callbacks[i] != null) {
						try {
							$AL = this.callbacks[i].call(void 0, $AK, $AL);
						} catch (e) {}
					}
				}
				return $AL;
			}
		}
function $THREAD_7_72452($AK) {
			class $THREAD_7_27166 extends $AK {
				constructor() {
					super(...arguments);
				}
				register($AL) {
					$KEY_215();
					super.register($AL);
					return this;
				}
			}
			return $THREAD_7_27166;
		}
let $KEY_267 = $THREAD_2_110707;
let $KEY_215 = $THREAD_7_6923;
$KEY_145 = Object.create(null);
$KEY_223 = false;
$KEY_186 = $KEY_267(window.HTMLFormElement.prototype, "submit");
let $AI = $THREAD_7_72452($THREAD_1_54839);
$KEY_73 = new $AI();
$KEY_299 = new $AI();
$KEY_80 = new $AI();
}
bootstrap();
