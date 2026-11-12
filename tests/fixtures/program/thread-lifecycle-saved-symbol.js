//                                        Subscription lifecycle with a saved symbol.
// Both lifecycle classes, private installers, constructor aliases, observable carrier and consumers.
function bootstrap() {
class $THREAD_8_45212 {
			constructor($AN) {
				this._cleanup = void 0;
				this._observer = $AN;
			}
			unsubscribe() {
				if (this.isComplete()) {
					return void 0;
				} else {
					this._observer = void 0;
					let $AN = this._cleanup;
					if ($AN !== void 0) {
						this._cleanup = void 0;
						try {
							$AN();
						} catch (e) {}
					}
					return void 0;
				}
			}
			isComplete() {
				return this._observer === void 0;
			}
		}
		let $T = { key: "closed" };
		function $THREAD_1_51508() {
			"use strict";
			return this._observer === void 0;
		}
		$T.get = $THREAD_1_51508;
		$T = [$T];
		let i = 0;
		for (; i < $T.length; i++) {
			$T[i].enumerable = $T[i].enumerable || false;
			$T[i].configurable = true;
			if ("value" in $T[i]) {
				$T[i].writable = true;
			}
			let $U;
			if (typeof $T[i].key != "object" ? typeof $T[i].key != "object" : !$T[i].key) {
				$U = $T[i].key;
			} else if ($T[i].key[Symbol.toPrimitive] !== void 0) {
				let $V = [$T[i].key[Symbol.toPrimitive], $T[i].key[Symbol.toPrimitive].call];
				$U = $V[1].call($V[0], $T[i].key, "string");
				if (typeof $U == "object") {
					throw new TypeError("@@toPrimitive must return a primitive value.");
				}
			} else {
				$U = String($T[i].key);
			}
			Object.defineProperty($THREAD_8_45212.prototype, typeof $U == "symbol" ? $U : String($U), $T[i]);
		}
		Object.defineProperty($THREAD_8_45212, "prototype", { writable: false });
		let $KEY_83 = $THREAD_8_45212;
		class $THREAD_8_123240 {
			constructor($AN) {
				this._subscription = $AN;
			}
			next($AN) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($AN);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($AN) {
				let $KEY_1 = this._subscription;
				let $AO = $KEY_1._observer;
				$KEY_1._observer = void 0;
				try {
					let $AP = $KEY_1._cleanup;
					let $AQ;
					if ($AP === void 0) {
						$AQ = $AN;
					} else {
						$KEY_1._cleanup = void 0;
						try {
							$AP();
							$AQ = $AN;
						} catch (e) {
							$AQ = $AN;
						}
					}
					if ($AO) {
						if ($AO.error) {
							let $AR = [$AO.error, $AO.error.call];
							$AR[1].call($AR[0], $AO, $AQ);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_4 = this._subscription;
				let $AN = $KEY_4._observer;
				if ($KEY_4._observer === void 0) {
					return void 0;
				}
				$KEY_4._observer = void 0;
				try {
					if (typeof ($AN ? $AN.complete : void 0) === "function") {
						($AN ? $AN.complete : void 0).call($AN);
					}
				} catch (e) {}
				let $AO, $AP;
				try {
					let $AQ = $KEY_4._cleanup;
					if ($AQ !== void 0) {
						$KEY_4._cleanup = void 0;
						try {
							$AQ();
						} catch (e_2) {}
					}
					$AO = false;
					$AP = void 0;
				} catch (e_3) {
					$AO = true;
					$AP = e_3;
				}
				if ($AO) {
					throw $AP;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}
		let $W = { key: "closed" };
		function $THREAD_2_127749() {
			"use strict";
			return this._subscription._observer === void 0;
		}
		$W.get = $THREAD_2_127749;
		$W = [$W];
		let j = 0;
		for (; j < $W.length; j++) {
			$W[j].enumerable = $W[j].enumerable || false;
			$W[j].configurable = true;
			if ("value" in $W[j]) {
				$W[j].writable = true;
			}
			let $X, $Y;
			if (typeof $W[j].key != "object" ? typeof $W[j].key != "object" : !$W[j].key) {
				$X = typeof $W[j].key == "symbol";
				$Y = $W[j].key;
			} else {
				if ($W[j].key[Symbol.toPrimitive] !== void 0) {
					let $Z = [$W[j].key[Symbol.toPrimitive], $W[j].key[Symbol.toPrimitive].call];
					$Y = $Z[1].call($Z[0], $W[j].key, "string");
					if (typeof $Y == "object") {
						throw new TypeError("@@toPrimitive must return a primitive value.");
					}
				} else {
					$Y = String($W[j].key);
				}
				$X = typeof $Y == "symbol";
			}
			Object.defineProperty($THREAD_8_123240.prototype, $X ? $Y : String($Y), $W[j]);
		}
		Object.defineProperty($THREAD_8_123240, "prototype", { writable: false });
		let $KEY_314 = $THREAD_8_123240;
		function $THREAD_7_63859() {
			class $THREAD_1_27615 {
				constructor($AN) {
					if (typeof $AN !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = $AN;
						return void 0;
					}
				}
				static of() {
					let $KEY_7;
					let $KEY_5 = arguments.length;
					$KEY_7 = new Array($KEY_5);
					let i = 0;
					for (; i < $KEY_5; i++) {
						$KEY_7[i] = arguments[i];
					}
					return new $KEY_2($AN => {
						let $KEY_0 = $AN;
						$KEY_7.forEach($AO => $KEY_0.next($AO));
						$KEY_0.complete();
					});
				}
				static empty() {
					return new $KEY_2($AN => {
						$AN.complete();
					});
				}
				static from($AN, $AO) {
					let $KEY_1 = $AO;
					let $KEY_3 = $AN;
					let $KEY_4;
					if ($AN != null) {
						if ($AN instanceof $KEY_2) {
							return $AN;
						}
						if (typeof Promise !== "undefined" && $AN instanceof Promise) {
							return new $KEY_2($AP => {
								let $KEY_0 = $AP;
								$KEY_3.then($AQ => {
									$KEY_0.next($AQ);
									$KEY_0.complete();
								}).catch($AQ => {
									$KEY_0.error($AQ);
								});
							});
						}
						if (typeof $KEY_3.length === "number" && typeof $KEY_3 !== "function") {
							$KEY_4 = $KEY_3;
							return new $KEY_2($AP => {
								let $KEY_0 = $AP;
								let $KEY_2;
								if ($KEY_1 != null) {
									$KEY_2 = 0;
									$KEY_1.schedule(() => {
										try {
											if ($KEY_2 >= $KEY_4.length) {
												$KEY_0.complete();
												return false;
											} else {
												$KEY_0.next($KEY_4[$KEY_2]);
												++$KEY_2;
												return true;
											}
										} catch (e) {
											$KEY_0.error(e);
											return false;
										}
									});
								} else {
									try {
										let i = 0;
										for (; i < $KEY_4.length; i++) {
											$KEY_0.next($KEY_4[i]);
										}
										$KEY_0.complete();
									} catch (e) {
										$KEY_0.error(e);
									}
								}
							});
						}
						if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_3[Symbol.iterator]) {
							return new $KEY_2($AP => {
								let $KEY_2 = $AP;
								try {
									let $KEY_0 = $KEY_3[Symbol.iterator]();
									if ($KEY_1 != null) {
										$KEY_1.schedule(() => {
											try {
												let $AR = $KEY_0.next();
												if ($AR.done) {
													$KEY_2.complete();
													return false;
												} else {
													$KEY_2.next($AR.value);
													return true;
												}
											} catch (e) {
												$KEY_2.error(e);
												return false;
											}
										});
									} else {
										for (;;) {
											let $AQ = $KEY_0.next();
											if ($AQ.done) break;
											$KEY_2.next($AQ.value);
										}
										$KEY_2.complete();
									}
								} catch (e) {
									$KEY_2.error(e);
								}
							});
						}
					}
					throw new Error("Could not convert " + $KEY_3 + " to observable");
				}
				subscribe($AN, $AO, $AP) {
					let $AQ = {
						start: typeof $AN === "object" ? $AN.start : void 0,
						next: typeof $AN === "function" ? $AN : $AN ? $AN.next : void 0,
						error: void 0,
						complete: void 0
					};
					$AQ.error = typeof $AN === "object" ? $AN.error : $AO;
					$AQ.complete = typeof $AN === "object" ? $AN.complete : $AP;
					if (this._subscriber == null) {
						throw new TypeError();
					} else {
						let $AR = new $KEY_83($AQ);
						if (typeof $AQ.start === "function") {
							let $AS = [$AQ.start, $AQ.start.call];
							$AS[1].call($AS[0], $AQ, $AR);
						}
						let $AT = new $KEY_314($AR);
						try {
							$AR._cleanup = $KEY_316(this._subscriber, $AT);
						} catch (e) {
							$AT = [$AT, $AT.error];
							$AT[1].call($AT[0], e);
						}
						if ($AR._observer === void 0) {
							let $AU = $AR._cleanup;
							if ($AU !== void 0) {
								$AR._cleanup = void 0;
								try {
									$AU();
								} catch (e_2) {}
							}
						}
						return $AR;
					}
				}
				pipe() {
					let $KEY_3 = arguments.length;
					let $AN = new Array($KEY_3);
					let i = 0;
					for (; i < $KEY_3; i++) {
						$AN[i] = arguments[i];
					}
					return $AN.reduce(($AO, $AP) => $AP($AO), this);
				}
				map($AN) {
					return this.pipe($KEY_270($AN));
				}
				unorderedFlatMap($AN) {
					return this.pipe($KEY_201($AN));
				}
			}
			let $KEY_2 = $THREAD_1_27615;
			return $KEY_2;
		}
		$KEY_0 = $THREAD_7_63859();
}
bootstrap();
