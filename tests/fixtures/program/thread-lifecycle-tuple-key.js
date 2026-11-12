//                                             Subscription lifecycle with a tuple key.
// Both lifecycle classes, private installers, constructor aliases, observable carrier and consumers.
function bootstrap() {
class $THREAD_0_78027 {
					constructor($BH) {
						this._cleanup = void 0;
						this._observer = $BH;
					}
					unsubscribe() {
						if (this.isComplete()) {
							return void 0;
						} else {
							this._observer = void 0;
							let $BH = this._cleanup;
							if ($BH !== void 0) {
								this._cleanup = void 0;
								try {
									$BH();
								} catch (e) {}
							}
							return void 0;
						}
					}
					isComplete() {
						return this._observer === void 0;
					}
				}
				let $L = { key: "closed" };
				function $THREAD_3_149244() {
					"use strict";
					return this._observer === void 0;
				}
				$L.get = $THREAD_3_149244;
				$L = [$L];
				let i = 0;
				for (; i < $L.length; i++) {
					$L[i].enumerable = $L[i].enumerable || false;
					$L[i].configurable = true;
					if ("value" in $L[i]) {
						$L[i].writable = true;
					}
					let $M;
					if (typeof $L[i].key != "object" ? typeof $L[i].key != "object" : !$L[i].key) {
						$M = $L[i].key;
					} else if ($L[i].key[Symbol.toPrimitive] !== void 0) {
						$M = $L[i].key[Symbol.toPrimitive].call($L[i].key, "string");
						if (typeof $M == "object") {
							throw new TypeError("@@toPrimitive must return a primitive value.");
						}
					} else {
						$M = String($L[i].key);
					}
					$M = [$M, $M];
					Object.defineProperty($THREAD_0_78027.prototype, typeof $M[1] == "symbol" ? $M[0] : $M[0] + "", $L[i]);
				}
				Object.defineProperty($THREAD_0_78027, "prototype", { writable: false });
				let $KEY_293 = $THREAD_0_78027;
				class $THREAD_9_127080 {
					constructor($BH) {
						this._subscription = $BH;
					}
					next($BH) {
						try {
							if (this._subscription._observer == null) {
								return void 0;
							}
							if (this._subscription._observer.next) {
								this._subscription._observer.next($BH);
								return void 0;
							} else {
								return void 0;
							}
						} catch (e) {
							return void 0;
						}
					}
					error($BH) {
						let $KEY_2 = this._subscription;
						let $BI = $KEY_2._observer;
						$KEY_2._observer = void 0;
						try {
							let $BJ = $KEY_2._cleanup;
							let $BK;
							if ($BJ === void 0) {
								$BK = $BH;
							} else {
								$KEY_2._cleanup = void 0;
								try {
									$BJ();
									$BK = $BH;
								} catch (e) {
									$BK = $BH;
								}
							}
							if ($BI) {
								if ($BI.error) {
									$BI.error($BK);
								}
							}
						} catch (e_2) {}
					}
					complete() {
						let $KEY_1 = this._subscription;
						let $BH = $KEY_1._observer;
						if ($KEY_1._observer === void 0) {
							return void 0;
						}
						$KEY_1._observer = void 0;
						try {
							if (typeof ($BH ? $BH.complete : void 0) === "function") {
								($BH ? $BH.complete : void 0).call($BH);
							}
						} catch (e) {}
						let $BI, $BJ;
						try {
							let $BK = $KEY_1._cleanup;
							if ($BK !== void 0) {
								$KEY_1._cleanup = void 0;
								try {
									$BK();
								} catch (e_2) {}
							}
							$BI = false;
							$BJ = void 0;
						} catch (e_3) {
							$BI = true;
							$BJ = e_3;
						}
						if ($BI) {
							throw $BJ;
						} else {
							return void 0;
						}
					}
					isComplete() {
						return this._subscription.isComplete();
					}
				}
				let $N = { key: "closed" };
				function $THREAD_7_140377() {
					"use strict";
					return this._subscription._observer === void 0;
				}
				$N.get = $THREAD_7_140377;
				$N = [$N];
				let j = 0;
				for (; j < $N.length; j++) {
					$N[j].enumerable = $N[j].enumerable || false;
					$N[j].configurable = true;
					if ("value" in $N[j]) {
						$N[j].writable = true;
					}
					let $O;
					if (typeof $N[j].key != "object" ? typeof $N[j].key != "object" : !$N[j].key) {
						$O = $N[j].key;
					} else if ($N[j].key[Symbol.toPrimitive] !== void 0) {
						$O = $N[j].key[Symbol.toPrimitive].call($N[j].key, "string");
						if (typeof $O == "object") {
							throw new TypeError("@@toPrimitive must return a primitive value.");
						}
					} else {
						$O = String($N[j].key);
					}
					Object.defineProperty($THREAD_9_127080.prototype, typeof $O == "symbol" ? $O : $O + "", $N[j]);
				}
				Object.defineProperty($THREAD_9_127080, "prototype", { writable: false });
				let $KEY_52 = $THREAD_9_127080;
				function $THREAD_0_14142($BH, $BI, $BJ) {
					function $THREAD_6_7466() {
						if ($KEY_8.length > 0) {
							$KEY_8 = [];
							$KEY_10 = false;
							$KEY_9.forEach($BM => {
								$KEY_3($BM, 0);
							});
						}
					}
					function $THREAD_7_70950($BM) {
						$KEY_9.push($BM);
						if ($KEY_2 && !$KEY_10 && $KEY_8.length === 0) {
							$KEY_3($BM, 0);
						}
					}
					function $THREAD_8_74603($BM) {
						let $KEY_0 = $BM;
						let $KEY_2;
						if ($KEY_11) {
							$KEY_2 = $KEY_1;
							$KEY_3(() => {
								$KEY_0($KEY_2);
							}, 0);
						} else {
							$KEY_5.push($KEY_0);
						}
					}
					let $KEY_0 = $BH;
					let $KEY_1, $KEY_2;
					let $KEY_3 = $BI;
					let $KEY_5, $KEY_6, $KEY_8, $KEY_9, $KEY_10, $KEY_11;
					const $BK = $BJ === void 0 ? [] : $BJ;
					$KEY_2 = false;
					$KEY_10 = false;
					$KEY_11 = false;
					$KEY_9 = [];
					$KEY_5 = [];
					$KEY_8 = $BK.slice();
					function $THREAD_3_164672($BM) {
						let $KEY_4 = $BM;
						let $KEY_12;
						if ($BM != null) {
							$KEY_1 = $BM;
							$KEY_11 = true;
							$KEY_5.forEach($BN => {
								let $KEY_0 = $BN;
								$KEY_3(() => $KEY_0($KEY_4), 0);
							});
							return void 0;
						} else {
							if ($KEY_8.length > 0) {
								$KEY_10 = true;
								$KEY_12 = $KEY_8.shift();
								$KEY_3(() => {
									$KEY_0($KEY_12, $KEY_53($KEY_6));
								}, 0);
							} else {
								$KEY_10 = false;
								$KEY_9.forEach($BN => {
									$KEY_3($BN, 0);
								});
							}
							return void 0;
						}
					}
					$KEY_6 = $THREAD_3_164672;
					function $THREAD_8_10951() {
						if ($KEY_11) {
							throw new Error("Queue has already thrown an error: " + ("" + $KEY_1));
						}
						if ($KEY_2) {
							return void 0;
						} else {
							$KEY_2 = true;
							$KEY_6();
							return void 0;
						}
					}
					let $BL = { start: $THREAD_8_10951 };
					function $THREAD_0_165585() {
						if ($KEY_11) {
							throw new Error("Queue has already thrown an error: " + ("" + $KEY_1));
						} else {
							let $KEY_9 = arguments.length;
							let $BM = new Array($KEY_9);
							let i = 0;
							for (; i < $KEY_9; i++) {
								$BM[i] = arguments[i];
							}
							Array.prototype.push.apply($KEY_8, $BM);
							if ($KEY_2 && !$KEY_10) {
								$KEY_6();
							}
							return void 0;
						}
					}
					$BL.enqueue = $THREAD_0_165585;
					$BL.empty = $THREAD_6_7466;
					$BL.onEmpty = $THREAD_7_70950;
					$BL.onError = $THREAD_8_74603;
					return $BL;
				}
				function $THREAD_4_82649() {
					class $THREAD_1_66489 {
						constructor($BH) {
							if (typeof $BH !== "function") {
								throw new TypeError();
							} else {
								this._subscriber = $BH;
								return void 0;
							}
						}
						static of() {
							let $KEY_4;
							let $KEY_1 = arguments.length;
							$KEY_4 = new Array($KEY_1);
							let i = 0;
							for (; i < $KEY_1; i++) {
								$KEY_4[i] = arguments[i];
							}
							return new $KEY_0($BH => {
								let $KEY_0 = $BH;
								$KEY_4.forEach($BI => $KEY_0.next($BI));
								$KEY_0.complete();
							});
						}
						static empty() {
							return new $KEY_0($BH => {
								$BH.complete();
							});
						}
						static from($BH, $BI) {
							let $KEY_1 = $BI;
							let $KEY_2;
							let $KEY_3 = $BH;
							if ($BH != null) {
								if ($BH instanceof $KEY_0) {
									return $BH;
								}
								if (typeof Promise !== "undefined" && $BH instanceof Promise) {
									return new $KEY_0($BJ => {
										let $KEY_0 = $BJ;
										$KEY_3.then($BK => {
											$KEY_0.next($BK);
											$KEY_0.complete();
										}).catch($BK => {
											$KEY_0.error($BK);
										});
									});
								}
								if (typeof $KEY_3.length === "number" && typeof $KEY_3 !== "function") {
									$KEY_2 = $KEY_3;
									return new $KEY_0($BJ => {
										let $KEY_0 = $BJ;
										let $KEY_5;
										if ($KEY_1 != null) {
											$KEY_5 = 0;
											$KEY_1.schedule(() => {
												try {
													if ($KEY_5 >= $KEY_2.length) {
														$KEY_0.complete();
														return false;
													} else {
														$KEY_0.next($KEY_2[$KEY_5]);
														++$KEY_5;
														return true;
													}
												} catch (e) {
													$KEY_0.error(e);
													return false;
												}
											});
										} else {
											try {
												let $BK = 0;
												while ($BK < $KEY_2.length) {
													$KEY_0.next($KEY_2[$BK]);
													++$BK;
												}
												$KEY_0.complete();
											} catch (e) {
												$KEY_0.error(e);
											}
										}
									});
								}
								if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_3[Symbol.iterator]) {
									return new $KEY_0($BJ => {
										let $KEY_0 = $BJ;
										let $KEY_2;
										try {
											$KEY_2 = $KEY_3[Symbol.iterator]();
											if ($KEY_1 != null) {
												$KEY_1.schedule(() => {
													try {
														let $BL = $KEY_2.next();
														if ($BL.done) {
															$KEY_0.complete();
															return false;
														} else {
															$KEY_0.next($BL.value);
															return true;
														}
													} catch (e) {
														$KEY_0.error(e);
														return false;
													}
												});
											} else {
												for (;;) {
													let $BK = $KEY_2.next();
													if ($BK.done) break;
													$KEY_0.next($BK.value);
												}
												$KEY_0.complete();
											}
										} catch (e) {
											$KEY_0.error(e);
										}
									});
								}
							}
							throw new Error("Could not convert " + $KEY_3 + " to observable");
						}
						subscribe($BH, $BI, $BJ) {
							let $BK = {
								start: typeof $BH === "object" ? $BH.start : void 0,
								next: typeof $BH === "function" ? $BH : $BH ? $BH.next : void 0,
								error: typeof $BH === "object" ? $BH.error : $BI,
								complete: typeof $BH === "object" ? $BH.complete : $BJ
							};
							if (this._subscriber == null) {
								throw new TypeError();
							} else {
								let $BL = new $KEY_293($BK);
								if (typeof $BK.start === "function") {
									$BK.start($BL);
								}
								let $BM = new $KEY_52($BL);
								try {
									$BL._cleanup = $KEY_444(this._subscriber, $BM);
								} catch (e) {
									$BM.error(e);
								}
								if ($BL._observer === void 0) {
									let $BN = $BL._cleanup;
									if ($BN !== void 0) {
										$BL._cleanup = void 0;
										try {
											$BN();
										} catch (e_2) {}
									}
								}
								return $BL;
							}
						}
						pipe() {
							let $KEY_4 = arguments.length;
							let $BH = new Array($KEY_4);
							let i = 0;
							for (; i < $KEY_4; i++) {
								$BH[i] = arguments[i];
							}
							return $BH.reduce(($BI, $BJ) => $BJ($BI), this);
						}
						map($BH) {
							return this.pipe($KEY_41($BH));
						}
						unorderedFlatMap($BH) {
							return this.pipe($KEY_158($BH));
						}
					}
					let $KEY_0 = $THREAD_1_66489;
					return $KEY_0;
				}
				$KEY_148 = $THREAD_4_82649();
}
bootstrap();
