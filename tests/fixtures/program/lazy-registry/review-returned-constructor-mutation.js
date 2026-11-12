function bootstrap() {
function $THREAD_1_75932($BR, $BS, $BT) {
			let $KEY_0 = $BT;
			let $KEY_3, $KEY_4, $KEY_12;
			if ($BT === void 0) {
				$KEY_0 = false;
			}
			let $BU = $KEY_9($BR, $BS);
			if ($BU == null) {
				return null;
			}
			let $BV = $BU.containingObj;
			$BU = $BU.desc;
			let $BW = $BU.configurable;
			let $BX = $BU.writable;
			$KEY_12 = $BU.value;
			if (!$KEY_406(Object.hasOwnProperty, $BU, "value")) {
				return null;
			}
			let $BY = Object.create(null);
			$BY.value = $KEY_12;
			if ($BW === false && $BX === false || typeof $KEY_12 !== "function") {
				$BY = { originals: $BY };
				return $BY;
			} else {
				let $BZ = $KEY_142($BV, $BS);
				if ($BZ != null) {
					if ($BZ.result != null) {
						return $BZ.result;
					}
					$KEY_12 = $BZ.original;
					$BY.value = $KEY_12;
				}
				$KEY_3 = $KEY_360();
				$KEY_4 = $KEY_360();
				function $THREAD_8_34667() {
					"use strict";
					let $KEY_5 = arguments;
					let $CG = $KEY_3.notify({
						args: $KEY_5,
						thisObj: this
					});
					let $CH;
					if ($CG) {
						if ($CG.bypassResult != null) {
							if ($CG.bypassResult.throw) {
								throw $CG.bypassResult.value;
							} else {
								return $CG.bypassResult.value;
							}
						} else {
							$CH = $CG.args != null ? $CG.args : $KEY_5;
						}
					} else {
						$CH = $KEY_5;
					}
					$KEY_5 = arguments;
					let $CI = {
						args: $KEY_5,
						thisObj: this,
						threw: true,
						result: null
					};
					let $CJ, $CK;
					try {
						$CJ = $KEY_0 && this instanceof $THREAD_8_34667 ? $KEY_263($KEY_12, $CH) : $KEY_451($KEY_12, this, $CH);
						$CI = {
							args: arguments,
							thisObj: this,
							threw: false,
							result: $CJ
						};
					} finally {
						let $CL = $KEY_4.notify($CI);
						if ($CL && $CL.bypassResult != null) {
							if ($CL.bypassResult.throw) {
								throw $CL.bypassResult.value;
							} else {
								return $CL.bypassResult.value;
							}
						} else {
							$CK = $CJ;
						}
					}
					return $CK;
				}
				$BU.value = $THREAD_8_34667;
				let $CA = $BU.value;
				if (typeof $KEY_12 === "function") {
					try {
						let $CB = $KEY_213.toString.call($KEY_12);
						$KEY_73.push($CB);
						$KEY_266.push($CA);
						if (Function.prototype.toString !== $KEY_213.toString) {
							Function.prototype.toString = $KEY_213.toString;
						}
					} catch (e) {}
				}
				let $CC = Object.getOwnPropertyNames($KEY_12);
				let i = 0;
				for (; i < $CC.length; i++) {
					if ($KEY_406(Array.prototype.indexOf, $KEY_206, $CC[i]) === -1) {
						let $CD = Object.getOwnPropertyDescriptor($CA, $CC[i]);
						if ($CD == null || $CD.configurable === true || $CD.writable === true) {
							let $CE = Object.getOwnPropertyDescriptor($KEY_12, $CC[i]);
							if ($CE != null) {
								if ($CD == null || $CD.configurable !== false || $CE.configurable !== true) {
									Object.defineProperty($CA, $CC[i], $CE);
								}
							}
						}
					}
				}
				try {
					if (!$KEY_406(Object.hasOwnProperty, $KEY_12, "prototype")) {
						$CA.prototype = void 0;
					}
				} catch (e_2) {}
				Object.defineProperty($BV, $BS, $BU);
				$BY = {
					onBeforeInvoke: $KEY_3,
					onAfterInvoke: $KEY_4,
					originals: $BY
				};
				$BV = {
					object: $BV,
					result: $BY,
					original: $KEY_12
				};
				let $CF = $KEY_140[$BS];
				if ($CF == null) {
					$CF = [];
					$KEY_140[$BS] = $CF;
				}
				$KEY_406(Array.prototype.push, $CF, $BV);
				return $BY;
			}
		}
let $KEY_195 = $THREAD_1_75932;
function $THREAD_9_78581() {
			let $KEY_1, $KEY_4, $KEY_7, $KEY_8;
			function $THREAD_9_28059($BT) {
				let $KEY_0 = $BT.target;
				if ($KEY_0 == null) {
					return void 0;
				}
				if ($KEY_0 != null && $KEY_0.nodeType === 1 && $KEY_0.nodeName === "FORM") {
					let $BU;
					if ($BT.event != null && !$BT.event.isTrusted && "originalTarget" in $BT.event) {
						$BU = $KEY_0;
					} else {
						let $BV = $KEY_0;
						let $BW;
						for (;;) {
							let $BX = $BV.parentNode;
							if ($BX == null || $BX != null && $BX.nodeType === 1 && $BX.nodeName === "FORM" || $BX === $KEY_0) break;
							$BV = $BX;
							$BW = $BX ?? $BW;
						}
						$BU = $BV;
					}
					let $KEY_5 = $BU;
					let $KEY_10;
					function $THREAD_8_15652($BY) {
						try {
							$KEY_5.removeEventListener("submit", $KEY_10);
							if ($BY != null && $BY.defaultPrevented) {
								return void 0;
							} else {
								$KEY_368.notify({
									form: $KEY_0,
									event: $BY
								});
								$KEY_1 = true;
								return void 0;
							}
						} catch (e) {
							$KEY_412.notify("2.27:" + e);
							return void 0;
						}
					}
					$KEY_10 = $THREAD_8_15652;
					if ($BT.event == null) {
						$KEY_10();
						$KEY_4 = null;
						$KEY_7 = null;
						$KEY_8 = null;
						$KEY_5.removeEventListener("submit", $KEY_10);
						return void 0;
					} else {
						function $THREAD_6_63888($BY) {
							try {
								if (!$BY.isTrusted && !$BY.cancelable) {
									return void 0;
								}
								if ($BY.type !== "submit" || $BY.target !== $KEY_0) {
									return void 0;
								} else {
									$KEY_5.removeEventListener("submit", $KEY_10);
									$KEY_7 = null;
									$KEY_4 = null;
									$KEY_70.notify($KEY_0);
									$KEY_1 = false;
									return void 0;
								}
							} catch (e) {
								$KEY_412.notify("2.28:" + e);
								return void 0;
							}
						}
						function $THREAD_0_150877($BY) {
							if ($BY.type !== "submit" || $BY.target !== $KEY_0) {
								return void 0;
							} else {
								$KEY_10($BY);
								$KEY_7 = null;
								return void 0;
							}
						}
						function $THREAD_9_54748($BY) {
							if ($BY.type !== "submit" || $BY.target !== $KEY_0) {
								return void 0;
							} else {
								$KEY_10($BY);
								$KEY_4 = null;
								return void 0;
							}
						}
						$KEY_4 = $THREAD_9_54748;
						$KEY_7 = $THREAD_0_150877;
						$KEY_8 = $THREAD_6_63888;
						$KEY_5.addEventListener("submit", $KEY_10);
						return void 0;
					}
				} else {
					return void 0;
				}
			}
			let $KEY_3 = $THREAD_9_28059;
			if ($KEY_351) {
				return void 0;
			} else {
				function $THREAD_6_8050($BT) {
					if ($KEY_1) {
						return void 0;
					} else {
						let $BU = $BT.formData;
						let $BV = $BT.destination.url;
						try {
							if ($BU != null) {
								let $BW = window.document.getElementsByTagName("form");
								let $BX = 0;
								let $BY = null;
								while ($BX < $BW.length) {
									let $BZ = $BW[$BX];
									if ($BZ.action === $BV) {
										if ($BY == null) {
											++$BX;
											$BY = $BZ;
											continue;
										}
										$BU = {
											form: null,
											formData: $BU,
											url: $BV
										};
										$BU = $KEY_198.notify($BU);
										if ($BU !== null) {
											if ($BT.canIntercept && $BU.cancelSubmission) {
												let $CA, $CB;
												if ($KEY_287 == null) {
													$CA = $KEY_287 == null;
													$CB = void 0;
												} else {
													$CB = $KEY_287.originals.value;
													$CA = $CB == null;
												}
												if (!$CA) {
													$CB.call($BT);
												}
											}
											if ($BU.updatedFormData != null) {
												if (window.navigation != null) {
													window.navigation.removeEventListener("navigate", $KEY_47);
												}
												$KEY_408($BV, $BU.updatedFormData, null);
											}
										}
										return void 0;
									} else {
										++$BX;
										continue;
									}
								}
								$BU = {
									form: $BY,
									formData: $BU,
									url: $BV
								};
								$BU = $KEY_198.notify($BU);
								if ($BU !== null) {
									if ($BT.canIntercept && $BU.cancelSubmission) {
										let $CC, $CD;
										if ($KEY_287 == null) {
											$CC = $KEY_287 == null;
											$CD = void 0;
										} else {
											$CD = $KEY_287.originals.value;
											$CC = $CD == null;
										}
										if (!$CC) {
											$CD.call($BT);
										}
									}
									if ($BU.updatedFormData != null) {
										if (window.navigation != null) {
											window.navigation.removeEventListener("navigate", $KEY_47);
										}
										$KEY_408($BV, $BU.updatedFormData, $BY);
									}
								}
							}
						} catch (e) {}
						return void 0;
					}
				}
				$KEY_1 = false;
				$KEY_47 = $THREAD_6_8050;
				try {
					function $THREAD_4_28042($BT) {
						let $BU = {
							event: void 0,
							target: void 0
						};
						$BU.event = $BT;
						$BU.target = $BT.target;
						return $KEY_3($BU);
					}
					$KEY_405 = $THREAD_4_28042;
					window.document.addEventListener("submit", $THREAD_4_28042, true);
					if (window.navigation != null) {
						window.navigation.addEventListener("navigate", $KEY_47);
					}
				} catch (e) {
					$KEY_412.notify("2.21:" + e);
					return void 0;
				}
				if ($KEY_190?.onBeforeInvoke != null) {
					$KEY_190.onBeforeInvoke.register(($BT, $BU) => {
						try {
							if ($BU?.bypassResult != null) {
								return $BU;
							} else {
								$KEY_3({ target: $BT.thisObj });
								return $BU;
							}
						} catch (e) {
							$KEY_412.notify("2.22:" + e);
							return $BU;
						}
					});
				}
				if ($KEY_418?.onBeforeInvoke != null) {
					$KEY_418.onBeforeInvoke.register(($BT, $BU) => {
						try {
							if ($BU?.bypassResult != null) {
								return $BU;
							} else {
								if ($KEY_4 != null) {
									$KEY_4($BT.thisObj);
								}
								return $BU;
							}
						} catch (e) {
							$KEY_412.notify("2.23:" + e);
							return $BU;
						}
					});
				}
				if ($KEY_232?.onBeforeInvoke != null) {
					$KEY_232.onBeforeInvoke.register(($BT, $BU) => {
						try {
							if ($BU?.bypassResult != null) {
								return $BU;
							} else {
								if ($KEY_7 != null) {
									$KEY_7($BT.thisObj);
								}
								return $BU;
							}
						} catch (e) {
							$KEY_412.notify("2.24:" + e);
							return $BU;
						}
					});
				}
				if ($KEY_39?.onBeforeInvoke != null) {
					$KEY_39.onBeforeInvoke.register(($BT, $BU) => {
						try {
							if ($BU?.bypassResult != null) {
								return $BU;
							} else {
								if ($KEY_8 != null) {
									$KEY_8($BT.thisObj);
								}
								return $BU;
							}
						} catch (e) {
							$KEY_412.notify("2.25:" + e);
							return $BU;
						}
					});
				}
				if ($KEY_445?.onBeforeSet != null) {
					$KEY_445.onBeforeSet.register(($BT, $BU) => {
						try {
							if ($BU?.bypassResult != null) {
								return $BU;
							}
							if ($BT.param == null) {
								return $BU;
							} else {
								window.Event.prototype.stopPropagation.call($BT.thisObj);
								return $BU;
							}
						} catch (e) {
							$KEY_412.notify("2.26:" + e);
							return $BU;
						}
					});
				}
				let $BR, $BS;
				if ($KEY_287 == null) {
					$BR = $KEY_287 == null;
					$BS = void 0;
				} else {
					$BS = $KEY_287.onAfterInvoke;
					$BR = $BS == null;
				}
				if (!$BR) {
					$BS.register(($BT, $BU) => {
						if ($BU?.bypassResult != null || $BT.threw) {
							return $BU;
						} else {
							let $BV;
							if ($BT.thisObj?.formData != null) {
								let $BW = window.document.getElementsByTagName("form");
								let $BX = 0;
								$BV = null;
								while ($BX < $BW.length) {
									let $BY = $BW[$BX];
									if ($BY.action === $BT.thisObj.destination.url) {
										if ($BV == null) {
											++$BX;
											$BV = $BY;
											continue;
										}
										$KEY_1 = false;
										return $BU;
									} else {
										++$BX;
										continue;
									}
								}
							} else {
								$BV = null;
							}
							if ($BV != null) {
								$KEY_70.notify($BV);
							}
							$KEY_1 = false;
							return $BU;
						}
					});
				}
				$KEY_351 = true;
				return void 0;
			}
		}
let $KEY_28 = $THREAD_9_78581;
class $THREAD_1_2521 {
			constructor() {
				this.callbacks = [];
			}
			register($BR) {
				this.callbacks.push($BR);
				return this;
			}
			unregister($BR) {
				let i = 0;
				for (; i < this.callbacks.length; i++) {
					if (this.callbacks[i] === $BR) {
						this.callbacks[i] = void 0;
					}
				}
				return this;
			}
			clear() {
				this.callbacks = [];
				return this;
			}
			notify($BR) {
				let $BS = Array.prototype.slice.call(this.callbacks);
				let $BT = $BS.length;
				let $BU = null;
				while ($BT - 1 >= 0) {
					if ($BS[$BT - 1] != null) {
						try {
							$BU = $BS[$BT - 1].call(void 0, $BR, $BU);
						} catch (e) {}
					}
					--$BT;
				}
				return $BU;
			}
		}

let $KEY_190;
$KEY_190 = $KEY_195(window.HTMLFormElement.prototype, "submit");
function $THREAD_8_110436($BR) {
			class $THREAD_7_64696 extends $BR {
				constructor() {
					super(...arguments);
				}
				register($BS) {
					$KEY_28();
					super.register($BS);
					return this;
				}
			}
			return $THREAD_7_64696;
		}
let $BP = $THREAD_8_110436($THREAD_1_2521);
 const ReturnedAlias = $BP; ReturnedAlias.prototype.register = function () { return this; };
		$KEY_368 = new $BP();
		$KEY_70 = new $BP();
		$KEY_198 = new $BP();
		$KEY_412 = new $BP();

}
bootstrap();
