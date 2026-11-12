function bootstrap() {
		let $KEY_23, $KEY_90, $KEY_95, $KEY_148, $KEY_180, $KEY_199, $KEY_297, $KEY_300, $KEY_405, $KEY_409, $KEY_422, $KEY_441;
		$KEY_441 = false;
		let $KEY_24, $KEY_119, $KEY_156, $KEY_209, $KEY_216, $KEY_269, $KEY_321, $KEY_326;
		$KEY_326 = Object.create(null);
function $THREAD_8_45175($BT, $BU, $BV) {
			let $KEY_6 = $BV;
			let $KEY_7;
			if ($BV === void 0) {
				$KEY_6 = false;
			}
			let $BW = $KEY_209($BT, $BU);
			if ($BW == null) {
				return null;
			}
			let $BX = $BW.containingObj;
			$BW = $BW.desc;
			let $BY = $BW.configurable;
			let $BZ = $BW.writable;
			$KEY_7 = $BW.value;
			if (!$KEY_119(Object.hasOwnProperty, $BW, "value")) {
				return null;
			}
			let $CA = Object.create(null);
			$CA.value = $KEY_7;
			if ($BY === false && $BZ === false || typeof $KEY_7 !== "function") {
				$CA = { originals: $CA };
				return $CA;
			} else {
				let $CB = $KEY_269($BX, $BU);
				if ($CB != null) {
					if ($CB.result != null) {
						return $CB.result;
					}
					$KEY_7 = $CB.original;
					$CA.value = $KEY_7;
				}
				let $KEY_3 = $KEY_156();
				let $KEY_2 = $KEY_156();
				function $THREAD_8_133993() {
					"use strict";
					let $KEY_8 = arguments;
					let $CI = $KEY_3.notify({
						args: $KEY_8,
						thisObj: this
					});
					let $CJ;
					if ($CI) {
						if ($CI.bypassResult != null) {
							if ($CI.bypassResult.throw) {
								throw $CI.bypassResult.value;
							} else {
								return $CI.bypassResult.value;
							}
						} else {
							$CJ = $CI.args != null ? $CI.args : $KEY_8;
						}
					} else {
						$CJ = $KEY_8;
					}
					let $CK = {
						args: void 0,
						thisObj: void 0
					};
					$KEY_8 = arguments;
					$CK.args = $KEY_8;
					$CK.thisObj = this;
					$CK.threw = true;
					$CK.result = null;
					let $CL;
					let $CM = $CK;
					let $CN;
					try {
						$CL = $KEY_6 && this instanceof $THREAD_8_133993 ? $KEY_418($KEY_7, $CJ) : $KEY_177($KEY_7, this, $CJ);
						$CM = {
							args: arguments,
							thisObj: this,
							threw: false,
							result: $CL
						};
					} finally {
						let $CP = $KEY_2.notify($CM);
						if ($CP && $CP.bypassResult != null) {
							if ($CP.bypassResult.throw) {
								throw $CP.bypassResult.value;
							} else {
								return $CP.bypassResult.value;
							}
						} else {
							$CN = $CL;
						}
					}
					return $CN;
				}
				$BW.value = $THREAD_8_133993;
				let $CC = $BW.value;
				if (typeof $KEY_7 === "function") {
					try {
						let $CD = $KEY_216.toString.call($KEY_7);
						$KEY_24.push($CD);
						$KEY_321.push($CC);
						if (Function.prototype.toString !== $KEY_216.toString) {
							Function.prototype.toString = $KEY_216.toString;
						}
					} catch (e) {}
				}
				let $CE = Object.getOwnPropertyNames($KEY_7);
				let i = 0;
				for (; i < $CE.length; i++) {
					if ($KEY_119(Array.prototype.indexOf, $KEY_61, $CE[i]) === -1) {
						let $CF = Object.getOwnPropertyDescriptor($CC, $CE[i]);
						if ($CF == null || $CF.configurable === true || $CF.writable === true) {
							let $CG = Object.getOwnPropertyDescriptor($KEY_7, $CE[i]);
							if ($CG != null) {
								if ($CF == null || $CF.configurable !== false || $CG.configurable !== true) {
									Object.defineProperty($CC, $CE[i], $CG);
								}
							}
						}
					}
				}
				try {
					if (!$KEY_119(Object.hasOwnProperty, $KEY_7, "prototype")) {
						$CC.prototype = void 0;
					}
				} catch (e_2) {}
				Object.defineProperty($BX, $BU, $BW);
				$CA = {
					onBeforeInvoke: $KEY_3,
					onAfterInvoke: $KEY_2,
					originals: $CA
				};
				$BX = {
					object: $BX,
					result: $CA,
					original: $KEY_7
				};
				let $CH = $KEY_326[$BU];
				if ($CH == null) {
					$CH = [];
					$KEY_326[$BU] = $CH;
				}
				$KEY_119(Array.prototype.push, $CH, $BX);
				return $CA;
			}
		}
let $KEY_26 = $THREAD_8_45175;
function $THREAD_2_39726() {
			let $KEY_1, $KEY_2, $KEY_3, $KEY_4;
			function $THREAD_5_39262($BV) {
				let $KEY_5 = $BV.target;
				if ($KEY_5 == null) {
					return void 0;
				}
				if ($KEY_5 != null && $KEY_5.nodeType === 1 && $KEY_5.nodeName === "FORM") {
					let $BW;
					if ($BV.event != null && !$BV.event.isTrusted && "originalTarget" in $BV.event) {
						$BW = $KEY_5;
					} else {
						let $BX = $KEY_5;
						let $BY;
						for (;;) {
							let $BZ = $BX.parentNode;
							if ($BZ == null || $BZ != null && $BZ.nodeType === 1 && $BZ.nodeName === "FORM" || $BZ === $KEY_5) break;
							$BX = $BZ;
							$BY = $BZ ?? $BY;
						}
						$BW = $BX;
					}
					let $KEY_8 = $BW;
					let $KEY_10;
					function $THREAD_6_148240($CA) {
						try {
							$KEY_8.removeEventListener("submit", $KEY_10);
							if ($CA != null && $CA.defaultPrevented) {
								return void 0;
							} else {
								$KEY_148.notify({
									form: $KEY_5,
									event: $CA
								});
								$KEY_1 = true;
								return void 0;
							}
						} catch (e) {
							$KEY_405.notify("2.27:" + e);
							return void 0;
						}
					}
					$KEY_10 = $THREAD_6_148240;
					if ($BV.event == null) {
						$THREAD_6_148240();
						$KEY_3 = null;
						$KEY_2 = null;
						$KEY_4 = null;
						$KEY_8.removeEventListener("submit", $KEY_10);
						return void 0;
					} else {
						function $THREAD_8_128315($CA) {
							if ($CA.type !== "submit" || $CA.target !== $KEY_5) {
								return void 0;
							} else {
								$KEY_10($CA);
								$KEY_3 = null;
								return void 0;
							}
						}
						$KEY_3 = $THREAD_8_128315;
						function $THREAD_5_56065($CA) {
							if ($CA.type !== "submit" || $CA.target !== $KEY_5) {
								return void 0;
							} else {
								$KEY_10($CA);
								$KEY_2 = null;
								return void 0;
							}
						}
						$KEY_2 = $THREAD_5_56065;
						function $THREAD_5_66963($CA) {
							try {
								if (!$CA.isTrusted && !$CA.cancelable) {
									return void 0;
								}
								if ($CA.type !== "submit" || $CA.target !== $KEY_5) {
									return void 0;
								} else {
									$KEY_8.removeEventListener("submit", $KEY_10);
									$KEY_2 = null;
									$KEY_3 = null;
									$KEY_300.notify($KEY_5);
									$KEY_1 = false;
									return void 0;
								}
							} catch (e) {
								$KEY_405.notify("2.28:" + e);
								return void 0;
							}
						}
						$KEY_4 = $THREAD_5_66963;
						$KEY_8.addEventListener("submit", $KEY_10);
						return void 0;
					}
				} else {
					return void 0;
				}
			}
			let $KEY_0 = $THREAD_5_39262;
			if ($KEY_441) {
				return void 0;
			} else {
				$KEY_1 = false;
				function $THREAD_4_81296($BV) {
					if ($KEY_1) {
						return void 0;
					} else {
						let $BW = $BV.formData;
						let $BX = $BV.destination.url;
						try {
							if ($BW != null) {
								let $BY = window.document.getElementsByTagName("form");
								let $BZ = 0;
								let $CA = null;
								while ($BZ < $BY.length) {
									let $CB = $BY[$BZ];
									if ($CB.action === $BX) {
										if ($CA == null) {
											++$BZ;
											$CA = $CB;
											continue;
										}
										$BW = {
											form: null,
											formData: $BW,
											url: $BX
										};
										$BW = $KEY_199.notify($BW);
										if ($BW !== null) {
											if ($BV.canIntercept && $BW.cancelSubmission) {
												let $CC, $CD;
												if ($KEY_23 == null) {
													$CC = $KEY_23 == null;
													$CD = void 0;
												} else {
													$CD = $KEY_23.originals.value;
													$CC = $CD == null;
												}
												if (!$CC) {
													$CD.call($BV);
												}
											}
											if ($BW.updatedFormData != null) {
												if (window.navigation != null) {
													window.navigation.removeEventListener("navigate", $KEY_422);
												}
												$KEY_28($BX, $BW.updatedFormData, null);
											}
										}
										return void 0;
									} else {
										++$BZ;
										continue;
									}
								}
								$BW = {
									form: $CA,
									formData: $BW,
									url: $BX
								};
								$BW = $KEY_199.notify($BW);
								if ($BW !== null) {
									if ($BV.canIntercept && $BW.cancelSubmission) {
										let $CE, $CF;
										if ($KEY_23 == null) {
											$CE = $KEY_23 == null;
											$CF = void 0;
										} else {
											$CF = $KEY_23.originals.value;
											$CE = $CF == null;
										}
										if (!$CE) {
											$CF.call($BV);
										}
									}
									if ($BW.updatedFormData != null) {
										if (window.navigation != null) {
											window.navigation.removeEventListener("navigate", $KEY_422);
										}
										$KEY_28($BX, $BW.updatedFormData, $CA);
									}
								}
							}
						} catch (e) {}
						return void 0;
					}
				}
				$KEY_422 = $THREAD_4_81296;
				try {
					function $THREAD_0_69964($BV) {
						let $BW = {
							event: void 0,
							target: void 0
						};
						$BW.event = $BV;
						$BW.target = $BV.target;
						return $KEY_0($BW);
					}
					$KEY_90 = $THREAD_0_69964;
					window.document.addEventListener("submit", $THREAD_0_69964, true);
					if (window.navigation != null) {
						window.navigation.addEventListener("navigate", $KEY_422);
					}
				} catch (e) {
					$KEY_405.notify("2.21:" + e);
					return void 0;
				}
				if ($KEY_448?.onBeforeInvoke != null) {
					$KEY_448.onBeforeInvoke.register(($BV, $BW) => {
						try {
							if ($BW?.bypassResult != null) {
								return $BW;
							} else {
								$KEY_0({ target: $BV.thisObj });
								return $BW;
							}
						} catch (e) {
							$KEY_405.notify("2.22:" + e);
							return $BW;
						}
					});
				}
				if ($KEY_95?.onBeforeInvoke != null) {
					$KEY_95.onBeforeInvoke.register(($BV, $BW) => {
						try {
							if ($BW?.bypassResult != null) {
								return $BW;
							} else {
								if ($KEY_3 != null) {
									$KEY_3($BV.thisObj);
								}
								return $BW;
							}
						} catch (e) {
							$KEY_405.notify("2.23:" + e);
							return $BW;
						}
					});
				}
				if ($KEY_297?.onBeforeInvoke != null) {
					$KEY_297.onBeforeInvoke.register(($BV, $BW) => {
						try {
							if ($BW?.bypassResult != null) {
								return $BW;
							} else {
								if ($KEY_2 != null) {
									$KEY_2($BV.thisObj);
								}
								return $BW;
							}
						} catch (e) {
							$KEY_405.notify("2.24:" + e);
							return $BW;
						}
					});
				}
				if ($KEY_180?.onBeforeInvoke != null) {
					$KEY_180.onBeforeInvoke.register(($BV, $BW) => {
						try {
							if ($BW?.bypassResult != null) {
								return $BW;
							} else {
								if ($KEY_4 != null) {
									$KEY_4($BV.thisObj);
								}
								return $BW;
							}
						} catch (e) {
							$KEY_405.notify("2.25:" + e);
							return $BW;
						}
					});
				}
				if ($KEY_409?.onBeforeSet != null) {
					$KEY_409.onBeforeSet.register(($BV, $BW) => {
						try {
							if ($BW?.bypassResult != null) {
								return $BW;
							}
							if ($BV.param == null) {
								return $BW;
							} else {
								window.Event.prototype.stopPropagation.call($BV.thisObj);
								return $BW;
							}
						} catch (e) {
							$KEY_405.notify("2.26:" + e);
							return $BW;
						}
					});
				}
				let $BT, $BU;
				if ($KEY_23 == null) {
					$BT = $KEY_23 == null;
					$BU = void 0;
				} else {
					$BU = $KEY_23.onAfterInvoke;
					$BT = $BU == null;
				}
				if (!$BT) {
					$BU.register(($BV, $BW) => {
						if ($BW?.bypassResult != null || $BV.threw) {
							return $BW;
						} else {
							let $BX;
							if ($BV.thisObj?.formData != null) {
								let $BY = window.document.getElementsByTagName("form");
								let $BZ = 0;
								$BX = null;
								while ($BZ < $BY.length) {
									let $CA = $BY[$BZ];
									if ($CA.action === $BV.thisObj.destination.url) {
										if ($BX == null) {
											++$BZ;
											$BX = $CA;
											continue;
										}
										$KEY_1 = false;
										return $BW;
									} else {
										++$BZ;
										continue;
									}
								}
							} else {
								$BX = null;
							}
							if ($BX != null) {
								$KEY_300.notify($BX);
							}
							$KEY_1 = false;
							return $BW;
						}
					});
				}
				$KEY_441 = true;
				return void 0;
			}
		}
let $KEY_328 = $THREAD_2_39726;
function $THREAD_5_99971() {
			"use strict";
			this.callbacks = [];
		}
function $THREAD_9_93980($BT) {
			"use strict";
			this.callbacks.push($BT);
			return this;
		}
		$THREAD_5_99971.prototype.register = $THREAD_9_93980;
		function $THREAD_2_77235($BT) {
			"use strict";
			let i = 0;
			for (; i < this.callbacks.length; i++) {
				if (this.callbacks[i] === $BT) {
					this.callbacks[i] = void 0;
				}
			}
			return this;
		}
		$THREAD_5_99971.prototype.unregister = $THREAD_2_77235;
		function $THREAD_4_158679() {
			"use strict";
			this.callbacks = [];
			return this;
		}
		$THREAD_5_99971.prototype.clear = $THREAD_4_158679;
		function $THREAD_2_99666($BT) {
			"use strict";
			let $BU = Array.prototype.slice.call(this.callbacks);
			let $BV = $BU.length;
			let $BW = null;
			while ($BV - 1 >= 0) {
				if ($BU[$BV - 1] != null) {
					try {
						$BW = $BU[$BV - 1].call(void 0, $BT, $BW);
					} catch (e) {}
				}
				--$BV;
			}
			return $BW;
		}
		$THREAD_5_99971.prototype.notify = $THREAD_2_99666;
let $KEY_448;
$KEY_448 = $KEY_26(window.HTMLFormElement.prototype, "submit");
function $THREAD_3_76512($BT) {
			class $THREAD_8_75676 extends $BT {
				constructor() {
					super(...arguments);
				}
				register($BU) {
					$KEY_328();
					super.register($BU);
					return this;
				}
			}
			return $THREAD_8_75676;
		}
let $BR = $THREAD_3_76512($THREAD_5_99971);
		$KEY_148 = new $BR();
		$KEY_300 = new $BR();
		$KEY_199 = new $BR();
		$KEY_405 = new $BR();

}
bootstrap();
