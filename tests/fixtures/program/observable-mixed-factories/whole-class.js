function bootstrap(){
const $KEY_95 = class { constructor($BV){ this._observer = $BV; } get closed(){ return this._subscription._observer === void 0; } };
const $KEY_63 = class { constructor($BV){ this._observer = $BV; } error($e){} };
const $KEY_142 = function($a,$b){ return function(){}; };
const $KEY_378 = function($a){ return $a; };
const $KEY_370 = function($a){ return $a; };
function $THREAD_5_1328() {
			class $THREAD_6_110488 {
				constructor($BS) {
					if (typeof $BS !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = $BS;
						return void 0;
					}
				}
				static of() {
					let $KEY_3 = arguments.length;
					let $KEY_4 = new Array($KEY_3);
					let i = 0;
					for (; i < $KEY_3; i++) {
						$KEY_4[i] = arguments[i];
					}
					return new $KEY_0($BS => {
						let $KEY_0 = $BS;
						$KEY_4.forEach($BT => $KEY_0.next($BT));
						$KEY_0.complete();
					});
				}
				static empty() {
					return new $KEY_0($BS => {
						$BS.complete();
					});
				}
				static from($BS, $BT) {
					let $KEY_1 = $BS;
					let $KEY_2;
					let $KEY_3 = $BT;
					if ($BS != null) {
						if ($BS instanceof $KEY_0) {
							return $BS;
						}
						if (typeof Promise !== "undefined" && $BS instanceof Promise) {
							return new $KEY_0($BU => {
								let $KEY_0 = $BU;
								$KEY_1.then($BV => {
									$KEY_0.next($BV);
									$KEY_0.complete();
								}).catch($BV => {
									$KEY_0.error($BV);
								});
							});
						}
						if (typeof $KEY_1.length === "number" && typeof $KEY_1 !== "function") {
							$KEY_2 = $KEY_1;
							return new $KEY_0($BU => {
								let $KEY_4 = $BU;
								if ($KEY_3 != null) {
									let $KEY_0 = 0;
									$KEY_3.schedule(() => {
										try {
											if ($KEY_0 >= $KEY_2.length) {
												$KEY_4.complete();
												return false;
											} else {
												$KEY_4.next($KEY_2[$KEY_0]);
												++$KEY_0;
												return true;
											}
										} catch (e) {
											$KEY_4.error(e);
											return false;
										}
									});
								} else {
									try {
										let $BV = 0;
										while ($BV < $KEY_2.length) {
											$KEY_4.next($KEY_2[$BV]);
											++$BV;
										}
										$KEY_4.complete();
									} catch (e) {
										$KEY_4.error(e);
									}
								}
							});
						}
						if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_1[Symbol.iterator]) {
							return new $KEY_0($BU => {
								let $KEY_4 = $BU;
								try {
									let $KEY_0 = $KEY_1[Symbol.iterator]();
									if ($KEY_3 != null) {
										$KEY_3.schedule(() => {
											try {
												let $BW = $KEY_0.next();
												if ($BW.done) {
													$KEY_4.complete();
													return false;
												} else {
													$KEY_4.next($BW.value);
													return true;
												}
											} catch (e) {
												$KEY_4.error(e);
												return false;
											}
										});
									} else {
										for (;;) {
											let $BV = $KEY_0.next();
											if ($BV.done) break;
											$KEY_4.next($BV.value);
										}
										$KEY_4.complete();
									}
								} catch (e) {
									$KEY_4.error(e);
								}
							});
						}
					}
					throw new Error("Could not convert " + $KEY_1 + " to observable");
				}
				subscribe($BS, $BT, $BU) {
					let $BV = {
						start: typeof $BS === "object" ? $BS.start : void 0,
						next: typeof $BS === "function" ? $BS : $BS ? $BS.next : void 0,
						error: typeof $BS === "object" ? $BS.error : $BT,
						complete: typeof $BS === "object" ? $BS.complete : $BU
					};
					if (this._subscriber == null) {
						throw new TypeError();
					} else {
						let $BW = new $KEY_95($BV);
						if (typeof $BV.start === "function") {
							$BV.start($BW);
						}
						let $BX = new $KEY_63($BW);
						try {
							$BW._cleanup = $KEY_142(this._subscriber, $BX);
						} catch (e) {
							$BX.error(e);
						}
						if ($BW._observer === void 0) {
							let $BY = $BW._cleanup;
							if ($BY !== void 0) {
								$BW._cleanup = void 0;
								try {
									$BY();
								} catch (e_2) {}
							}
						}
						return $BW;
					}
				}
				pipe() {
					let $KEY_0 = arguments.length;
					let $BS = new Array($KEY_0);
					let i = 0;
					for (; i < $KEY_0; i++) {
						$BS[i] = arguments[i];
					}
					return $BS.reduce(($BT, $BU) => $BU($BT), this);
				}
				map($BS) {
					return this.pipe($KEY_378($BS));
				}
				unorderedFlatMap($BS) {
					return this.pipe($KEY_370($BS));
				}
			}
			let $KEY_0 = $THREAD_6_110488;
			return $KEY_0;
		}
return $THREAD_5_1328;
}
bootstrap();