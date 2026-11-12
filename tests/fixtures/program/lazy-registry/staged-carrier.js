function bootstrap() {
		let $KEY_0, $KEY_20, $KEY_32, $KEY_96, $KEY_100, $KEY_171, $KEY_253, $KEY_330, $KEY_339, $KEY_350;
		$KEY_330 = false;
		let $KEY_23, $KEY_102, $KEY_144, $KEY_209, $KEY_316, $KEY_327, $KEY_363, $KEY_421;
		$KEY_327 = Object.create(null);
		function $THREAD_1_125747($BM, $BN, $BO) {
			let $KEY_14 = $BO;
			if ($BO === void 0) {
				$KEY_14 = false;
			}
			let $BP = $KEY_421($BM, $BN);
			if ($BP == null) {
				return null;
			}
			let $BQ = $BP.containingObj;
			$BP = $BP.desc;
			let $BR = $BP.configurable;
			let $BS = $BP.writable;
			let $KEY_3 = $BP.value;
			if (!$KEY_144(Object.hasOwnProperty, $BP, "value")) {
				return null;
			}
			let $BT = Object.create(null);
			$BT.value = $KEY_3;
			if ($BR === false && $BS === false || typeof $KEY_3 !== "function") {
				$BT = { originals: $BT };
				return $BT;
			} else {
				let $BU = $KEY_209($BQ, $BN);
				if ($BU != null) {
					if ($BU.result != null) {
						return $BU.result;
					}
					$KEY_3 = $BU.original;
					$BT.value = $KEY_3;
				}
				let $KEY_2 = $KEY_363();
				let $KEY_11 = $KEY_363();
				function $THREAD_3_123797() {
					"use strict";
					let $KEY_6 = arguments;
					let $CB = $KEY_2.notify({
						args: $KEY_6,
						thisObj: this
					});
					let $CC;
					if ($CB) {
						if ($CB.bypassResult != null) {
							if ($CB.bypassResult.throw) {
								throw $CB.bypassResult.value;
							} else {
								return $CB.bypassResult.value;
							}
						} else {
							$CC = $CB.args != null ? $CB.args : $KEY_6;
						}
					} else {
						$CC = $KEY_6;
					}
					$KEY_6 = arguments;
					let $CD;
					let $CE = {
						args: $KEY_6,
						thisObj: this,
						threw: true,
						result: null
					};
					let $CF;
					try {
						$CD = $KEY_14 && this instanceof $THREAD_3_123797 ? $KEY_360($KEY_3, $CC) : $KEY_250($KEY_3, this, $CC);
						$CE = {
							args: arguments,
							thisObj: this,
							threw: false,
							result: $CD
						};
					} finally {
						let $CG = $KEY_11.notify($CE);
						if ($CG && $CG.bypassResult != null) {
							if ($CG.bypassResult.throw) {
								throw $CG.bypassResult.value;
							} else {
								return $CG.bypassResult.value;
							}
						} else {
							$CF = $CD;
						}
					}
					return $CF;
				}
				$BP.value = $THREAD_3_123797;
				let $BV = $BP.value;
				if (typeof $KEY_3 === "function") {
					try {
						let $BW = $KEY_316.toString.call($KEY_3);
						$KEY_23.push($BW);
						$KEY_102.push($BV);
						if (Function.prototype.toString !== $KEY_316.toString) {
							Function.prototype.toString = $KEY_316.toString;
						}
					} catch (e) {}
				}
				let $BX = Object.getOwnPropertyNames($KEY_3);
				let i = 0;
				for (; i < $BX.length; i++) {
					if ($KEY_144(Array.prototype.indexOf, $KEY_238, $BX[i]) === -1) {
						let $BY = Object.getOwnPropertyDescriptor($BV, $BX[i]);
						if ($BY == null || $BY.configurable === true || $BY.writable === true) {
							let $BZ = Object.getOwnPropertyDescriptor($KEY_3, $BX[i]);
							if ($BZ != null) {
								if ($BY == null || $BY.configurable !== false || $BZ.configurable !== true) {
									Object.defineProperty($BV, $BX[i], $BZ);
								}
							}
						}
					}
				}
				try {
					if (!$KEY_144(Object.hasOwnProperty, $KEY_3, "prototype")) {
						$BV.prototype = void 0;
					}
				} catch (e_2) {}
				Object.defineProperty($BQ, $BN, $BP);
				$BT = {
					onBeforeInvoke: $KEY_2,
					onAfterInvoke: $KEY_11,
					originals: $BT
				};
				$BQ = {
					object: $BQ,
					result: $BT,
					original: $KEY_3
				};
				let $CA = $KEY_327[$BN];
				if ($CA == null) {
					$CA = [];
					$KEY_327[$BN] = $CA;
				}
				$KEY_144(Array.prototype.push, $CA, $BQ);
				return $BT;
			}
		}
		let $KEY_173 = $THREAD_1_125747;
		function $THREAD_8_99654() {
			let $KEY_1, $KEY_2, $KEY_5;
			function $THREAD_3_77510($BM) {
				let $KEY_10 = $BM.target;
				if ($KEY_10 == null) {
					return void 0;
				}
				if ($KEY_10 != null && $KEY_10.nodeType === 1 && $KEY_10.nodeName === "FORM") {
					let $BN, $BO;
					if ($BM.event != null && !$BM.event.isTrusted) {
						$BO = $BM.event;
						$BN = "originalTarget" in $BO;
					} else {
						$BN = false;
						$BO = $KEY_10;
					}
					let $BP;
					if ($BN) {
						$BP = $KEY_10;
					} else {
						let $BQ = $KEY_10;
						for (;;) {
							if ($BQ.parentNode != null) {
								$BO = $BQ.parentNode;
								$BO = $BO == null || $BO.nodeType !== 1 || $BO.nodeName !== "FORM";
							} else {
								$BO = false;
							}
							if (!$BO || $BQ.parentNode === $KEY_10) break;
							$BQ = $BQ.parentNode;
						}
						$BP = $BQ;
					}
					let $KEY_4 = $BP;
					let $KEY_7;
					function $THREAD_7_141687($BR) {
						try {
							$KEY_4.removeEventListener("submit", $KEY_7);
							if ($BR != null && $BR.defaultPrevented) {
								return void 0;
							} else {
								$KEY_100.notify({
									form: $KEY_10,
									event: $BR
								});
								return void 0;
							}
						} catch (e) {
							$KEY_0.notify("2.27:" + e);
							return void 0;
						}
					}
					$KEY_7 = $THREAD_7_141687;
					if ($BM.event == null) {
						$KEY_7();
						$KEY_5 = null;
						$KEY_2 = null;
						$KEY_1 = null;
						$KEY_4.removeEventListener("submit", $KEY_7);
						return void 0;
					} else {
						function $THREAD_2_93251($BR) {
							if ($BR.type !== "submit" || $BR.target !== $KEY_10) {
								return void 0;
							} else {
								$KEY_7($BR);
								$KEY_5 = null;
								return void 0;
							}
						}
						$KEY_5 = $THREAD_2_93251;
						function $THREAD_2_125393($BR) {
							if ($BR.type !== "submit" || $BR.target !== $KEY_10) {
								return void 0;
							} else {
								$KEY_7($BR);
								$KEY_2 = null;
								return void 0;
							}
						}
						$KEY_2 = $THREAD_2_125393;
						function $THREAD_1_69795($BR) {
							try {
								if (!$BR.isTrusted && !$BR.cancelable) {
									return void 0;
								}
								if ($BR.type !== "submit" || $BR.target !== $KEY_10) {
									return void 0;
								} else {
									$KEY_4.removeEventListener("submit", $KEY_7);
									$KEY_2 = null;
									$KEY_5 = null;
									$KEY_350.notify($KEY_10);
									return void 0;
								}
							} catch (e) {
								$KEY_0.notify("2.28:" + e);
								return void 0;
							}
						}
						$KEY_1 = $THREAD_1_69795;
						$KEY_4.addEventListener("submit", $KEY_7);
						return void 0;
					}
				} else {
					return void 0;
				}
			}
			let $KEY_4 = $THREAD_3_77510;
			if ($KEY_330) {
				return void 0;
			} else {
				try {
					function $THREAD_0_21380($BM) {
						let $BN = { event: void 0 };
						$BN.event = $BM;
						$BN.target = $BM.target;
						return $KEY_4($BN);
					}
					$KEY_32 = $THREAD_0_21380;
					window.document.addEventListener("submit", $THREAD_0_21380, true);
				} catch (e) {
					$KEY_0.notify("2.21:" + e);
					return void 0;
				}
				if ($KEY_96 != null && $KEY_96.onBeforeInvoke != null) {
					$KEY_96.onBeforeInvoke.register(($BM, $BN) => {
						try {
							if ($BN != null && $BN.bypassResult != null) {
								return $BN;
							} else {
								let $BO = {};
								$BO.target = $BM.thisObj;
								$KEY_4($BO);
								return $BN;
							}
						} catch (e) {
							$KEY_0.notify("2.22:" + e);
							return $BN;
						}
					});
				}
				if ($KEY_339 != null && $KEY_339.onBeforeInvoke != null) {
					$KEY_339.onBeforeInvoke.register(($BM, $BN) => {
						try {
							if ($BN != null && $BN.bypassResult != null) {
								return $BN;
							} else {
								if ($KEY_5 != null) {
									$KEY_5($BM.thisObj);
								}
								return $BN;
							}
						} catch (e) {
							$KEY_0.notify("2.23:" + e);
							return $BN;
						}
					});
				}
				if ($KEY_253 != null && $KEY_253.onBeforeInvoke != null) {
					$KEY_253.onBeforeInvoke.register(($BM, $BN) => {
						try {
							if ($BN != null && $BN.bypassResult != null) {
								return $BN;
							} else {
								if ($KEY_2 != null) {
									$KEY_2($BM.thisObj);
								}
								return $BN;
							}
						} catch (e) {
							$KEY_0.notify("2.24:" + e);
							return $BN;
						}
					});
				}
				if ($KEY_20 != null && $KEY_20.onBeforeInvoke != null) {
					$KEY_20.onBeforeInvoke.register(($BM, $BN) => {
						try {
							if ($BN != null && $BN.bypassResult != null) {
								return $BN;
							} else {
								if ($KEY_1 != null) {
									$KEY_1($BM.thisObj);
								}
								return $BN;
							}
						} catch (e) {
							$KEY_0.notify("2.25:" + e);
							return $BN;
						}
					});
				}
				if ($KEY_171 != null && $KEY_171.onBeforeSet != null) {
					$KEY_171.onBeforeSet.register(($BM, $BN) => {
						try {
							if ($BN != null && $BN.bypassResult != null) {
								return $BN;
							}
							if ($BM.param == null) {
								return $BN;
							} else {
								window.Event.prototype.stopPropagation.call($BM.thisObj);
								return $BN;
							}
						} catch (e) {
							$KEY_0.notify("2.26:" + e);
							return $BN;
						}
					});
				}
				$KEY_330 = true;
				return void 0;
			}
		}
		let $KEY_307 = $THREAD_8_99654;
		class $THREAD_8_71041 {
			constructor() {
				this.callbacks = [];
			}
			register($BM) {
				this.callbacks.push($BM);
				return this;
			}
			unregister($BM) {
				let i = 0;
				for (; i < this.callbacks.length; i++) {
					if (this.callbacks[i] === $BM) {
						this.callbacks[i] = void 0;
					}
				}
				return this;
			}
			clear() {
				this.callbacks = [];
				return this;
			}
			notify($BM) {
				let $BN = Array.prototype.slice.call(this.callbacks);
				let $BO = $BN.length;
				let $BP = null;
				while ($BO - 1 >= 0) {
					if ($BN[$BO - 1] != null) {
						try {
							$BP = $BN[$BO - 1].call(void 0, $BM, $BP);
						} catch (e) {}
					}
					--$BO;
				}
				return $BP;
			}
		}
		$KEY_96 = $KEY_173(window.HTMLFormElement.prototype, "submit");
		function $THREAD_5_120885($BM) {
			class $THREAD_1_120161 extends $BM {
				constructor() {
					super(...arguments);
				}
				register($BN) {
					$KEY_307();
					super.register($BN);
					return this;
				}
			}
			return $THREAD_1_120161;
		}
		let $BK = $THREAD_5_120885($THREAD_8_71041);
		$KEY_100 = new $BK();
		$KEY_350 = new $BK();
		$KEY_0 = new $BK();

}
bootstrap();
