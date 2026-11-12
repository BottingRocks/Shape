//                                            Subscription lifecycle with a separate list.
// Both lifecycle classes, private installers, constructor aliases, observable carrier and consumers.
function bootstrap() {
class $THREAD_9_130851 {
			constructor($AI) {
				this._cleanup = void 0;
				this._observer = $AI;
			}
			unsubscribe() {
				if (this.isComplete()) {
					return void 0;
				} else {
					this._observer = void 0;
					let $AI = this._cleanup;
					if ($AI !== void 0) {
						this._cleanup = void 0;
						try {
							$AI();
						} catch (e) {}
					}
					return void 0;
				}
			}
			isComplete() {
				return this._observer === void 0;
			}
		}
		let $R = { key: "closed" };
		function $THREAD_6_12053() {
			"use strict";
			return this._observer === void 0;
		}
		$R.get = $THREAD_6_12053;
		let $KEY_4 = [$R];
		let i = 0;
		for (; i < $KEY_4.length; i++) {
			$KEY_4[i].enumerable = $KEY_4[i].enumerable || false;
			$KEY_4[i].configurable = true;
			if ("value" in $KEY_4[i]) {
				$KEY_4[i].writable = true;
			}
			let $S;
			if (typeof $KEY_4[i].key != "object" || !$KEY_4[i].key) {
				$S = $KEY_4[i].key;
			} else if ($KEY_4[i].key[Symbol.toPrimitive] !== void 0) {
				$S = $KEY_4[i].key[Symbol.toPrimitive].call($KEY_4[i].key, "string");
				if (typeof $S == "object") {
					throw new TypeError("@@toPrimitive must return a primitive value.");
				}
			} else {
				$S = String($KEY_4[i].key);
			}
			Object.defineProperty($THREAD_9_130851.prototype, typeof $S == "symbol" ? $S : String($S), $KEY_4[i]);
		}
		Object.defineProperty($THREAD_9_130851, "prototype", { writable: false });
		let $KEY_213 = $THREAD_9_130851;
		class $THREAD_3_39152 {
			constructor($AI) {
				this._subscription = $AI;
			}
			next($AI) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($AI);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($AI) {
				let $KEY_2 = this._subscription;
				let $AJ = $KEY_2._observer;
				$KEY_2._observer = void 0;
				try {
					let $AK = $KEY_2._cleanup;
					let $AL;
					if ($AK === void 0) {
						$AL = $AI;
					} else {
						$KEY_2._cleanup = void 0;
						try {
							$AK();
							$AL = $AI;
						} catch (e) {
							$AL = $AI;
						}
					}
					if ($AJ) {
						if ($AJ.error) {
							$AJ.error($AL);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_3 = this._subscription;
				let $AI = $KEY_3._observer;
				if ($KEY_3._observer === void 0) {
					return void 0;
				}
				$KEY_3._observer = void 0;
				try {
					if (typeof ($AI ? $AI.complete : void 0) === "function") {
						($AI ? $AI.complete : void 0).call($AI);
					}
				} catch (e) {}
				let $AJ, $AK;
				try {
					let $AL = $KEY_3._cleanup;
					if ($AL !== void 0) {
						$KEY_3._cleanup = void 0;
						try {
							$AL();
						} catch (e_2) {}
					}
					$AJ = void 0;
					$AK = false;
				} catch (e_3) {
					$AJ = e_3;
					$AK = true;
				}
				if ($AK) {
					throw $AJ;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}
		let $T = { key: "closed" };
		function $THREAD_9_131171() {
			"use strict";
			return this._subscription._observer === void 0;
		}
		$T.get = $THREAD_9_131171;
		$T = [$T];
		let j = 0;
		for (; j < $T.length; j++) {
			$T[j].enumerable = $T[j].enumerable || false;
			$T[j].configurable = true;
			if ("value" in $T[j]) {
				$T[j].writable = true;
			}
			let $U;
			if (typeof $T[j].key != "object" ? typeof $T[j].key != "object" : !$T[j].key) {
				$U = $T[j].key;
			} else if ($T[j].key[Symbol.toPrimitive] !== void 0) {
				$U = $T[j].key[Symbol.toPrimitive].call($T[j].key, "string");
				if (typeof $U == "object") {
					throw new TypeError("@@toPrimitive must return a primitive value.");
				}
			} else {
				$U = String($T[j].key);
			}
			Object.defineProperty($THREAD_3_39152.prototype, typeof $U == "symbol" ? $U : String($U), $T[j]);
		}
		Object.defineProperty($THREAD_3_39152, "prototype", { writable: false });
		let $KEY_148 = $THREAD_3_39152;
		function $THREAD_9_102172() {
			class $THREAD_0_25162 {
				constructor($AI) {
					if (typeof $AI !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = $AI;
						return void 0;
					}
				}
				static of() {
					let $KEY_6;
					let $KEY_1 = arguments.length;
					$KEY_6 = new Array($KEY_1);
					let i = 0;
					for (; i < $KEY_1; i++) {
						$KEY_6[i] = arguments[i];
					}
					return new $KEY_0($AI => {
						let $KEY_0 = $AI;
						$KEY_6.forEach($AJ => $KEY_0.next($AJ));
						$KEY_0.complete();
					});
				}
				static empty() {
					return new $KEY_0($AI => {
						$AI.complete();
					});
				}
				static from($AI, $AJ) {
					let $KEY_1 = $AJ;
					let $KEY_2;
					let $KEY_4 = $AI;
					if ($AI != null) {
						if ($AI instanceof $KEY_0) {
							return $AI;
						}
						if (typeof Promise !== "undefined" && $AI instanceof Promise) {
							return new $KEY_0($AK => {
								let $KEY_0 = $AK;
								$KEY_4.then($AL => {
									$KEY_0.next($AL);
									$KEY_0.complete();
								}).catch($AL => {
									$KEY_0.error($AL);
								});
							});
						}
						if (typeof $KEY_4.length === "number" && typeof $KEY_4 !== "function") {
							$KEY_2 = $KEY_4;
							return new $KEY_0($AK => {
								let $KEY_0 = $AK;
								let $KEY_4;
								if ($KEY_1 != null) {
									$KEY_4 = 0;
									$KEY_1.schedule(() => {
										try {
											if ($KEY_4 >= $KEY_2.length) {
												$KEY_0.complete();
												return false;
											} else {
												$KEY_0.next($KEY_2[$KEY_4]);
												++$KEY_4;
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
										for (; i < $KEY_2.length; i++) {
											$KEY_0.next($KEY_2[i]);
										}
										$KEY_0.complete();
									} catch (e) {
										$KEY_0.error(e);
									}
								}
							});
						}
						if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_4[Symbol.iterator]) {
							return new $KEY_0($AK => {
								let $KEY_3 = $AK;
								try {
									let $KEY_0 = $KEY_4[Symbol.iterator]();
									if ($KEY_1 != null) {
										$KEY_1.schedule(() => {
											try {
												let $AM = $KEY_0.next();
												if ($AM.done) {
													$KEY_3.complete();
													return false;
												} else {
													$KEY_3.next($AM.value);
													return true;
												}
											} catch (e) {
												$KEY_3.error(e);
												return false;
											}
										});
									} else {
										for (;;) {
											let $AL = $KEY_0.next();
											if ($AL.done) break;
											$KEY_3.next($AL.value);
										}
										$KEY_3.complete();
									}
								} catch (e) {
									$KEY_3.error(e);
								}
							});
						}
					}
					throw new Error("Could not convert " + $KEY_4 + " to observable");
				}
				subscribe($AI, $AJ, $AK) {
					let $AL = {
						start: typeof $AI === "object" ? $AI.start : void 0,
						next: typeof $AI === "function" ? $AI : $AI ? $AI.next : void 0,
						error: typeof $AI === "object" ? $AI.error : $AJ,
						complete: typeof $AI === "object" ? $AI.complete : $AK
					};
					if (this._subscriber == null) {
						throw new TypeError();
					} else {
						let $AM = new $KEY_213($AL);
						if (typeof $AL.start === "function") {
							$AL.start($AM);
						}
						let $AN = new $KEY_148($AM);
						try {
							$AM._cleanup = $KEY_383(this._subscriber, $AN);
						} catch (e) {
							$AN.error(e);
						}
						if ($AM._observer === void 0) {
							let $AO = $AM._cleanup;
							if ($AO !== void 0) {
								$AM._cleanup = void 0;
								try {
									$AO();
								} catch (e_2) {}
							}
						}
						return $AM;
					}
				}
				pipe() {
					let $KEY_2 = arguments.length;
					let $AI = new Array($KEY_2);
					let i = 0;
					for (; i < $KEY_2; i++) {
						$AI[i] = arguments[i];
					}
					return $AI.reduce(($AJ, $AK) => $AK($AJ), this);
				}
				map($AI) {
					return this.pipe($KEY_191($AI));
				}
				unorderedFlatMap($AI) {
					return this.pipe($KEY_232($AI));
				}
			}
			let $KEY_0 = $THREAD_0_25162;
			return $KEY_0;
		}
		$KEY_196 = $THREAD_9_102172();
}
bootstrap();
