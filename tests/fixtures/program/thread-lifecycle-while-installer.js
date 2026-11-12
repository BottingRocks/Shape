//                                     Subscription lifecycle with a while installer.
// Both lifecycle classes, private installers, constructor aliases, observable carrier and consumers.
function bootstrap() {
class $THREAD_7_59971 {
			constructor($AM) {
				this._cleanup = void 0;
				this._observer = $AM;
			}
			unsubscribe() {
				if (this.isComplete()) {
					return void 0;
				} else {
					this._observer = void 0;
					let $AM = this._cleanup;
					if ($AM !== void 0) {
						this._cleanup = void 0;
						try {
							$AM();
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
		function $THREAD_9_104962() {
			"use strict";
			return this._observer === void 0;
		}
		$R.get = $THREAD_9_104962;
		$R = [$R];
		let $S = 0;
		while ($S < $R.length) {
			$R[$S].enumerable = $R[$S].enumerable || false;
			$R[$S].configurable = true;
			if ("value" in $R[$S]) {
				$R[$S].writable = true;
			}
			let $T;
			if (typeof $R[$S].key != "object" ? typeof $R[$S].key != "object" : !$R[$S].key) {
				$T = $R[$S].key;
			} else if ($R[$S].key[Symbol.toPrimitive] !== void 0) {
				$T = $R[$S].key[Symbol.toPrimitive].call($R[$S].key, "string");
				if (typeof $T == "object") {
					throw new TypeError("@@toPrimitive must return a primitive value.");
				}
			} else {
				$T = String($R[$S].key);
			}
			Object.defineProperty($THREAD_7_59971.prototype, typeof $T == "symbol" ? $T : String($T), $R[$S]);
			++$S;
		}
		Object.defineProperty($THREAD_7_59971, "prototype", { writable: false });
		let $KEY_67 = $THREAD_7_59971;
		class $THREAD_0_106364 {
			constructor($AM) {
				this._subscription = $AM;
			}
			next($AM) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($AM);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($AM) {
				let $KEY_2 = this._subscription;
				let $AN = $KEY_2._observer;
				$KEY_2._observer = void 0;
				try {
					let $AO = $KEY_2._cleanup;
					let $AP;
					if ($AO === void 0) {
						$AP = $AM;
					} else {
						$KEY_2._cleanup = void 0;
						try {
							$AO();
							$AP = $AM;
						} catch (e) {
							$AP = $AM;
						}
					}
					if ($AN) {
						if ($AN.error) {
							$AN.error($AP);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_2 = this._subscription;
				let $AM = $KEY_2._observer;
				if ($KEY_2._observer === void 0) {
					return void 0;
				}
				$KEY_2._observer = void 0;
				try {
					if (typeof ($AM ? $AM.complete : void 0) === "function") {
						($AM ? $AM.complete : void 0).call($AM);
					}
				} catch (e) {}
				let $AN, $AO;
				try {
					let $AP = $KEY_2._cleanup;
					if ($AP !== void 0) {
						$KEY_2._cleanup = void 0;
						try {
							$AP();
						} catch (e_2) {}
					}
					$AN = void 0;
					$AO = false;
				} catch (e_3) {
					$AN = e_3;
					$AO = true;
				}
				if ($AO) {
					throw $AN;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}
		let $U = { key: "closed" };
		function $THREAD_0_122339() {
			"use strict";
			return this._subscription._observer === void 0;
		}
		$U.get = $THREAD_0_122339;
		$U = [$U];
		let $V = 0;
		while ($V < $U.length) {
			$U[$V].enumerable = $U[$V].enumerable || false;
			$U[$V].configurable = true;
			if ("value" in $U[$V]) {
				$U[$V].writable = true;
			}
			let $W;
			if (typeof $U[$V].key != "object" ? typeof $U[$V].key != "object" : !$U[$V].key) {
				$W = $U[$V].key;
			} else if ($U[$V].key[Symbol.toPrimitive] !== void 0) {
				$W = $U[$V].key[Symbol.toPrimitive].call($U[$V].key, "string");
				if (typeof $W == "object") {
					throw new TypeError("@@toPrimitive must return a primitive value.");
				}
			} else {
				$W = String($U[$V].key);
			}
			Object.defineProperty($THREAD_0_106364.prototype, typeof $W == "symbol" ? $W : String($W), $U[$V]);
			++$V;
		}
		Object.defineProperty($THREAD_0_106364, "prototype", { writable: false });
		let $KEY_349 = $THREAD_0_106364;
		function $THREAD_1_103330() {
			class $THREAD_5_67674 {
				constructor($AM) {
					if (typeof $AM !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = $AM;
						return void 0;
					}
				}
				static of() {
					let $KEY_7 = arguments.length;
					let $KEY_4 = new Array($KEY_7);
					let i = 0;
					for (; i < $KEY_7; i++) {
						$KEY_4[i] = arguments[i];
					}
					return new $KEY_1($AM => {
						let $KEY_0 = $AM;
						$KEY_4.forEach($AN => $KEY_0.next($AN));
						$KEY_0.complete();
					});
				}
				static empty() {
					return new $KEY_1($AM => {
						$AM.complete();
					});
				}
				static from($AM, $AN) {
					let $KEY_2 = $AM;
					let $KEY_3;
					let $KEY_4 = $AN;
					if ($AM != null) {
						if ($AM instanceof $KEY_1) {
							return $AM;
						}
						if (typeof Promise !== "undefined" && $AM instanceof Promise) {
							return new $KEY_1($AO => {
								let $KEY_0 = $AO;
								$KEY_2.then($AP => {
									$KEY_0.next($AP);
									$KEY_0.complete();
								}).catch($AP => {
									$KEY_0.error($AP);
								});
							});
						}
						if (typeof $KEY_2.length === "number" && typeof $KEY_2 !== "function") {
							$KEY_3 = $KEY_2;
							return new $KEY_1($AO => {
								let $KEY_1 = $AO;
								let $KEY_2;
								if ($KEY_4 != null) {
									$KEY_2 = 0;
									$KEY_4.schedule(() => {
										try {
											if ($KEY_2 >= $KEY_3.length) {
												$KEY_1.complete();
												return false;
											} else {
												$KEY_1.next($KEY_3[$KEY_2]);
												++$KEY_2;
												return true;
											}
										} catch (e) {
											$KEY_1.error(e);
											return false;
										}
									});
								} else {
									try {
										let i = 0;
										for (; i < $KEY_3.length; i++) {
											$KEY_1.next($KEY_3[i]);
										}
										$KEY_1.complete();
									} catch (e) {
										$KEY_1.error(e);
									}
								}
							});
						}
						if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_2[Symbol.iterator]) {
							return new $KEY_1($AO => {
								let $KEY_0 = $AO;
								let $KEY_3;
								try {
									$KEY_3 = $KEY_2[Symbol.iterator]();
									if ($KEY_4 != null) {
										$KEY_4.schedule(() => {
											try {
												let $AQ = $KEY_3.next();
												if ($AQ.done) {
													$KEY_0.complete();
													return false;
												} else {
													$KEY_0.next($AQ.value);
													return true;
												}
											} catch (e) {
												$KEY_0.error(e);
												return false;
											}
										});
									} else {
										for (;;) {
											let $AP = $KEY_3.next();
											if ($AP.done) break;
											$KEY_0.next($AP.value);
										}
										$KEY_0.complete();
									}
								} catch (e) {
									$KEY_0.error(e);
								}
							});
						}
					}
					throw new Error("Could not convert " + $KEY_2 + " to observable");
				}
				subscribe($AM, $AN, $AO) {
					let $AP = {
						start: typeof $AM === "object" ? $AM.start : void 0,
						next: typeof $AM === "function" ? $AM : $AM ? $AM.next : void 0,
						error: typeof $AM === "object" ? $AM.error : $AN,
						complete: typeof $AM === "object" ? $AM.complete : $AO
					};
					if (this._subscriber == null) {
						throw new TypeError();
					} else {
						let $AQ = new $KEY_67($AP);
						if (typeof $AP.start === "function") {
							$AP.start($AQ);
						}
						let $AR = new $KEY_349($AQ);
						try {
							$AQ._cleanup = $KEY_57(this._subscriber, $AR);
						} catch (e) {
							$AR.error(e);
						}
						if ($AQ._observer === void 0) {
							let $AS = $AQ._cleanup;
							if ($AS !== void 0) {
								$AQ._cleanup = void 0;
								try {
									$AS();
								} catch (e_2) {}
							}
						}
						return $AQ;
					}
				}
				pipe() {
					let $KEY_6 = arguments.length;
					let $AM = new Array($KEY_6);
					let i = 0;
					for (; i < $KEY_6; i++) {
						$AM[i] = arguments[i];
					}
					return $AM.reduce(($AN, $AO) => $AO($AN), this);
				}
				map($AM) {
					return this.pipe($KEY_194($AM));
				}
				unorderedFlatMap($AM) {
					return this.pipe($KEY_315($AM));
				}
			}
			let $KEY_1 = $THREAD_5_67674;
			return $KEY_1;
		}
		$KEY_300 = $THREAD_1_103330();
}
bootstrap();
