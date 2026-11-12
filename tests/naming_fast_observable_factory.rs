use shape::naming::name_threads;

// Fast-loop family observable-class-factory (`scanlexfam-00105`).
// Canonical existing role: `createObservableClass`. The owner witness is a
// generated zero-formal factory that declares one subscriber-backed class and
// returns it through one immutable alias. The class API is proved from the
// member bodies; class member order is free, statement order inside a member
// is not.
const EVALUATOR: &str = "shared_fast_observable_factory";
const NAME: &str = "createObservableClass";

const POSITIVE: &str = r##"function bootstrap(){
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
bootstrap();"##;

// Every generated/capture spelling changes. The rule may not depend on a seed
// lexeme; only the target keeps a generated THREAD binding so it is eligible
// for renaming.
const ALPHA: &str = r##"function bootstrap(){
const $A_400 = class { constructor($A_401){ this._observer = $A_401; } get closed(){ return this._subscription._observer === void 0; } };
const $A_402 = class { constructor($A_401){ this._observer = $A_401; } error($A_403){} };
const $A_404 = function($A_405,$A_406){ return function(){}; };
const $A_407 = function($A_405){ return $A_405; };
const $A_408 = function($A_405){ return $A_405; };
function $THREAD_900_777_2() {
			class $THREAD_901_888_3 {
				constructor($A_409) {
					if (typeof $A_409 !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = $A_409;
						return void 0;
					}
				}
				static of() {
					let $A_410 = arguments.length;
					let $A_411 = new Array($A_410);
					let i = 0;
					for (; i < $A_410; i++) {
						$A_411[i] = arguments[i];
					}
					return new $A_412($A_409 => {
						let $A_412 = $A_409;
						$A_411.forEach($A_413 => $A_412.next($A_413));
						$A_412.complete();
					});
				}
				static empty() {
					return new $A_412($A_409 => {
						$A_409.complete();
					});
				}
				static from($A_409, $A_413) {
					let $A_414 = $A_409;
					let $A_415;
					let $A_410 = $A_413;
					if ($A_409 != null) {
						if ($A_409 instanceof $A_412) {
							return $A_409;
						}
						if (typeof Promise !== "undefined" && $A_409 instanceof Promise) {
							return new $A_412($A_416 => {
								let $A_412 = $A_416;
								$A_414.then($A_401 => {
									$A_412.next($A_401);
									$A_412.complete();
								}).catch($A_401 => {
									$A_412.error($A_401);
								});
							});
						}
						if (typeof $A_414.length === "number" && typeof $A_414 !== "function") {
							$A_415 = $A_414;
							return new $A_412($A_416 => {
								let $A_411 = $A_416;
								if ($A_410 != null) {
									let $A_412 = 0;
									$A_410.schedule(() => {
										try {
											if ($A_412 >= $A_415.length) {
												$A_411.complete();
												return false;
											} else {
												$A_411.next($A_415[$A_412]);
												++$A_412;
												return true;
											}
										} catch (e) {
											$A_411.error(e);
											return false;
										}
									});
								} else {
									try {
										let $A_401 = 0;
										while ($A_401 < $A_415.length) {
											$A_411.next($A_415[$A_401]);
											++$A_401;
										}
										$A_411.complete();
									} catch (e) {
										$A_411.error(e);
									}
								}
							});
						}
						if (typeof Symbol !== "undefined" && Symbol.iterator && $A_414[Symbol.iterator]) {
							return new $A_412($A_416 => {
								let $A_411 = $A_416;
								try {
									let $A_412 = $A_414[Symbol.iterator]();
									if ($A_410 != null) {
										$A_410.schedule(() => {
											try {
												let $A_417 = $A_412.next();
												if ($A_417.done) {
													$A_411.complete();
													return false;
												} else {
													$A_411.next($A_417.value);
													return true;
												}
											} catch (e) {
												$A_411.error(e);
												return false;
											}
										});
									} else {
										for (;;) {
											let $A_401 = $A_412.next();
											if ($A_401.done) break;
											$A_411.next($A_401.value);
										}
										$A_411.complete();
									}
								} catch (e) {
									$A_411.error(e);
								}
							});
						}
					}
					throw new Error("Could not convert " + $A_414 + " to observable");
				}
				subscribe($A_409, $A_413, $A_416) {
					let $A_401 = {
						start: typeof $A_409 === "object" ? $A_409.start : void 0,
						next: typeof $A_409 === "function" ? $A_409 : $A_409 ? $A_409.next : void 0,
						error: typeof $A_409 === "object" ? $A_409.error : $A_413,
						complete: typeof $A_409 === "object" ? $A_409.complete : $A_416
					};
					if (this._subscriber == null) {
						throw new TypeError();
					} else {
						let $A_417 = new $A_400($A_401);
						if (typeof $A_401.start === "function") {
							$A_401.start($A_417);
						}
						let $A_418 = new $A_402($A_417);
						try {
							$A_417._cleanup = $A_404(this._subscriber, $A_418);
						} catch (e) {
							$A_418.error(e);
						}
						if ($A_417._observer === void 0) {
							let $A_419 = $A_417._cleanup;
							if ($A_419 !== void 0) {
								$A_417._cleanup = void 0;
								try {
									$A_419();
								} catch (e_2) {}
							}
						}
						return $A_417;
					}
				}
				pipe() {
					let $A_412 = arguments.length;
					let $A_409 = new Array($A_412);
					let i = 0;
					for (; i < $A_412; i++) {
						$A_409[i] = arguments[i];
					}
					return $A_409.reduce(($A_413, $A_416) => $A_416($A_413), this);
				}
				map($A_409) {
					return this.pipe($A_407($A_409));
				}
				unorderedFlatMap($A_409) {
					return this.pipe($A_408($A_409));
				}
			}
			let $A_412 = $THREAD_901_888_3;
			return $A_412;
		}
return $THREAD_900_777_2;
}
bootstrap();"##;

// Readable spellings prove the rule follows scoped identities and protocol
// shape, not `$KEY_` names.
const HUMAN: &str = r##"function bootstrap(){
const ObserverHandle = class { constructor(value8){ this._observer = value8; } get closed(){ return this._subscription._observer === void 0; } };
const SubscriptionHandle = class { constructor(value8){ this._observer = value8; } error(value9){} };
const runTeardown = function(value10,value11){ return function(){}; };
const mapOperator = function(value10){ return value10; };
const flatMapOperator = function(value10){ return value10; };
function $THREAD_71_8123() {
			class Observable {
				constructor(value12) {
					if (typeof value12 !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = value12;
						return void 0;
					}
				}
				static of() {
					let value13 = arguments.length;
					let value14 = new Array(value13);
					let i = 0;
					for (; i < value13; i++) {
						value14[i] = arguments[i];
					}
					return new value15(value12 => {
						let value15 = value12;
						value14.forEach(value16 => value15.next(value16));
						value15.complete();
					});
				}
				static empty() {
					return new value15(value12 => {
						value12.complete();
					});
				}
				static from(value12, value16) {
					let value17 = value12;
					let value18;
					let value13 = value16;
					if (value12 != null) {
						if (value12 instanceof value15) {
							return value12;
						}
						if (typeof Promise !== "undefined" && value12 instanceof Promise) {
							return new value15(value19 => {
								let value15 = value19;
								value17.then(value8 => {
									value15.next(value8);
									value15.complete();
								}).catch(value8 => {
									value15.error(value8);
								});
							});
						}
						if (typeof value17.length === "number" && typeof value17 !== "function") {
							value18 = value17;
							return new value15(value19 => {
								let value14 = value19;
								if (value13 != null) {
									let value15 = 0;
									value13.schedule(() => {
										try {
											if (value15 >= value18.length) {
												value14.complete();
												return false;
											} else {
												value14.next(value18[value15]);
												++value15;
												return true;
											}
										} catch (e) {
											value14.error(e);
											return false;
										}
									});
								} else {
									try {
										let value8 = 0;
										while (value8 < value18.length) {
											value14.next(value18[value8]);
											++value8;
										}
										value14.complete();
									} catch (e) {
										value14.error(e);
									}
								}
							});
						}
						if (typeof Symbol !== "undefined" && Symbol.iterator && value17[Symbol.iterator]) {
							return new value15(value19 => {
								let value14 = value19;
								try {
									let value15 = value17[Symbol.iterator]();
									if (value13 != null) {
										value13.schedule(() => {
											try {
												let value20 = value15.next();
												if (value20.done) {
													value14.complete();
													return false;
												} else {
													value14.next(value20.value);
													return true;
												}
											} catch (e) {
												value14.error(e);
												return false;
											}
										});
									} else {
										for (;;) {
											let value8 = value15.next();
											if (value8.done) break;
											value14.next(value8.value);
										}
										value14.complete();
									}
								} catch (e) {
									value14.error(e);
								}
							});
						}
					}
					throw new Error("Could not convert " + value17 + " to observable");
				}
				subscribe(value12, value16, value19) {
					let value8 = {
						start: typeof value12 === "object" ? value12.start : void 0,
						next: typeof value12 === "function" ? value12 : value12 ? value12.next : void 0,
						error: typeof value12 === "object" ? value12.error : value16,
						complete: typeof value12 === "object" ? value12.complete : value19
					};
					if (this._subscriber == null) {
						throw new TypeError();
					} else {
						let value20 = new ObserverHandle(value8);
						if (typeof value8.start === "function") {
							value8.start(value20);
						}
						let value21 = new SubscriptionHandle(value20);
						try {
							value20._cleanup = runTeardown(this._subscriber, value21);
						} catch (e) {
							value21.error(e);
						}
						if (value20._observer === void 0) {
							let value22 = value20._cleanup;
							if (value22 !== void 0) {
								value20._cleanup = void 0;
								try {
									value22();
								} catch (e_2) {}
							}
						}
						return value20;
					}
				}
				pipe() {
					let value15 = arguments.length;
					let value12 = new Array(value15);
					let i = 0;
					for (; i < value15; i++) {
						value12[i] = arguments[i];
					}
					return value12.reduce((value16, value19) => value19(value16), this);
				}
				map(value12) {
					return this.pipe(mapOperator(value12));
				}
				unorderedFlatMap(value12) {
					return this.pipe(flatMapOperator(value12));
				}
			}
			let value15 = Observable;
			return value15;
		}
return $THREAD_71_8123;
}
bootstrap();"##;

// Pure class members may be emitted in any order.
const ORDER: &str = r##"function bootstrap(){
const $KEY_95 = class { constructor($BV){ this._observer = $BV; } get closed(){ return this._subscription._observer === void 0; } };
const $KEY_63 = class { constructor($BV){ this._observer = $BV; } error($e){} };
const $KEY_142 = function($a,$b){ return function(){}; };
const $KEY_378 = function($a){ return $a; };
const $KEY_370 = function($a){ return $a; };
function $THREAD_5_1328() {
			class $THREAD_6_110488 {
				map($BS) {
					return this.pipe($KEY_378($BS));
				}
				unorderedFlatMap($BS) {
					return this.pipe($KEY_370($BS));
				}
				constructor($BS) {
					if (typeof $BS !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = $BS;
						return void 0;
					}
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
				pipe() {
					let $KEY_0 = arguments.length;
					let $BS = new Array($KEY_0);
					let i = 0;
					for (; i < $KEY_0; i++) {
						$BS[i] = arguments[i];
					}
					return $BS.reduce(($BT, $BU) => $BU($BT), this);
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
			}
			let $KEY_0 = $THREAD_6_110488;
			return $KEY_0;
		}
return $THREAD_5_1328;
}
bootstrap();"##;

// The from() protocol branches are meaningful and must stay ordered.
const BRANCH_SWAP: &str = r##"function bootstrap(){
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
bootstrap();"##;

fn target_outcome<'a>(
    result: &'a shape::naming::NamedThreads,
    target: &str,
) -> &'a shape::naming::ThreadName {
    result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes"))
}

fn named(source: &str, target: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        let outcome = target_outcome(&result, target);
        outcome.applied
            && outcome.proposed_name.as_deref() == Some(NAME)
            && outcome.evaluator.as_deref() == Some(EVALUATOR)
    })
}

fn our_claim(source: &str, target: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        let outcome = target_outcome(&result, target);
        outcome.evaluator.as_deref() == Some(EVALUATOR)
    })
}

fn parses(source: &str) -> bool {
    name_threads(source, true).is_ok()
}

fn valid_model(source: &str) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = shape::analysis::parse(&allocator, source);
    assert!(!parsed.panicked && parsed.errors.is_empty(), "fixture did not parse: {:?}", parsed.errors);
    // This public inventory entry constructs the same Model before running
    // collector inventory; it does not invoke the Observable matcher.
    shape::naming::collectors::identify_collectors(source, false)
        .expect("fixture must build a valid Model before matcher evaluation");
}

fn denied(source: &str, target: &str) {
    valid_model(source);
    assert!(!named(source, target), "unexpected {NAME} for: {source}");
    assert!(!our_claim(source, target), "unexpected fast claim for: {source}");
}

fn refuses(template: &str, target: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        denied(&template.replace(from, to), target);
    }
}

fn varied(template: &str, changes: &[(&str, &str)]) -> String {
    let mut source = template.to_string();
    for &(from, to) in changes {
        assert!(source.contains(from), "missing replacement {from}");
        source = source.replace(from, to);
    }
    source
}

// The hoisted array local: `let arr;` then `arr = new Array(len)`.
const HOISTED_OF: &[(&str, &str)] = &[
    (
        "let $KEY_4 = new Array($KEY_3);",
        "$KEY_4 = new Array($KEY_3);",
    ),
    (
        "let $KEY_3 = arguments.length;",
        "let $KEY_4; let $KEY_3 = arguments.length;",
    ),
];

fn hoisted() -> String {
    varied(POSITIVE, HOISTED_OF)
}

// The unscheduled array-like copy lowered as `for (; i < n; i++)`.
fn for_loop() -> String {
    varied(
        POSITIVE,
        &[(
            "while ($BV < $KEY_2.length) {",
            "for (; $BV < $KEY_2.length; $BV++) {",
        )],
    )
}

// The pre-declared index/iterator local plus the assignment forms, combined
// with the for-loop copy.
fn predeclared() -> String {
    varied(
        &for_loop(),
        &[
            (
                "let $KEY_4 = $BU;\n\t\t\t\t\t\t\t\tif ($KEY_3 != null) {",
                "let $KEY_4 = $BU;\n\t\t\t\t\t\t\t\tlet $KEY_0;\n\t\t\t\t\t\t\t\tif ($KEY_3 != null) {",
            ),
            (
                "let $KEY_4 = $BU;\n\t\t\t\t\t\t\t\ttry {",
                "let $KEY_4 = $BU;\n\t\t\t\t\t\t\t\tlet $KEY_0;\n\t\t\t\t\t\t\t\ttry {",
            ),
            ("let $KEY_0 = 0;", "$KEY_0 = 0;"),
            (
                "let $KEY_0 = $KEY_1[Symbol.iterator]();",
                "$KEY_0 = $KEY_1[Symbol.iterator]();",
            ),
        ],
    )
}

// The tracer bundled method call for `start` and the teardown error.
const BUNDLED_SUBSCRIBE: &[(&str, &str)] = &[
    (
        "$BV.start($BW);",
        "let $BND = [$BV.start, $BV.start.call]; $BND[1].call($BND[0], $BV, $BW);",
    ),
    (
        "$BX.error(e);",
        "$BX = [$BX, $BX.error]; $BX[1].call($BX[0], e);",
    ),
];

fn bundled() -> String {
    varied(POSITIVE, BUNDLED_SUBSCRIBE)
}

// The normalized observer is a placeholder literal filled by same-handle writes.
const POSTLITERAL_OBSERVER: &[(&str, &str)] = &[
    (
        r#"next: typeof $BS === "function" ? $BS : $BS ? $BS.next : void 0,"#,
        "next: void 0,",
    ),
    (
        r#"error: typeof $BS === "object" ? $BS.error : $BT,"#,
        "error: void 0,",
    ),
    (
        r#"complete: typeof $BS === "object" ? $BS.complete : $BU"#,
        "complete: void 0",
    ),
    (
        "if (this._subscriber == null) {",
        r#"$BV.next = typeof $BS === "function" ? $BS : $BS ? $BS.next : void 0; $BV.error = typeof $BS === "object" ? $BS.error : $BT; $BV.complete = typeof $BS === "object" ? $BS.complete : $BU; if (this._subscriber == null) {"#,
    ),
];

fn postliteral() -> String {
    varied(POSITIVE, POSTLITERAL_OBSERVER)
}

// Complete observable-factory witness. Surrounding declarations provide only the captured
// callable origins needed by the source-backed proof. The JavaScript is inert.
const FULL_OPERATOR_FACTORY: &str = r##"function bootstrap() {
const $KEY_128 = class {};
const $KEY_134 = class {};
const $KEY_410 = function ($a, $b) {};
const $KEY_361 = function ($a) {};
const $KEY_24 = function ($a) {};
function $THREAD_0_47191() {
    class $THREAD_6_148009 {
        constructor($BR) {
            if (typeof $BR !== "function") {
                throw new TypeError();
            } else {
                this._subscriber = $BR;
                return void 0;
            }
        }
        static of() {
            let $KEY_7 = arguments.length;
            let $KEY_3 = new Array($KEY_7);
            let i = 0;
            for (; i < $KEY_7; i++) {
                $KEY_3[i] = arguments[i];
            }
            return new $KEY_0($BR => {
                let $KEY_0 = $BR;
                $KEY_3.forEach($BS => $KEY_0.next($BS));
                $KEY_0.complete();
            });
        }
        static empty() {
            return new $KEY_0($BR => {
                $BR.complete();
            });
        }
        static from($BR, $BS) {
            let $KEY_1 = $BS;
            let $KEY_4;
            let $KEY_6 = $BR;
            if ($BR != null) {
                if ($BR instanceof $KEY_0) {
                    return $BR;
                }
                if (typeof Promise !== "undefined" && $BR instanceof Promise) {
                    return new $KEY_0($BT => {
                        let $KEY_0 = $BT;
                        $KEY_6.then($BU => {
                            $KEY_0.next($BU);
                            $KEY_0.complete();
                        }).catch($BU => {
                            $KEY_0.error($BU);
                        });
                    });
                }
                if (typeof $KEY_6.length === "number" && typeof $KEY_6 !== "function") {
                    $KEY_4 = $KEY_6;
                    return new $KEY_0($BT => {
                        let $KEY_2 = $BT;
                        if ($KEY_1 != null) {
                            let $KEY_0 = 0;
                            $KEY_1.schedule(() => {
                                try {
                                    if ($KEY_0 >= $KEY_4.length) {
                                        $KEY_2.complete();
                                        return false;
                                    } else {
                                        $KEY_2.next($KEY_4[$KEY_0]);
                                        ++$KEY_0;
                                        return true;
                                    }
                                } catch (e) {
                                    $KEY_2.error(e);
                                    return false;
                                }
                            });
                        } else {
                            try {
                                let $BU = 0;
                                while ($BU < $KEY_4.length) {
                                    $KEY_2.next($KEY_4[$BU]);
                                    ++$BU;
                                }
                                $KEY_2.complete();
                            } catch (e) {
                                $KEY_2.error(e);
                            }
                        }
                    });
                }
                if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_6[Symbol.iterator]) {
                    return new $KEY_0($BT => {
                        let $KEY_2 = $BT;
                        try {
                            let $KEY_0 = $KEY_6[Symbol.iterator]();
                            if ($KEY_1 != null) {
                                $KEY_1.schedule(() => {
                                    try {
                                        let $BV = $KEY_0.next();
                                        if ($BV.done) {
                                            $KEY_2.complete();
                                            return false;
                                        } else {
                                            $KEY_2.next($BV.value);
                                            return true;
                                        }
                                    } catch (e) {
                                        $KEY_2.error(e);
                                        return false;
                                    }
                                });
                            } else {
                                for (;;) {
                                    let $BU = $KEY_0.next();
                                    if ($BU.done) break;
                                    $KEY_2.next($BU.value);
                                }
                                $KEY_2.complete();
                            }
                        } catch (e) {
                            $KEY_2.error(e);
                        }
                    });
                }
            }
            throw new Error("Could not convert " + $KEY_6 + " to observable");
        }
        subscribe($BR, $BS, $BT) {
            let $BU = {
                start: typeof $BR === "object" ? $BR.start : void 0,
                next: typeof $BR === "function" ? $BR : $BR ? $BR.next : void 0,
                error: typeof $BR === "object" ? $BR.error : $BS,
                complete: typeof $BR === "object" ? $BR.complete : $BT
            };
            if (this._subscriber == null) {
                throw new TypeError();
            } else {
                let $BV = new $KEY_128($BU);
                if (typeof $BU.start === "function") {
                    let $BW = [$BU.start, $BU.start.call];
                    $BW[1].call($BW[0], $BU, $BV);
                }
                let $BX = new $KEY_134($BV);
                try {
                    $BV._cleanup = $KEY_410(this._subscriber, $BX);
                } catch (e) {
                    $BX = [$BX, $BX.error];
                    $BX[1].call($BX[0], e);
                }
                if ($BV._observer === void 0) {
                    let $BY = $BV._cleanup;
                    if ($BY !== void 0) {
                        $BV._cleanup = void 0;
                        try {
                            $BY();
                        } catch (e_2) {}
                    }
                }
                return $BV;
            }
        }
        pipe() {
            let $KEY_3 = arguments.length;
            let $BR = new Array($KEY_3);
            let i = 0;
            for (; i < $KEY_3; i++) {
                $BR[i] = arguments[i];
            }
            return $BR.reduce(($BS, $BT) => $BT($BS), this);
        }
        map($BR) {
            return this.pipe($KEY_361($BR));
        }
        unorderedFlatMap($BR) {
            return this.pipe($KEY_24($BR));
        }
    }
    let $KEY_0 = $THREAD_6_148009;
    return $KEY_0;
}
return $THREAD_0_47191;
}
bootstrap();"##;


// Verbatim observable class body with only its captured callable origins.
const TARGET_ACTUAL: &str = r##"function bootstrap() {
const $KEY_376 = class {};
const $KEY_275 = class {};
const $KEY_165 = function ($a, $b) {};
const $KEY_294 = function ($a) {};
const $KEY_29 = function ($a) {};
function $THREAD_3_141537() {
			class $THREAD_5_15575 {
				constructor($AP) {
					if (typeof $AP !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = $AP;
						return void 0;
					}
				}
				static of() {
					let $KEY_2 = arguments.length;
					let $KEY_4 = new Array($KEY_2);
					let i = 0;
					for (; i < $KEY_2; i++) {
						$KEY_4[i] = arguments[i];
					}
					return new $KEY_1($AP => {
						let $KEY_0 = $AP;
						$KEY_4.forEach($AQ => $KEY_0.next($AQ));
						$KEY_0.complete();
					});
				}
				static empty() {
					return new $KEY_1($AP => {
						$AP.complete();
					});
				}
				static from($AP, $AQ) {
					let $KEY_3 = $AQ;
					let $KEY_4 = $AP;
					if ($AP != null) {
						if ($AP instanceof $KEY_1) {
							return $AP;
						}
						if (typeof Promise !== "undefined" && $AP instanceof Promise) {
							return new $KEY_1($AR => {
								let $KEY_0 = $AR;
								$KEY_4.then($AS => {
									$KEY_0.next($AS);
									$KEY_0.complete();
								}).catch($AS => {
									$KEY_0.error($AS);
								});
							});
						}
						if (typeof $KEY_4.length === "number" && typeof $KEY_4 !== "function") {
							let $KEY_0 = $KEY_4;
							return new $KEY_1($AR => {
								let $KEY_2 = $AR;
								if ($KEY_3 != null) {
									let $KEY_1 = 0;
									$KEY_3.schedule(() => {
										try {
											if ($KEY_1 >= $KEY_0.length) {
												$KEY_2.complete();
												return false;
											} else {
												$KEY_2.next($KEY_0[$KEY_1]);
												++$KEY_1;
												return true;
											}
										} catch (e) {
											$KEY_2.error(e);
											return false;
										}
									});
								} else {
									try {
										let i = 0;
										for (; i < $KEY_0.length; i++) {
											$KEY_2.next($KEY_0[i]);
										}
										$KEY_2.complete();
									} catch (e) {
										$KEY_2.error(e);
									}
								}
							});
						}
						if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_4[Symbol.iterator]) {
							return new $KEY_1($AR => {
								let $KEY_2 = $AR;
								let $KEY_6;
								try {
									$KEY_6 = $KEY_4[Symbol.iterator]();
									if ($KEY_3 != null) {
										$KEY_3.schedule(() => {
											try {
												let $AT = $KEY_6.next();
												if ($AT.done) {
													$KEY_2.complete();
													return false;
												} else {
													$KEY_2.next($AT.value);
													return true;
												}
											} catch (e) {
												$KEY_2.error(e);
												return false;
											}
										});
									} else {
										for (;;) {
											let $AS = $KEY_6.next();
											if ($AS.done) break;
											$KEY_2.next($AS.value);
										}
										$KEY_2.complete();
									}
								} catch (e) {
									$KEY_2.error(e);
								}
							});
						}
					}
					throw new Error("Could not convert " + $KEY_4 + " to observable");
				}
				subscribe($AP, $AQ, $AR) {
					let $AS;
					if (typeof $AP === "function") {
						$AS = $AP;
					} else {
						$AS = $AP ? $AP.next : void 0;
					}
					let $AT = {
						start: typeof $AP === "object" ? $AP.start : void 0,
						next: void 0,
						error: void 0,
						complete: void 0
					};
					$AT.next = $AS;
					$AT.error = typeof $AP === "object" ? $AP.error : $AQ;
					$AT.complete = typeof $AP === "object" ? $AP.complete : $AR;
					if (this._subscriber == null) {
						throw new TypeError();
					} else {
						let $AU = new $KEY_376($AT);
						if (typeof $AT.start === "function") {
							$AT.start($AU);
						}
						let $AV = new $KEY_275($AU);
						try {
							$AU._cleanup = $KEY_165(this._subscriber, $AV);
						} catch (e) {
							$AV.error(e);
						}
						if ($AU._observer === void 0) {
							let $AW = $AU._cleanup;
							if ($AW !== void 0) {
								$AU._cleanup = void 0;
								try {
									$AW();
								} catch (e_2) {}
							}
						}
						return $AU;
					}
				}
				pipe() {
					let $KEY_1 = arguments.length;
					let $AP = new Array($KEY_1);
					let i = 0;
					for (; i < $KEY_1; i++) {
						$AP[i] = arguments[i];
					}
					return $AP.reduce(($AQ, $AR) => $AR($AQ), this);
				}
				map($AP) {
					return this.pipe($KEY_294($AP));
				}
				unorderedFlatMap($AP) {
					return this.pipe($KEY_29($AP));
				}
			}
			let $KEY_1 = $THREAD_5_15575;
			return $KEY_1;
		}
return $THREAD_3_141537;
}
bootstrap();"##;

// Bounded observable-factory witness. The wrapper supplies only the captured callable
// origins required by the structural proof; the class body is unchanged.
const GUARDED_CONSTRUCTOR: &str = r##"function bootstrap() {
const $KEY_118 = class {};
const $KEY_256 = class {};
const $KEY_312 = function ($a, $b) {};
const $KEY_311 = function ($a) {};
const $KEY_56 = function ($a) {};
function $THREAD_6_100381() {
			class $THREAD_8_105355 {
				constructor($BR) {
					if (typeof $BR !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = $BR;
						return void 0;
					}
				}
				static of() {
					let $KEY_3;
					let $KEY_0 = arguments.length;
					$KEY_3 = new Array($KEY_0);
					let i = 0;
					for (; i < $KEY_0; i++) {
						$KEY_3[i] = arguments[i];
					}
					return new $KEY_1($BR => {
						let $KEY_0 = $BR;
						$KEY_3.forEach($BS => $KEY_0.next($BS));
						$KEY_0.complete();
					});
				}
				static empty() {
					return new $KEY_1($BR => {
						$BR.complete();
					});
				}
				static from($BR, $BS) {
					let $KEY_2 = $BR;
					let $KEY_4 = $BS;
					if ($BR != null) {
						if ($BR instanceof $KEY_1) {
							return $BR;
						}
						if (typeof Promise !== "undefined" && $BR instanceof Promise) {
							return new $KEY_1($BT => {
								let $KEY_0 = $BT;
								$KEY_2.then($BU => {
									$KEY_0.next($BU);
									$KEY_0.complete();
								}).catch($BU => {
									$KEY_0.error($BU);
								});
							});
						}
						if (typeof $KEY_2.length === "number" && typeof $KEY_2 !== "function") {
							let $KEY_0 = $KEY_2;
							return new $KEY_1($BT => {
								let $KEY_3 = $BT;
								if ($KEY_4 != null) {
									let $KEY_2 = 0;
									$KEY_4.schedule(() => {
										try {
											if ($KEY_2 >= $KEY_0.length) {
												$KEY_3.complete();
												return false;
											} else {
												$KEY_3.next($KEY_0[$KEY_2]);
												++$KEY_2;
												return true;
											}
										} catch (e) {
											$KEY_3.error(e);
											return false;
										}
									});
								} else {
									try {
										let $BU = 0;
										while ($BU < $KEY_0.length) {
											$KEY_3.next($KEY_0[$BU]);
											++$BU;
										}
										$KEY_3.complete();
									} catch (e) {
										$KEY_3.error(e);
									}
								}
							});
						}
						if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_2[Symbol.iterator]) {
							return new $KEY_1($BT => {
								let $KEY_1 = $BT;
								let $KEY_3;
								try {
									$KEY_3 = $KEY_2[Symbol.iterator]();
									if ($KEY_4 != null) {
										$KEY_4.schedule(() => {
											try {
												let $BV = $KEY_3.next();
												if ($BV.done) {
													$KEY_1.complete();
													return false;
												} else {
													$KEY_1.next($BV.value);
													return true;
												}
											} catch (e) {
												$KEY_1.error(e);
												return false;
											}
										});
									} else {
										for (;;) {
											let $BU = $KEY_3.next();
											if ($BU.done) break;
											$KEY_1.next($BU.value);
										}
										$KEY_1.complete();
									}
								} catch (e) {
									$KEY_1.error(e);
								}
							});
						}
					}
					throw new Error("Could not convert " + $KEY_2 + " to observable");
				}
				subscribe($BR, $BS, $BT) {
					let $BU = {
						start: typeof $BR === "object" ? $BR.start : void 0,
						next: typeof $BR === "function" ? $BR : $BR ? $BR.next : void 0,
						error: typeof $BR === "object" ? $BR.error : $BS,
						complete: typeof $BR === "object" ? $BR.complete : $BT
					};
					if (this._subscriber == null) {
						throw new TypeError();
					} else {
						let $BV = new $KEY_118($BU);
						if (typeof $BU.start === "function") {
							$BU.start($BV);
						}
						let $BW = new $KEY_256($BV);
						try {
							$BV._cleanup = $KEY_312(this._subscriber, $BW);
						} catch (e) {
							$BW.error(e);
						}
						if ($BV._observer === void 0) {
							let $BX = $BV._cleanup;
							if ($BX !== void 0) {
								$BV._cleanup = void 0;
								try {
									$BX();
								} catch (e_2) {}
							}
						}
						return $BV;
					}
				}
				pipe() {
					let $KEY_1 = arguments.length;
					let $BR = new Array($KEY_1);
					let i = 0;
					for (; i < $KEY_1; i++) {
						$BR[i] = arguments[i];
					}
					return $BR.reduce(($BS, $BT) => $BT($BS), this);
				}
				map($BR) {
					return this.pipe($KEY_311($BR));
				}
				unorderedFlatMap($BR) {
					return this.pipe($KEY_56($BR));
				}
			}
			let $KEY_1 = $THREAD_8_105355;
			return $KEY_1;
		}
return $THREAD_6_100381;
}
bootstrap();"##;


// The subscribe member uses a six-statement, two-slot split: a next prelude (`let n; if
// (typeof o === "function") …`), a complete prelude (`let c; if (typeof o === "object")
// …`), then the resolved observer literal assigned back to the next slot. The carrier
// supplies only captured callable origins.
const SPLIT_OBSERVER_PRELUDE: &str = r##"function bootstrap() {
const $KEY_221 = class {};
const $KEY_41 = class {};
const $KEY_244 = function ($a, $b) {};
const $KEY_303 = function ($a) {};
const $KEY_301 = function ($a) {};
function $THREAD_1_126926() {
					class $THREAD_1_12528 {
						constructor($BP) {
							if (typeof $BP !== "function") {
								throw new TypeError();
							} else {
								this._subscriber = $BP;
								return void 0;
							}
						}
						static of() {
							let $KEY_6;
							let $KEY_5 = arguments.length;
							$KEY_6 = new Array($KEY_5);
							let i = 0;
							for (; i < $KEY_5; i++) {
								$KEY_6[i] = arguments[i];
							}
							return new $KEY_0($BP => {
								let $KEY_0 = $BP;
								$KEY_6.forEach($BQ => $KEY_0.next($BQ));
								$KEY_0.complete();
							});
						}
						static empty() {
							return new $KEY_0($BP => {
								$BP.complete();
							});
						}
						static from($BP, $BQ) {
							let $KEY_1 = $BP;
							let $KEY_3;
							let $KEY_4 = $BQ;
							if ($BP != null) {
								if ($BP instanceof $KEY_0) {
									return $BP;
								}
								if (typeof Promise !== "undefined" && $BP instanceof Promise) {
									return new $KEY_0($BR => {
										let $KEY_0 = $BR;
										$KEY_1.then($BS => {
											$KEY_0.next($BS);
											$KEY_0.complete();
										}).catch($BS => {
											$KEY_0.error($BS);
										});
									});
								}
								if (typeof $KEY_1.length === "number" && typeof $KEY_1 !== "function") {
									$KEY_3 = $KEY_1;
									return new $KEY_0($BR => {
										let $KEY_2 = $BR;
										if ($KEY_4 != null) {
											let $KEY_0 = 0;
											$KEY_4.schedule(() => {
												try {
													if ($KEY_0 >= $KEY_3.length) {
														$KEY_2.complete();
														return false;
													} else {
														$KEY_2.next($KEY_3[$KEY_0]);
														++$KEY_0;
														return true;
													}
												} catch (e) {
													$KEY_2.error(e);
													return false;
												}
											});
										} else {
											try {
												let i = 0;
												for (; i < $KEY_3.length; i++) {
													$KEY_2.next($KEY_3[i]);
												}
												$KEY_2.complete();
											} catch (e) {
												$KEY_2.error(e);
											}
										}
									});
								}
								if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_1[Symbol.iterator]) {
									return new $KEY_0($BR => {
										let $KEY_3 = $BR;
										try {
											let $KEY_0 = $KEY_1[Symbol.iterator]();
											if ($KEY_4 != null) {
												$KEY_4.schedule(() => {
													try {
														let $BT = $KEY_0.next();
														if ($BT.done) {
															$KEY_3.complete();
															return false;
														} else {
															$KEY_3.next($BT.value);
															return true;
														}
													} catch (e) {
														$KEY_3.error(e);
														return false;
													}
												});
											} else {
												for (;;) {
													let $BS = $KEY_0.next();
													if ($BS.done) break;
													$KEY_3.next($BS.value);
												}
												$KEY_3.complete();
											}
										} catch (e) {
											$KEY_3.error(e);
										}
									});
								}
							}
							throw new Error("Could not convert " + $KEY_1 + " to observable");
						}
						subscribe($BP, $BQ, $BR) {
							let $BS;
							if (typeof $BP === "function") {
								$BS = $BP;
							} else {
								$BS = $BP ? $BP.next : void 0;
							}
							let $BT;
							if (typeof $BP === "object") {
								$BT = $BP.complete;
							} else {
								$BT = $BR;
							}
							$BS = {
								start: typeof $BP === "object" ? $BP.start : void 0,
								next: $BS,
								error: typeof $BP === "object" ? $BP.error : $BQ,
								complete: $BT
							};
							if (this._subscriber == null) {
								throw new TypeError();
							} else {
								let $BU = new $KEY_221($BS);
								if (typeof $BS.start === "function") {
									$BS.start($BU);
								}
								let $BV = new $KEY_41($BU);
								try {
									$BU._cleanup = $KEY_244(this._subscriber, $BV);
								} catch (e) {
									$BV.error(e);
								}
								if ($BU._observer === void 0) {
									let $BW = $BU._cleanup;
									if ($BW !== void 0) {
										$BU._cleanup = void 0;
										try {
											$BW();
										} catch (e_2) {}
									}
								}
								return $BU;
							}
						}
						pipe() {
							let $KEY_2 = arguments.length;
							let $BP = new Array($KEY_2);
							let i = 0;
							for (; i < $KEY_2; i++) {
								$BP[i] = arguments[i];
							}
							return $BP.reduce(($BQ, $BR) => $BR($BQ), this);
						}
						map($BP) {
							return this.pipe($KEY_303($BP));
						}
						unorderedFlatMap($BP) {
							return this.pipe($KEY_301($BP));
						}
					}
					let $KEY_0 = $THREAD_1_12528;
					return $KEY_0;
				}
return $THREAD_1_126926;
}
bootstrap();"##;

// The pipe member copies `arguments` with its index declared in the loop head (`for (let i
// = 0; i < n; i++)`); of keeps a separate pre-loop index. Both must prove the same owned
// copy, with only captured callable origins supplied by the carrier.
const INLINE_ARGUMENT_INDEX: &str = r##"function bootstrap() {
const $KEY_376 = class {};
const $KEY_221 = class {};
const $KEY_120 = function ($a, $b) {};
const $KEY_105 = function ($a) {};
const $KEY_258 = function ($a) {};
function $THREAD_6_96922() {
			class $THREAD_2_157798 {
				constructor($BR) {
					if (typeof $BR !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = $BR;
						return void 0;
					}
				}
				static of() {
					let $KEY_6;
					let $KEY_5 = arguments.length;
					$KEY_6 = new Array($KEY_5);
					let i = 0;
					for (; i < $KEY_5; i++) {
						$KEY_6[i] = arguments[i];
					}
					return new $KEY_0($BR => {
						let $KEY_0 = $BR;
						$KEY_6.forEach($BS => $KEY_0.next($BS));
						$KEY_0.complete();
					});
				}
				static empty() {
					return new $KEY_0($BR => {
						$BR.complete();
					});
				}
				static from($BR, $BS) {
					let $KEY_5 = $BS;
					let $KEY_7 = $BR;
					let $KEY_8;
					if ($BR != null) {
						if ($BR instanceof $KEY_0) {
							return $BR;
						}
						if (typeof Promise !== "undefined" && $BR instanceof Promise) {
							return new $KEY_0($BT => {
								let $KEY_0 = $BT;
								$KEY_7.then($BU => {
									$KEY_0.next($BU);
									$KEY_0.complete();
								}).catch($BU => {
									$KEY_0.error($BU);
								});
							});
						}
						if (typeof $KEY_7.length === "number" && typeof $KEY_7 !== "function") {
							$KEY_8 = $KEY_7;
							return new $KEY_0($BT => {
								let $KEY_0 = $BT;
								let $KEY_2;
								if ($KEY_5 != null) {
									$KEY_2 = 0;
									$KEY_5.schedule(() => {
										try {
											if ($KEY_2 >= $KEY_8.length) {
												$KEY_0.complete();
												return false;
											} else {
												$KEY_0.next($KEY_8[$KEY_2]);
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
										for (; i < $KEY_8.length; i++) {
											$KEY_0.next($KEY_8[i]);
										}
										$KEY_0.complete();
									} catch (e) {
										$KEY_0.error(e);
									}
								}
							});
						}
						if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_7[Symbol.iterator]) {
							return new $KEY_0($BT => {
								let $KEY_1 = $BT;
								try {
									let $KEY_0 = $KEY_7[Symbol.iterator]();
									if ($KEY_5 != null) {
										$KEY_5.schedule(() => {
											try {
												let $BV = $KEY_0.next();
												if ($BV.done) {
													$KEY_1.complete();
													return false;
												} else {
													$KEY_1.next($BV.value);
													return true;
												}
											} catch (e) {
												$KEY_1.error(e);
												return false;
											}
										});
									} else {
										for (;;) {
											let $BU = $KEY_0.next();
											if ($BU.done) break;
											$KEY_1.next($BU.value);
										}
										$KEY_1.complete();
									}
								} catch (e) {
									$KEY_1.error(e);
								}
							});
						}
					}
					throw new Error("Could not convert " + $KEY_7 + " to observable");
				}
				subscribe($BR, $BS, $BT) {
					let $BU = {
						start: typeof $BR === "object" ? $BR.start : void 0,
						next: typeof $BR === "function" ? $BR : $BR ? $BR.next : void 0,
						error: typeof $BR === "object" ? $BR.error : $BS,
						complete: typeof $BR === "object" ? $BR.complete : $BT
					};
					if (this._subscriber == null) {
						throw new TypeError();
					} else {
						let $BV = new $KEY_376($BU);
						if (typeof $BU.start === "function") {
							let $BW = [$BU.start, $BU.start.call];
							$BW[1].call($BW[0], $BU, $BV);
						}
						let $BX = new $KEY_221($BV);
						try {
							$BV._cleanup = $KEY_120(this._subscriber, $BX);
						} catch (e) {
							$BX = [$BX, $BX.error];
							$BX[1].call($BX[0], e);
						}
						if ($BV._observer === void 0) {
							let $BY = $BV._cleanup;
							if ($BY !== void 0) {
								$BV._cleanup = void 0;
								try {
									$BY();
								} catch (e_2) {}
							}
						}
						return $BV;
					}
				}
				pipe() {
					let $KEY_4 = arguments.length;
					let $BR = new Array($KEY_4);
					for (let i = 0; i < $KEY_4; i++) {
						$BR[i] = arguments[i];
					}
					return $BR.reduce(($BS, $BT) => $BT($BS), this);
				}
				map($BR) {
					return this.pipe($KEY_105($BR));
				}
				unorderedFlatMap($BR) {
					return this.pipe($KEY_258($BR));
				}
			}
			let $KEY_0 = $THREAD_2_157798;
			return $KEY_0;
		}
return $THREAD_6_96922;
}
bootstrap();"##;

// Subscribe resolves the object-form prelude as the `error` member, falling back to the
// error argument, and keeps an inline `complete` conditional. The next slot comes from the
// function-form prelude. Only captured callable origins are supplied by the carrier.
const ERROR_PRELUDE_ACTUAL: &str = r##"function bootstrap() {
const $KEY_12 = class {};
const $KEY_170 = class {};
const $KEY_238 = function ($a, $b) {};
const $KEY_328 = function ($a) {};
const $KEY_199 = function ($a) {};
function $THREAD_8_130157() {
			class $THREAD_6_109373 {
				constructor($AO) {
					if (typeof $AO !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = $AO;
						return void 0;
					}
				}
				static of() {
					let $KEY_6 = arguments.length;
					let $KEY_2 = new Array($KEY_6);
					let i = 0;
					for (; i < $KEY_6; i++) {
						$KEY_2[i] = arguments[i];
					}
					return new $KEY_0($AO => {
						let $KEY_0 = $AO;
						$KEY_2.forEach($AP => $KEY_0.next($AP));
						$KEY_0.complete();
					});
				}
				static empty() {
					return new $KEY_0($AO => {
						$AO.complete();
					});
				}
				static from($AO, $AP) {
					let $KEY_1 = $AP;
					let $KEY_2 = $AO;
					let $KEY_3;
					if ($AO != null) {
						if ($AO instanceof $KEY_0) {
							return $AO;
						}
						if (typeof Promise !== "undefined" && $AO instanceof Promise) {
							return new $KEY_0($AQ => {
								let $KEY_0 = $AQ;
								$KEY_2.then($AR => {
									$KEY_0.next($AR);
									$KEY_0.complete();
								}).catch($AR => {
									$KEY_0.error($AR);
								});
							});
						}
						if (typeof $KEY_2.length === "number" && typeof $KEY_2 !== "function") {
							$KEY_3 = $KEY_2;
							return new $KEY_0($AQ => {
								let $KEY_2 = $AQ;
								if ($KEY_1 != null) {
									let $KEY_0 = 0;
									$KEY_1.schedule(() => {
										try {
											if ($KEY_0 >= $KEY_3.length) {
												$KEY_2.complete();
												return false;
											} else {
												$KEY_2.next($KEY_3[$KEY_0]);
												++$KEY_0;
												return true;
											}
										} catch (e) {
											$KEY_2.error(e);
											return false;
										}
									});
								} else {
									try {
										let i = 0;
										for (; i < $KEY_3.length; i++) {
											$KEY_2.next($KEY_3[i]);
										}
										$KEY_2.complete();
									} catch (e) {
										$KEY_2.error(e);
									}
								}
							});
						}
						if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_2[Symbol.iterator]) {
							return new $KEY_0($AQ => {
								let $KEY_3 = $AQ;
								try {
									let $KEY_0 = $KEY_2[Symbol.iterator]();
									if ($KEY_1 != null) {
										$KEY_1.schedule(() => {
											try {
												let $AS = $KEY_0.next();
												if ($AS.done) {
													$KEY_3.complete();
													return false;
												} else {
													$KEY_3.next($AS.value);
													return true;
												}
											} catch (e) {
												$KEY_3.error(e);
												return false;
											}
										});
									} else {
										for (;;) {
											let $AR = $KEY_0.next();
											if ($AR.done) break;
											$KEY_3.next($AR.value);
										}
										$KEY_3.complete();
									}
								} catch (e) {
									$KEY_3.error(e);
								}
							});
						}
					}
					throw new Error("Could not convert " + $KEY_2 + " to observable");
				}
				subscribe($AO, $AP, $AQ) {
					let $AR;
					if (typeof $AO === "function") {
						$AR = $AO;
					} else {
						$AR = $AO ? $AO.next : void 0;
					}
					let $AS;
					if (typeof $AO === "object") {
						$AS = $AO.error;
					} else {
						$AS = $AP;
					}
					$AR = {
						start: typeof $AO === "object" ? $AO.start : void 0,
						next: $AR,
						error: $AS,
						complete: typeof $AO === "object" ? $AO.complete : $AQ
					};
					if (this._subscriber == null) {
						throw new TypeError();
					} else {
						let $AT = new $KEY_12($AR);
						if (typeof $AR.start === "function") {
							let $AU = [$AR.start, $AR.start.call];
							$AU[1].call($AU[0], $AR, $AT);
						}
						let $AV = new $KEY_170($AT);
						try {
							$AT._cleanup = $KEY_238(this._subscriber, $AV);
						} catch (e) {
							$AV = [$AV, $AV.error];
							$AV[1].call($AV[0], e);
						}
						if ($AT._observer === void 0) {
							let $AW = $AT._cleanup;
							if ($AW !== void 0) {
								$AT._cleanup = void 0;
								try {
									$AW();
								} catch (e_2) {}
							}
						}
						return $AT;
					}
				}
				pipe() {
					let $KEY_0 = arguments.length;
					let $AO = new Array($KEY_0);
					let i = 0;
					for (; i < $KEY_0; i++) {
						$AO[i] = arguments[i];
					}
					return $AO.reduce(($AP, $AQ) => $AQ($AP), this);
				}
				map($AO) {
					return this.pipe($KEY_328($AO));
				}
				unorderedFlatMap($AO) {
					return this.pipe($KEY_199($AO));
				}
			}
			let $KEY_0 = $THREAD_6_109373;
			return $KEY_0;
		}
return $THREAD_8_130157;
}
bootstrap();"##;

// The error-prelude and inline-complete subscribe split also supports an array-like from()
// copy lowered to an inline-index `for (let i = 0; ...)` loop. Only captured callable
// origins are supplied by the carrier.
const PARTIAL_OBSERVER_ACTUAL: &str = r##"function bootstrap() {
const $KEY_92 = class {};
const $KEY_210 = class {};
const $KEY_291 = function ($a, $b) {};
const $KEY_425 = function ($a) {};
const $KEY_71 = function ($a) {};
function $THREAD_5_45439() {
					class $THREAD_3_42838 {
						constructor($BR) {
							if (typeof $BR !== "function") {
								throw new TypeError();
							} else {
								this._subscriber = $BR;
								return void 0;
							}
						}
						static of() {
							let $KEY_2 = arguments.length;
							let $KEY_3 = new Array($KEY_2);
							let i = 0;
							for (; i < $KEY_2; i++) {
								$KEY_3[i] = arguments[i];
							}
							return new $KEY_1($BR => {
								let $KEY_0 = $BR;
								$KEY_3.forEach($BS => $KEY_0.next($BS));
								$KEY_0.complete();
							});
						}
						static empty() {
							return new $KEY_1($BR => {
								$BR.complete();
							});
						}
						static from($BR, $BS) {
							let $KEY_0 = $BR;
							let $KEY_2 = $BS;
							let $KEY_3;
							if ($BR != null) {
								if ($BR instanceof $KEY_1) {
									return $BR;
								}
								if (typeof Promise !== "undefined" && $BR instanceof Promise) {
									return new $KEY_1($BT => {
										let $KEY_1 = $BT;
										$KEY_0.then($BU => {
											$KEY_1.next($BU);
											$KEY_1.complete();
										}).catch($BU => {
											$KEY_1.error($BU);
										});
									});
								}
								if (typeof $KEY_0.length === "number" && typeof $KEY_0 !== "function") {
									$KEY_3 = $KEY_0;
									return new $KEY_1($BT => {
										let $KEY_1 = $BT;
										if ($KEY_2 != null) {
											let $KEY_0 = 0;
											$KEY_2.schedule(() => {
												try {
													if ($KEY_0 >= $KEY_3.length) {
														$KEY_1.complete();
														return false;
													} else {
														$KEY_1.next($KEY_3[$KEY_0]);
														++$KEY_0;
														return true;
													}
												} catch (e) {
													$KEY_1.error(e);
													return false;
												}
											});
										} else {
											try {
												for (let i = 0; i < $KEY_3.length; i++) {
													$KEY_1.next($KEY_3[i]);
												}
												$KEY_1.complete();
											} catch (e) {
												$KEY_1.error(e);
											}
										}
									});
								}
								if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_0[Symbol.iterator]) {
									return new $KEY_1($BT => {
										let $KEY_1 = $BT;
										let $KEY_3;
										try {
											$KEY_3 = $KEY_0[Symbol.iterator]();
											if ($KEY_2 != null) {
												$KEY_2.schedule(() => {
													try {
														let $BV = $KEY_3.next();
														if ($BV.done) {
															$KEY_1.complete();
															return false;
														} else {
															$KEY_1.next($BV.value);
															return true;
														}
													} catch (e) {
														$KEY_1.error(e);
														return false;
													}
												});
											} else {
												for (;;) {
													let $BU = $KEY_3.next();
													if ($BU.done) break;
													$KEY_1.next($BU.value);
												}
												$KEY_1.complete();
											}
										} catch (e) {
											$KEY_1.error(e);
										}
									});
								}
							}
							throw new Error("Could not convert " + $KEY_0 + " to observable");
						}
						subscribe($BR, $BS, $BT) {
							let $BU = {
								start: typeof $BR === "object" ? $BR.start : void 0,
								next: typeof $BR === "function" ? $BR : $BR ? $BR.next : void 0,
								error: void 0,
								complete: void 0
							};
							$BU.error = typeof $BR === "object" ? $BR.error : $BS;
							$BU.complete = typeof $BR === "object" ? $BR.complete : $BT;
							if (this._subscriber == null) {
								throw new TypeError();
							} else {
								let $BV = new $KEY_92($BU);
								if (typeof $BU.start === "function") {
									let $BW = [$BU.start, $BU.start.call];
									$BW[1].call($BW[0], $BU, $BV);
								}
								let $BX = new $KEY_210($BV);
								try {
									$BV._cleanup = $KEY_291(this._subscriber, $BX);
								} catch (e) {
									$BX = [$BX, $BX.error];
									$BX[1].call($BX[0], e);
								}
								if ($BV._observer === void 0) {
									let $BY = $BV._cleanup;
									if ($BY !== void 0) {
										$BV._cleanup = void 0;
										try {
											$BY();
										} catch (e_2) {}
									}
								}
								return $BV;
							}
						}
						pipe() {
							let $KEY_5 = arguments.length;
							let $BR = new Array($KEY_5);
							let i = 0;
							for (; i < $KEY_5; i++) {
								$BR[i] = arguments[i];
							}
							return $BR.reduce(($BS, $BT) => $BT($BS), this);
						}
						map($BR) {
							return this.pipe($KEY_425($BR));
						}
						unorderedFlatMap($BR) {
							return this.pipe($KEY_71($BR));
						}
					}
					let $KEY_1 = $THREAD_3_42838;
					return $KEY_1;
				}
return $THREAD_5_45439;
}
bootstrap();"##;


const COMPLETE_PRELUDE_ACTUAL: &str = r##"function bootstrap() {
 const $KEY_385 = class {};
 const $KEY_84 = class {};
 const $KEY_431 = function ($a, $b) {};
 const $KEY_394 = function ($a) {};
 const $KEY_293 = function ($a) {};
 function $THREAD_6_436() {
					class $THREAD_6_163180 {
						constructor($BQ) {
							if (typeof $BQ !== "function") {
								throw new TypeError();
							} else {
								this._subscriber = $BQ;
								return void 0;
							}
						}
						static of() {
							let $KEY_7;
							let $KEY_6 = arguments.length;
							$KEY_7 = new Array($KEY_6);
							let i = 0;
							for (; i < $KEY_6; i++) {
								$KEY_7[i] = arguments[i];
							}
							return new $KEY_0($BQ => {
								let $KEY_0 = $BQ;
								$KEY_7.forEach($BR => $KEY_0.next($BR));
								$KEY_0.complete();
							});
						}
						static empty() {
							return new $KEY_0($BQ => {
								$BQ.complete();
							});
						}
						static from($BQ, $BR) {
							let $KEY_1 = $BR;
							let $KEY_3;
							let $KEY_4 = $BQ;
							if ($BQ != null) {
								if ($BQ instanceof $KEY_0) {
									return $BQ;
								}
								if (typeof Promise !== "undefined" && $BQ instanceof Promise) {
									return new $KEY_0($BS => {
										let $KEY_0 = $BS;
										$KEY_4.then($BT => {
											$KEY_0.next($BT);
											$KEY_0.complete();
										}).catch($BT => {
											$KEY_0.error($BT);
										});
									});
								}
								if (typeof $KEY_4.length === "number" && typeof $KEY_4 !== "function") {
									$KEY_3 = $KEY_4;
									return new $KEY_0($BS => {
										let $KEY_2 = $BS;
										let $KEY_5;
										if ($KEY_1 != null) {
											$KEY_5 = 0;
											$KEY_1.schedule(() => {
												try {
													if ($KEY_5 >= $KEY_3.length) {
														$KEY_2.complete();
														return false;
													} else {
														$KEY_2.next($KEY_3[$KEY_5]);
														++$KEY_5;
														return true;
													}
												} catch (e) {
													$KEY_2.error(e);
													return false;
												}
											});
										} else {
											try {
												let $BT = 0;
												while ($BT < $KEY_3.length) {
													$KEY_2.next($KEY_3[$BT]);
													++$BT;
												}
												$KEY_2.complete();
											} catch (e) {
												$KEY_2.error(e);
											}
										}
									});
								}
								if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_4[Symbol.iterator]) {
									return new $KEY_0($BS => {
										let $KEY_5 = $BS;
										try {
											let $KEY_2 = $KEY_4[Symbol.iterator]();
											if ($KEY_1 != null) {
												$KEY_1.schedule(() => {
													try {
														let $BU = $KEY_2.next();
														if ($BU.done) {
															$KEY_5.complete();
															return false;
														} else {
															$KEY_5.next($BU.value);
															return true;
														}
													} catch (e) {
														$KEY_5.error(e);
														return false;
													}
												});
											} else {
												for (;;) {
													let $BT = $KEY_2.next();
													if ($BT.done) break;
													$KEY_5.next($BT.value);
												}
												$KEY_5.complete();
											}
										} catch (e) {
											$KEY_5.error(e);
										}
									});
								}
							}
							throw new Error("Could not convert " + $KEY_4 + " to observable");
						}
						subscribe($BQ, $BR, $BS) {
							let $BT;
							if (typeof $BQ === "object") {
								$BT = $BQ.complete;
							} else {
								$BT = $BS;
							}
							$BT = {
								start: typeof $BQ === "object" ? $BQ.start : void 0,
								next: typeof $BQ === "function" ? $BQ : $BQ ? $BQ.next : void 0,
								error: typeof $BQ === "object" ? $BQ.error : $BR,
								complete: $BT
							};
							if (this._subscriber == null) {
								throw new TypeError();
							} else {
								let $BU = new $KEY_385($BT);
								if (typeof $BT.start === "function") {
									$BT.start($BU);
								}
								let $BV = new $KEY_84($BU);
								try {
									$BU._cleanup = $KEY_431(this._subscriber, $BV);
								} catch (e) {
									$BV.error(e);
								}
								if ($BU._observer === void 0) {
									let $BW = $BU._cleanup;
									if ($BW !== void 0) {
										$BU._cleanup = void 0;
										try {
											$BW();
										} catch (e_2) {}
									}
								}
								return $BU;
							}
						}
						pipe() {
							let $KEY_2 = arguments.length;
							let $BQ = new Array($KEY_2);
							let i = 0;
							for (; i < $KEY_2; i++) {
								$BQ[i] = arguments[i];
							}
							return $BQ.reduce(($BR, $BS) => $BS($BR), this);
						}
						map($BQ) {
							return this.pipe($KEY_394($BQ));
						}
						unorderedFlatMap($BQ) {
							return this.pipe($KEY_293($BQ));
						}
					}
					let $KEY_0 = $THREAD_6_163180;
					return $KEY_0;
				}
return $THREAD_6_436;
}
bootstrap();"##;


#[test]
fn observable_factory_accepts_witness_alpha_human_and_member_order() {
    for (source, target) in [
        (POSITIVE, "$THREAD_5_1328"),
        (ALPHA, "$THREAD_900_777_2"),
        (HUMAN, "$THREAD_71_8123"),
        (ORDER, "$THREAD_5_1328"),
    ] {
        assert!(named(source, target), "did not name observable factory: {target}");
        assert!(our_claim(source, target), "not the fast claim: {target}");
    }
}

#[test]
fn observable_factory_refuses_changed_constructor_and_provider() {
    refuses(
        POSITIVE,
        "$THREAD_5_1328",
        &[
            (r#"typeof $BS !== "function""#, r#"typeof $BS === "function""#),
            ("new TypeError()", "new Error()"),
            ("this._subscriber = $BS", "this._subscriber2 = $BS"),
            ("let $BW = new $KEY_95($BV);", "let $BW = new $KEY_95($BV, 0);"),
            ("$BW._cleanup = $KEY_142(this._subscriber, $BX);", "$BW._cleanup = other(this._subscriber, $BX);"),
        ],
    );
}

#[test]
fn observable_factory_refuses_changed_of_empty_and_pipe_operations() {
    refuses(
        POSITIVE,
        "$THREAD_5_1328",
        &[
            ("$KEY_4.forEach", "$KEY_4.map"),
            ("$KEY_0.next($BT)", "$KEY_0.complete($BT)"),
            ("$KEY_0.complete();", "$KEY_0.next();"),
            ("$BS.reduce", "$BS.forEach"),
            ("($BT, $BU) => $BU($BT)", "($BT, $BU) => $BT($BU)"),
            ("return this.pipe($KEY_378($BS));", "return this.subscribe($KEY_378($BS));"),
            ("return this.pipe($KEY_370($BS));", "return this.map($KEY_370($BS));"),
        ],
    );
}

#[test]
fn observable_factory_refuses_changed_from_protocol_and_branch_order() {
    refuses(
        POSITIVE,
        "$THREAD_5_1328",
        &[
            ("$BS instanceof $KEY_0", "$BS instanceof $KEY_95"),
            ("$KEY_1.then($BV =>", "$KEY_1.map($BV =>"),
            ("$KEY_3.schedule(() => {", "$KEY_3.map(() => {"),
            (r#"typeof $KEY_1.length === "number""#, r#"typeof $KEY_1.length !== "number""#),
            ("Symbol.iterator", "Symbol.asyncIterator"),
            (r#"" to observable""#, r#"" to observable!""#)
        ],
    );
    denied(BRANCH_SWAP, "$THREAD_5_1328");
}

#[test]
fn observable_factory_refuses_changed_alias_and_class_shape() {
    refuses(
        POSITIVE,
        "$THREAD_5_1328",
        &[
            ("let $KEY_0 = $THREAD_6_110488;", "let $KEY_0 = other;"),
            ("return $KEY_0;", "return $THREAD_6_110488;"),
            ("pipe() {", "filter($BS) { return this; }\n\t\t\t\tpipe() {"),
        ],
    );
    // Removing the alias leaves only two factory statements.
    denied(
        &POSITIVE.replace("\t\t\tlet $KEY_0 = $THREAD_6_110488;\n", ""),
        "$THREAD_5_1328",
    );
}

#[test]
fn observable_factory_refuses_changed_subscribe_normalization_and_cleanup() {
    refuses(
        POSITIVE,
        "$THREAD_5_1328",
        &[
            (r#"typeof $BS === "function" ? $BS"#, r#"typeof $BS === "object" ? $BS"#),
            (r#"if (typeof $BV.start === "function")"#, r#"if (typeof $BV.next === "function")"#),
            ("$BW._observer === void 0", "$BW._observer !== void 0"),
            ("$BY !== void 0", "$BY === void 0"),
            ("catch (e_2) {}", "catch (e_2) { other(); }"),
        ],
    );
}

#[test]
fn observable_factory_preserves_class_binding_and_human_names() {
    let result = name_threads(POSITIVE, true).unwrap();
    let outcome = target_outcome(&result, "$THREAD_5_1328");
    assert!(outcome.applied);
    // The complete owned class proof uses a separate class outcome.
    assert!(result.source.contains("class Observable"));
    assert!(result.report.class_outcomes.iter().any(|o| o.applied && o.proposed_name.as_deref() == Some("Observable")));
    assert!(!result.source.contains("$THREAD_5_1328"));
    assert!(result
        .report
        .outcomes
        .iter()
        .all(|outcome| outcome.binding.name != "$THREAD_6_110488"));

    let human = HUMAN.replace("$THREAD_71_8123", "humanObservableFactory");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
}

#[test]
fn observable_factory_collision_does_not_override_existing_name() {
    let collision = POSITIVE.replace(
        "function bootstrap(){",
        "function bootstrap(){let createObservableClass=17;useValue(createObservableClass);",
    );
    valid_model(&collision);
    assert!(parses(&collision));
    let result = name_threads(&collision, true).unwrap();
    let outcome = target_outcome(&result, "$THREAD_5_1328");
    assert!(!outcome.applied);
    assert_eq!(outcome.reason.as_deref(), Some("NameCollision"));
    assert!(result.source.contains("let createObservableClass=17"));
    assert!(result.source.contains("$THREAD_5_1328"));
}

#[test]
fn observable_factory_edits_are_exact_identifier_only_replacements() {
    let result = name_threads(POSITIVE, true).unwrap();
    let applied: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(NAME))
        .collect();
    assert_eq!(applied.len(), 1, "expected one applied target");
    let mut replay = POSITIVE.to_string();
    for edit in result.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}

#[test]
fn observable_factory_accepts_hoisted_array_and_for_loop_variants() {
    for (source, target) in [
        (hoisted(), "$THREAD_5_1328"),
        (for_loop(), "$THREAD_5_1328"),
        (predeclared(), "$THREAD_5_1328"),
    ] {
        assert!(named(&source, target), "did not name variant: {source}");
        assert!(our_claim(&source, target), "not the fast claim: {source}");
    }
}

#[test]
fn observable_factory_accepts_bundled_calls_and_postliteral_observer() {
    for (source, target) in [
        (bundled(), "$THREAD_5_1328"),
        (postliteral(), "$THREAD_5_1328"),
    ] {
        assert!(named(&source, target), "did not name variant: {source}");
        assert!(our_claim(&source, target), "not the fast claim: {source}");
    }
}

#[test]
fn observable_factory_accepts_complete_operator_protocol() {
    assert!(named(FULL_OPERATOR_FACTORY, "$THREAD_0_47191"), "{FULL_OPERATOR_FACTORY}");
    assert!(our_claim(FULL_OPERATOR_FACTORY, "$THREAD_0_47191"), "{FULL_OPERATOR_FACTORY}");
}

#[test]
fn observable_factory_accepts_verbatim_target_actual_and_rejects_role_neighbors() {
    assert!(named(TARGET_ACTUAL, "$THREAD_3_141537"), "{TARGET_ACTUAL}");
    assert!(our_claim(TARGET_ACTUAL, "$THREAD_3_141537"), "{TARGET_ACTUAL}");

    // Alpha spelling keeps the same owned protocol while changing the
    // generated factory/class names; the structural proof must not use them.
    let alpha = TARGET_ACTUAL
        .replace("$THREAD_3_141537", "$THREAD_900_777_3")
        .replace("$THREAD_5_15575", "$THREAD_901_888_4");
    assert!(named(&alpha, "$THREAD_900_777_3"), "{alpha}");

    for (from, to) in [
        ("let $KEY_3 = $AQ;", "let $KEY_3 = $AP;"),
        ("let $KEY_0 = $KEY_4;", "let $KEY_0 = $KEY_3;"),
        ("$AS = $AP;", "$AS = $AQ;"),
        (
            "return this.pipe($KEY_294($AP));",
            "return this.subscribe($KEY_294($AP));",
        ),
    ] {
        denied(&TARGET_ACTUAL.replace(from, to), "$THREAD_3_141537");
    }
}

#[test]
fn observable_factory_accepts_guarded_constructor_and_rejects_variant_neighbors() {
    assert!(named(GUARDED_CONSTRUCTOR, "$THREAD_6_100381"), "{GUARDED_CONSTRUCTOR}");
    assert!(our_claim(GUARDED_CONSTRUCTOR, "$THREAD_6_100381"), "{GUARDED_CONSTRUCTOR}");

    // These mutations exercise the two-alias from() recovery and the
    // unchanged full three-arm inline observer-next proof on a second witness.
    for (from, to) in [
        ("let $KEY_2 = $BR;", "let $KEY_2 = $BS;"),
        ("let $KEY_0 = $KEY_2;", "let $KEY_0 = $KEY_4;"),
        (
            "next: typeof $BR === \"function\" ? $BR : $BR ? $BR.next : void 0,",
            "next: typeof $BR === \"function\" ? $BR : $BS ? $BR.next : void 0,",
        ),
    ] {
        denied(&GUARDED_CONSTRUCTOR.replace(from, to), "$THREAD_6_100381");
    }
}

// Readable receiver/argument spellings prove the bundled-call proof follows
// scoped identities and protocol shape, not generated names.
#[test]
fn observable_factory_accepts_human_spelling_bundled_calls() {
    let human = varied(
        HUMAN,
        &[
            (
                "value8.start(value20);",
                "let bundle = [value8.start, value8.start.call]; bundle[1].call(bundle[0], value8, value20);",
            ),
            (
                "value21.error(e);",
                "value21 = [value21, value21.error]; value21[1].call(value21[0], e);",
            ),
        ],
    );
    assert!(named(&human, "$THREAD_71_8123"), "{human}");
    assert!(our_claim(&human, "$THREAD_71_8123"), "{human}");
}

#[test]
fn observable_factory_refuses_hoisted_array_and_for_loop_mismatches() {
    refuses(
        &hoisted(),
        "$THREAD_5_1328",
        &[
            (
                "$KEY_4 = new Array($KEY_3);",
                "$KEY_4 = new Array($KEY_3, 0);",
            ),
            ("$KEY_4 = new Array($KEY_3);", "$KEY_4 = new Array($OTHER);"),
            ("let $KEY_4; let $KEY_3", "let $KEY_4 = []; let $KEY_3"),
        ],
    );
    refuses(
        &for_loop(),
        "$THREAD_5_1328",
        &[("for (; $BV < $KEY_2.length; $BV++) {", "for (;;) {")],
    );
}

#[test]
fn observable_factory_refuses_predeclared_local_mismatches() {
    refuses(
        &predeclared(),
        "$THREAD_5_1328",
        &[
            ("$KEY_0 = 0;", "$KEY_0 = 1;"),
            (
                "let $KEY_0;\n\t\t\t\t\t\t\t\tif ($KEY_3 != null) {",
                "let $KEY_0 = 9;\n\t\t\t\t\t\t\t\tif ($KEY_3 != null) {",
            ),
        ],
    );
}

#[test]
fn observable_factory_refuses_bundled_call_mismatches() {
    refuses(
        &bundled(),
        "$THREAD_5_1328",
        &[
            ("[$BV.start, $BV.start.call]", "[$BW.start, $BW.start.call]"),
            ("[$BV.start, $BV.start.call]", "[$BV.start, $BV.start.other]"),
            (
                "$BND[1].call($BND[0], $BV, $BW)",
                "$BND[1].call($BND[0], $BW, $BV)",
            ),
            ("$BX = [$BX, $BX.error];", "$BX = [$BX, $BX.length];"),
            ("$BX[1].call($BX[0], e)", "$BX[0].call($BX[1], e)"),
        ],
    );
}

#[test]
fn observable_factory_refuses_observer_postliteral_mismatches() {
    refuses(
        &postliteral(),
        "$THREAD_5_1328",
        &[
            (
                r#"$BV.complete = typeof $BS === "object" ? $BS.complete : $BU;"#,
                r#"$BV.finish = typeof $BS === "object" ? $BS.complete : $BU;"#,
            ),
            (
                r#"$BV.error = typeof $BS === "object" ? $BS.error : $BT;"#,
                "",
            ),
            ("next: void 0,", "next: 0,"),
        ],
    );
}

#[test]
fn observable_factory_refuses_nested_donor_factory() {
    let nested = POSITIVE
        .replace(
            "function $THREAD_5_1328() {",
            "function $THREAD_9_9() {\nfunction $THREAD_5_1328() {",
        )
        .replace(
            "return $THREAD_5_1328;\n}\nbootstrap();",
            "return $THREAD_5_1328;\n}\nreturn $THREAD_9_9;\n}\nbootstrap();",
        );
    denied(&nested, "$THREAD_9_9");
    // The genuine inner factory is still the one the rule claims.
    assert!(named(&nested, "$THREAD_5_1328"), "{nested}");
}

#[test]
fn observable_factory_accepts_subscribe_subscribe_prelude_variant() {
    assert!(named(SPLIT_OBSERVER_PRELUDE, "$THREAD_1_126926"), "{SPLIT_OBSERVER_PRELUDE}");
    assert!(our_claim(SPLIT_OBSERVER_PRELUDE, "$THREAD_1_126926"), "{SPLIT_OBSERVER_PRELUDE}");

    // Generated spellings of the factory and owned class are not part of the
    // proof; only the structural protocol is.
    let alpha = SPLIT_OBSERVER_PRELUDE
        .replace("$THREAD_1_126926", "$THREAD_900_777_5")
        .replace("$THREAD_1_12528", "$THREAD_901_888_6");
    assert!(named(&alpha, "$THREAD_900_777_5"), "{alpha}");
    assert!(our_claim(&alpha, "$THREAD_900_777_5"), "{alpha}");
}

#[test]
fn observable_factory_refuses_subscribe_subscribe_prelude_mismatches() {
    refuses(
        SPLIT_OBSERVER_PRELUDE,
        "$THREAD_1_126926",
        &[
            // Wrong slot: the literal's `next` no longer names the prelude slot.
            ("next: $BS,", "next: $BT,"),
            // Wrong slot: the resolved literal is assigned into the complete slot.
            ("$BS = {\n", "$BT = {\n"),
            // Wrong callback: `start` is read off the wrong normalized handle.
            (
                r#"if (typeof $BS.start === "function") {"#,
                r#"if (typeof $BT.start === "function") {"#,
            ),
            // Wrong member: the literal's complete member is mislabelled.
            ("complete: $BT", "finish: $BT"),
            // Wrong member: the complete prelude reads the wrong observer member.
            ("$BT = $BP.complete;", "$BT = $BP.error;"),
            // Extra effect: an additional statement breaks the bounded arm.
            ("let $BT;", "let $BT; other();"),
            // Uninitialized/overwritten alias: the next slot carries an initializer.
            ("let $BS;", "let $BS = $BP;"),
            // Moved slot: the complete prelude writes the next slot instead.
            ("$BT = $BR;", "$BS = $BR;"),
        ],
    );
}

#[test]
fn observable_factory_accepts_inline_index_argument_copy_loop_variant() {
    assert!(named(INLINE_ARGUMENT_INDEX, "$THREAD_6_96922"), "{INLINE_ARGUMENT_INDEX}");
    assert!(our_claim(INLINE_ARGUMENT_INDEX, "$THREAD_6_96922"), "{INLINE_ARGUMENT_INDEX}");

    // Generated spellings of the factory and owned class are not part of the
    // proof; only the structural protocol is.
    let alpha = INLINE_ARGUMENT_INDEX
        .replace("$THREAD_6_96922", "$THREAD_900_777_7")
        .replace("$THREAD_2_157798", "$THREAD_901_888_8");
    assert!(named(&alpha, "$THREAD_900_777_7"), "{alpha}");
    assert!(our_claim(&alpha, "$THREAD_900_777_7"), "{alpha}");
}

#[test]
fn observable_factory_refuses_inline_index_argument_copy_loop_mismatches() {
    refuses(
        INLINE_ARGUMENT_INDEX,
        "$THREAD_6_96922",
        &[
            // Wrong init: the inline index no longer starts at zero.
            (
                "for (let i = 0; i < $KEY_4; i++) {",
                "for (let i = 2; i < $KEY_4; i++) {",
            ),
            // Foreign index: the loop test reads a binding the head never owns.
            (
                "for (let i = 0; i < $KEY_4; i++) {",
                "for (let i = 0; $BZ < $KEY_4; i++) {",
            ),
            // Wrong array: the copy target is not the owned argument array.
            ("$BR[i] = arguments[i];", "$BO[i] = arguments[i];"),
            // Wrong length: the test compares against a foreign count.
            ("i < $KEY_4", "i < $KEY_9"),
            // Wrong test: a non-strict bound is not the copy loop.
            ("i < $KEY_4", "i <= $KEY_4"),
            // Wrong update: the step is not the plain postfix increment.
            ("i < $KEY_4; i++)", "i < $KEY_4; i += 1)"),
            // Extra effect: a second statement breaks the bounded body.
            ("$BR[i] = arguments[i];", "$BR[i] = arguments[i]; other();"),
            // Scoped declaration: a destructuring head is not a plain index.
            (
                "for (let i = 0; i < $KEY_4; i++) {",
                "for (let [i] = [0]; i < $KEY_4; i++) {",
            ),
            // Borrowed nested helper: the loop is hidden behind a call.
            (
                "let $BR = new Array($KEY_4);\n\t\t\t\t\tfor (let i = 0; i < $KEY_4; i++) {\n\t\t\t\t\t\t$BR[i] = arguments[i];\n\t\t\t\t\t}",
                "let $BR = new Array($KEY_4);\n\t\t\t\t\tfunction $COPY() { for (let i = 0; i < $KEY_4; i++) { $BR[i] = arguments[i]; } }\n\t\t\t\t\t$COPY();",
            ),
        ],
    );
}

#[test]
fn observable_factory_accepts_error_prelude_error_prelude_variant() {
    assert!(named(ERROR_PRELUDE_ACTUAL, "$THREAD_8_130157"), "{ERROR_PRELUDE_ACTUAL}");
    assert!(our_claim(ERROR_PRELUDE_ACTUAL, "$THREAD_8_130157"), "{ERROR_PRELUDE_ACTUAL}");

    // Generated spellings of the factory and owned class are not part of the
    // proof; only the structural protocol is.
    let alpha = ERROR_PRELUDE_ACTUAL
        .replace("$THREAD_8_130157", "$THREAD_900_777_2")
        .replace("$THREAD_6_109373", "$THREAD_901_888_2");
    assert!(named(&alpha, "$THREAD_900_777_2"), "{alpha}");
    assert!(our_claim(&alpha, "$THREAD_900_777_2"), "{alpha}");
}

#[test]
fn observable_factory_refuses_error_prelude_error_prelude_mismatches() {
    refuses(
        ERROR_PRELUDE_ACTUAL,
        "$THREAD_8_130157",
        &[
            // Wrong member: the object-form prelude reads `.complete`, not `.error`.
            ("$AS = $AO.error;", "$AS = $AO.complete;"),
            // Wrong fallback slot: the error prelude falls back to the complete arg.
            ("$AS = $AP;", "$AS = $AQ;"),
            // Wrong literal slot: the resolved `error` names the next slot.
            ("error: $AS,", "error: $AR,"),
            // Wrong member: the inline complete reads the error member.
            (
                "complete: typeof $AO === \"object\" ? $AO.complete : $AQ",
                "complete: typeof $AO === \"object\" ? $AO.error : $AQ",
            ),
            // Moved slot: the resolved literal is assigned into the error slot.
            ("$AR = {\n", "$AS = {\n"),
            // Wrong receiver: `start` is read off the error slot.
            (
                "if (typeof $AR.start === \"function\") {",
                "if (typeof $AS.start === \"function\") {",
            ),
            // Extra effect: an added statement breaks the bounded six-statement arm.
            ("let $AS;", "let $AS; other();"),
        ],
    );
}

#[test]
fn observable_factory_accepts_observer_literal_partial_observer_literal_variant() {
    assert!(named(PARTIAL_OBSERVER_ACTUAL, "$THREAD_5_45439"), "{PARTIAL_OBSERVER_ACTUAL}");
    assert!(our_claim(PARTIAL_OBSERVER_ACTUAL, "$THREAD_5_45439"), "{PARTIAL_OBSERVER_ACTUAL}");

    let alpha = PARTIAL_OBSERVER_ACTUAL
        .replace("$THREAD_5_45439", "$THREAD_900_777_3")
        .replace("$THREAD_3_42838", "$THREAD_901_888_4");
    assert!(named(&alpha, "$THREAD_900_777_3"), "{alpha}");
    assert!(our_claim(&alpha, "$THREAD_900_777_3"), "{alpha}");
}

#[test]
fn observable_factory_refuses_observer_literal_partial_observer_literal_mismatches() {
    refuses(
        PARTIAL_OBSERVER_ACTUAL,
        "$THREAD_5_45439",
        &[
            // Wrong next: the literal's next is no longer the canonical conditional.
            (
                "next: typeof $BR === \"function\" ? $BR : $BR ? $BR.next : void 0,",
                "next: $BS,",
            ),
            // Wrong member: the first write targets `finish`, not `error`.
            ("$BU.error = ", "$BU.finish = "),
            // Wrong fallback: the error write falls back to the complete argument.
            ("? $BR.error : $BS;", "? $BR.error : $BT;"),
            // Wrong member: the complete write reads the error member.
            ("$BR.complete : $BT;", "$BR.error : $BT;"),
            // Missing write: the complete member is never filled.
            ("$BU.complete = typeof $BR === \"object\" ? $BR.complete : $BT;\n", ""),
            // Extra effect: an added statement breaks the bounded four-statement arm.
            ("$BU.error = ", "$BU.error = $BU.error;\n$BU.error = "),
        ],
    );
}

#[test]
fn observable_factory_accepts_observer_literal_complete_prelude_variant() {
    assert!(named(COMPLETE_PRELUDE_ACTUAL, "$THREAD_6_436"), "{COMPLETE_PRELUDE_ACTUAL}");
    assert!(our_claim(COMPLETE_PRELUDE_ACTUAL, "$THREAD_6_436"), "{COMPLETE_PRELUDE_ACTUAL}");

    // Generated spellings of the factory and owned class are not part of the
    // proof; the shared grammar's valid generated disambiguator is >= 2.
    let alpha = COMPLETE_PRELUDE_ACTUAL
        .replace("$THREAD_6_436", "$THREAD_900_777_2")
        .replace("$THREAD_6_163180", "$THREAD_901_888_2");
    assert!(named(&alpha, "$THREAD_900_777_2"), "{alpha}");
    assert!(our_claim(&alpha, "$THREAD_900_777_2"), "{alpha}");
}

#[test]
fn observable_factory_refuses_observer_literal_complete_prelude_mismatches() {
    refuses(
        COMPLETE_PRELUDE_ACTUAL,
        "$THREAD_6_436",
        &[
            // Wrong member: the object-form prelude reads `.error`, not the
            // complete member it falls back to.
            ("$BT = $BQ.complete;", "$BT = $BQ.error;"),
            // Wrong fallback: the complete prelude falls back to the next arg.
            ("$BT = $BS;", "$BT = $BR;"),
            // Wrong literal slot: the resolved `complete` names the next arg.
            ("complete: $BT", "complete: $BR"),
            // Moved normalization target: the literal is assigned into the
            // next argument instead of the role-proved prelude slot.
            ("$BT = {", "$BR = {"),
            // Extra effect: an added statement breaks the bounded four-statement
            // arm.
            ("let $BT;", "let $BT; other();"),
        ],
    );
}


// Factory excerpts retain their bodies; only the five captured callable dependencies are
// stubbed in the enclosing fixture. No recovered code runs. This spelling uses an inline
// argument-copy index.
const INLINE_COPY_INDEX: &str = r##"function bootstrap(){
const $KEY_419 = function(value){return value;};
const $KEY_100 = function(value){return value;};
const $KEY_48 = function(value){return value;};
const $KEY_403 = function(value){return value;};
const $KEY_36 = function(value){return value;};
function $THREAD_6_140623() {
			class $THREAD_5_98099 {
				constructor($BT) {
					if (typeof $BT !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = $BT;
						return void 0;
					}
				}
				static of() {
					let $KEY_6;
					let $KEY_2 = arguments.length;
					$KEY_6 = new Array($KEY_2);
					let i = 0;
					for (; i < $KEY_2; i++) {
						$KEY_6[i] = arguments[i];
					}
					return new $KEY_1($BT => {
						let $KEY_0 = $BT;
						$KEY_6.forEach($BU => $KEY_0.next($BU));
						$KEY_0.complete();
					});
				}
				static empty() {
					return new $KEY_1($BT => {
						$BT.complete();
					});
				}
				static from($BT, $BU) {
					let $KEY_2 = $BU;
					let $KEY_3 = $BT;
					let $KEY_4;
					if ($BT != null) {
						if ($BT instanceof $KEY_1) {
							return $BT;
						}
						if (typeof Promise !== "undefined" && $BT instanceof Promise) {
							return new $KEY_1($BV => {
								let $KEY_0 = $BV;
								$KEY_3.then($BW => {
									$KEY_0.next($BW);
									$KEY_0.complete();
								}).catch($BW => {
									$KEY_0.error($BW);
								});
							});
						}
						if (typeof $KEY_3.length === "number" && typeof $KEY_3 !== "function") {
							$KEY_4 = $KEY_3;
							return new $KEY_1($BV => {
								let $KEY_5 = $BV;
								if ($KEY_2 != null) {
									let $KEY_0 = 0;
									$KEY_2.schedule(() => {
										try {
											if ($KEY_0 >= $KEY_4.length) {
												$KEY_5.complete();
												return false;
											} else {
												$KEY_5.next($KEY_4[$KEY_0]);
												++$KEY_0;
												return true;
											}
										} catch (e) {
											$KEY_5.error(e);
											return false;
										}
									});
								} else {
									try {
										let i = 0;
										for (; i < $KEY_4.length; i++) {
											$KEY_5.next($KEY_4[i]);
										}
										$KEY_5.complete();
									} catch (e) {
										$KEY_5.error(e);
									}
								}
							});
						}
						if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_3[Symbol.iterator]) {
							return new $KEY_1($BV => {
								let $KEY_1 = $BV;
								let $KEY_4;
								try {
									$KEY_4 = $KEY_3[Symbol.iterator]();
									if ($KEY_2 != null) {
										$KEY_2.schedule(() => {
											try {
												let $BX = $KEY_4.next();
												if ($BX.done) {
													$KEY_1.complete();
													return false;
												} else {
													$KEY_1.next($BX.value);
													return true;
												}
											} catch (e) {
												$KEY_1.error(e);
												return false;
											}
										});
									} else {
										for (;;) {
											let $BW = $KEY_4.next();
											if ($BW.done) break;
											$KEY_1.next($BW.value);
										}
										$KEY_1.complete();
									}
								} catch (e) {
									$KEY_1.error(e);
								}
							});
						}
					}
					throw new Error("Could not convert " + $KEY_3 + " to observable");
				}
				subscribe($BT, $BU, $BV) {
					let $BW;
					if (typeof $BT === "function") {
						$BW = $BT;
					} else {
						$BW = $BT ? $BT.next : void 0;
					}
					$BW = {
						start: typeof $BT === "object" ? $BT.start : void 0,
						next: $BW,
						error: typeof $BT === "object" ? $BT.error : $BU,
						complete: typeof $BT === "object" ? $BT.complete : $BV
					};
					if (this._subscriber == null) {
						throw new TypeError();
					} else {
						let $BX = new $KEY_419($BW);
						if (typeof $BW.start === "function") {
							$BW.start($BX);
						}
						let $BY = new $KEY_100($BX);
						try {
							$BX._cleanup = $KEY_48(this._subscriber, $BY);
						} catch (e) {
							$BY.error(e);
						}
						if ($BX._observer === void 0) {
							let $BZ = $BX._cleanup;
							if ($BZ !== void 0) {
								$BX._cleanup = void 0;
								try {
									$BZ();
								} catch (e_2) {}
							}
						}
						return $BX;
					}
				}
				pipe() {
					let $KEY_6 = arguments.length;
					let $BT = new Array($KEY_6);
					let i = 0;
					for (; i < $KEY_6; i++) {
						$BT[i] = arguments[i];
					}
					return $BT.reduce(($BU, $BV) => $BV($BU), this);
				}
				map($BT) {
					return this.pipe($KEY_403($BT));
				}
				unorderedFlatMap($BT) {
					return this.pipe($KEY_36($BT));
				}
			}
			let $KEY_1 = $THREAD_5_98099;
			return $KEY_1;
		}
return $THREAD_6_140623;
} bootstrap();"##;
#[test]
fn observable_factory_accepts_inline_copy_index_lowering() {
    assert!(named(INLINE_COPY_INDEX, "$THREAD_6_140623"));
}
// Factory spelling with an object-form error prelude.
const ERROR_PRELUDE_LOWERING: &str = r##"function bootstrap(){
const $KEY_324 = function(value){return value;};
const $KEY_40 = function(value){return value;};
const $KEY_421 = function(value){return value;};
const $KEY_115 = function(value){return value;};
const $KEY_240 = function(value){return value;};
function $THREAD_3_97321() {
			class $THREAD_0_29495 {
				constructor($AJ) {
					if (typeof $AJ !== "function") {
						throw new TypeError();
					} else {
						this._subscriber = $AJ;
						return void 0;
					}
				}
				static of() {
					let $KEY_3 = arguments.length;
					let $KEY_5 = new Array($KEY_3);
					let i = 0;
					for (; i < $KEY_3; i++) {
						$KEY_5[i] = arguments[i];
					}
					return new $KEY_2($AJ => {
						let $KEY_0 = $AJ;
						$KEY_5.forEach($AK => $KEY_0.next($AK));
						$KEY_0.complete();
					});
				}
				static empty() {
					return new $KEY_2($AJ => {
						$AJ.complete();
					});
				}
				static from($AJ, $AK) {
					let $KEY_1 = $AJ;
					let $KEY_3 = $AK;
					let $KEY_5;
					if ($AJ != null) {
						if ($AJ instanceof $KEY_2) {
							return $AJ;
						}
						if (typeof Promise !== "undefined" && $AJ instanceof Promise) {
							return new $KEY_2($AM => {
								let $KEY_3 = $AM;
								$KEY_1.then($AN => {
									$KEY_3.next($AN);
									$KEY_3.complete();
								}).catch($AN => {
									$KEY_3.error($AN);
								});
							});
						}
						if (typeof $KEY_1.length === "number" && typeof $KEY_1 !== "function") {
							$KEY_5 = $KEY_1;
							return new $KEY_2($AM => {
								let $KEY_1 = $AM;
								let $KEY_2;
								if ($KEY_3 != null) {
									$KEY_2 = 0;
									$KEY_3.schedule(() => {
										try {
											if ($KEY_2 >= $KEY_5.length) {
												$KEY_1.complete();
												return false;
											} else {
												$KEY_1.next($KEY_5[$KEY_2]);
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
										let $AN = 0;
										while ($AN < $KEY_5.length) {
											$KEY_1.next($KEY_5[$AN]);
											++$AN;
										}
										$KEY_1.complete();
									} catch (e) {
										$KEY_1.error(e);
									}
								}
							});
						}
						let $AL;
						if (typeof Symbol !== "undefined") {
							$AL = Symbol.iterator;
						} else {
							$AL = typeof Symbol !== "undefined";
						}
						if ($AL && $KEY_1[Symbol.iterator]) {
							return new $KEY_2($AM => {
								let $KEY_0 = $AM;
								let $KEY_4;
								try {
									$KEY_4 = $KEY_1[Symbol.iterator]();
									if ($KEY_3 != null) {
										$KEY_3.schedule(() => {
											try {
												let $AO = $KEY_4.next();
												if ($AO.done) {
													$KEY_0.complete();
													return false;
												} else {
													$KEY_0.next($AO.value);
													return true;
												}
											} catch (e) {
												$KEY_0.error(e);
												return false;
											}
										});
									} else {
										for (;;) {
											let $AN = $KEY_4.next();
											if ($AN.done) break;
											$KEY_0.next($AN.value);
										}
										$KEY_0.complete();
									}
								} catch (e) {
									$KEY_0.error(e);
								}
							});
						}
					}
					throw new Error("Could not convert " + $KEY_1 + " to observable");
				}
				subscribe($AJ, $AK, $AL) {
					let $AM = {
						start: typeof $AJ === "object" ? $AJ.start : void 0,
						next: typeof $AJ === "function" ? $AJ : $AJ ? $AJ.next : void 0,
						error: typeof $AJ === "object" ? $AJ.error : $AK,
						complete: typeof $AJ === "object" ? $AJ.complete : $AL
					};
					if (this._subscriber == null) {
						throw new TypeError();
					} else {
						let $AN = new $KEY_324($AM);
						if (typeof $AM.start === "function") {
							$AM.start($AN);
						}
						let $AO = new $KEY_40($AN);
						try {
							$AN._cleanup = $KEY_421(this._subscriber, $AO);
						} catch (e) {
							$AO.error(e);
						}
						if ($AN._observer === void 0) {
							let $AP = $AN._cleanup;
							if ($AP !== void 0) {
								$AN._cleanup = void 0;
								try {
									$AP();
								} catch (e_2) {}
							}
						}
						return $AN;
					}
				}
				pipe() {
					let $KEY_3 = arguments.length;
					let $AJ = new Array($KEY_3);
					let i = 0;
					for (; i < $KEY_3; i++) {
						$AJ[i] = arguments[i];
					}
					return $AJ.reduce(($AK, $AL) => $AL($AK), this);
				}
				map($AJ) {
					return this.pipe($KEY_115($AJ));
				}
				unorderedFlatMap($AJ) {
					return this.pipe($KEY_240($AJ));
				}
			}
			let $KEY_2 = $THREAD_0_29495;
			return $KEY_2;
		}
return $THREAD_3_97321;
} bootstrap();"##;
#[test]
fn observable_factory_accepts_error_prelude_lowering() {
    assert!(named(ERROR_PRELUDE_LOWERING, "$THREAD_3_97321"));
}
// Factory spelling with split observer preludes.
const SPLIT_OBSERVER_PRELUDE_LOWERING: &str = r##"function bootstrap(){
const $KEY_391 = function(value){return value;};
const $KEY_142 = function(value){return value;};
const $KEY_284 = function(value){return value;};
const $KEY_397 = function(value){return value;};
const $KEY_143 = function(value){return value;};
function $THREAD_4_57213() {
					class $THREAD_6_24015 {
						constructor($BL) {
							if (typeof $BL !== "function") {
								throw new TypeError();
							} else {
								this._subscriber = $BL;
								return void 0;
							}
						}
						static of() {
							let $KEY_3 = arguments.length;
							let $KEY_2 = new Array($KEY_3);
							let i = 0;
							for (; i < $KEY_3; i++) {
								$KEY_2[i] = arguments[i];
							}
							return new $KEY_1($BL => {
								let $KEY_0 = $BL;
								$KEY_2.forEach($BM => $KEY_0.next($BM));
								$KEY_0.complete();
							});
						}
						static empty() {
							return new $KEY_1($BL => {
								$BL.complete();
							});
						}
						static from($BL, $BM) {
							let $KEY_0 = $BL;
							let $KEY_4;
							let $KEY_5 = $BM;
							if ($BL != null) {
								if ($BL instanceof $KEY_1) {
									return $BL;
								}
								let $BN;
								if (typeof Promise !== "undefined") {
									$BN = $BL instanceof Promise;
								} else {
									$BN = typeof Promise !== "undefined";
								}
								if ($BN) {
									return new $KEY_1($BO => {
										let $KEY_1 = $BO;
										$KEY_0.then($BP => {
											$KEY_1.next($BP);
											$KEY_1.complete();
										}).catch($BP => {
											$KEY_1.error($BP);
										});
									});
								}
								if (typeof $KEY_0.length === "number" && typeof $KEY_0 !== "function") {
									$KEY_4 = $KEY_0;
									return new $KEY_1($BO => {
										let $KEY_0 = $BO;
										let $KEY_1;
										if ($KEY_5 != null) {
											$KEY_1 = 0;
											$KEY_5.schedule(() => {
												try {
													if ($KEY_1 >= $KEY_4.length) {
														$KEY_0.complete();
														return false;
													} else {
														$KEY_0.next($KEY_4[$KEY_1]);
														++$KEY_1;
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
								if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_0[Symbol.iterator]) {
									return new $KEY_1($BO => {
										let $KEY_2 = $BO;
										let $KEY_3;
										try {
											$KEY_3 = $KEY_0[Symbol.iterator]();
											if ($KEY_5 != null) {
												$KEY_5.schedule(() => {
													try {
														let $BQ = $KEY_3.next();
														if ($BQ.done) {
															$KEY_2.complete();
															return false;
														} else {
															$KEY_2.next($BQ.value);
															return true;
														}
													} catch (e) {
														$KEY_2.error(e);
														return false;
													}
												});
											} else {
												for (;;) {
													let $BP = $KEY_3.next();
													if ($BP.done) break;
													$KEY_2.next($BP.value);
												}
												$KEY_2.complete();
											}
										} catch (e) {
											$KEY_2.error(e);
										}
									});
								}
							}
							throw new Error("Could not convert " + $KEY_0 + " to observable");
						}
						subscribe($BL, $BM, $BN) {
							let $BO;
							if (typeof $BL === "object") {
								$BO = $BL.complete;
							} else {
								$BO = $BN;
							}
							let $BP = {
								start: typeof $BL === "object" ? $BL.start : void 0,
								next: void 0,
								error: void 0,
								complete: void 0
							};
							$BP.next = typeof $BL === "function" ? $BL : $BL ? $BL.next : void 0;
							$BP.error = typeof $BL === "object" ? $BL.error : $BM;
							$BP.complete = $BO;
							if (this._subscriber == null) {
								throw new TypeError();
							} else {
								let $BQ = new $KEY_391($BP);
								if (typeof $BP.start === "function") {
									$BP.start($BQ);
								}
								let $BR = new $KEY_142($BQ);
								try {
									$BQ._cleanup = $KEY_284(this._subscriber, $BR);
								} catch (e) {
									$BR.error(e);
								}
								if ($BQ._observer === void 0) {
									let $BS = $BQ._cleanup;
									if ($BS !== void 0) {
										$BQ._cleanup = void 0;
										try {
											$BS();
										} catch (e_2) {}
									}
								}
								return $BQ;
							}
						}
						pipe() {
							let $KEY_4 = arguments.length;
							let $BL = new Array($KEY_4);
							let i = 0;
							for (; i < $KEY_4; i++) {
								$BL[i] = arguments[i];
							}
							return $BL.reduce(($BM, $BN) => $BN($BM), this);
						}
						map($BL) {
							return this.pipe($KEY_397($BL));
						}
						unorderedFlatMap($BL) {
							return this.pipe($KEY_143($BL));
						}
					}
					let $KEY_1 = $THREAD_6_24015;
					return $KEY_1;
				}
return $THREAD_4_57213;
} bootstrap();"##;
#[test]
fn observable_factory_accepts_split_observer_prelude_lowering() {
    assert!(named(SPLIT_OBSERVER_PRELUDE_LOWERING, "$THREAD_4_57213"));
}

#[test]
fn observable_factory_new_normalization_refuses_owner_and_sequence_neighbors() {
    refuses(
        INLINE_COPY_INDEX,
        "$THREAD_6_140623",
        &[
            ("return $KEY_1;", "return otherClass;"),
            ("let $KEY_1 = $THREAD_5_98099;", "let $KEY_1 = otherClass;"),
            ("return $KEY_1;", "$KEY_1 = otherClass; return $KEY_1;"),
            (
                "return $KEY_1;",
                "if (flag) { return otherClass; } return $KEY_1;",
            ),
            (
                "let $KEY_1 = $THREAD_5_98099;",
                "let $KEY_1; if (flag) { $KEY_1 = $THREAD_5_98099; }",
            ),
            (
                "let $KEY_1 = $THREAD_5_98099;",
                "let $KEY_1; (() => { $KEY_1 = $THREAD_5_98099; })();",
            ),
            ("return $KEY_1;", "$KEY_1.subscribe = 0; return $KEY_1;"),
            ("this._subscriber = $BT;", "this._subscriber = other;"),
            (
                "$KEY_5.next($KEY_4[$KEY_0]);",
                "(() => { $KEY_5.next($KEY_4[$KEY_0]); });",
            ),
            (
                "$KEY_5.next($KEY_4[$KEY_0]);",
                "class Donor { next(){ $KEY_5.next($KEY_4[$KEY_0]); } }",
            ),
            ("$KEY_5.next($KEY_4[$KEY_0]);", "$KEY_5.next(other);"),
            ("$BW = $BT ? $BT.next : void 0;", "$BW = void 0;"),
            ("next: $BW,", "next: other,"),
            ("$BW = {", "$BW = 0; $BW = {"),
            ("$KEY_0 >= $KEY_4.length", "$KEY_0 > $KEY_4.length"),
        ],
    );
    // A complete donor in a nested arrow does not certify the outer factory.
    let nested = INLINE_COPY_INDEX
        .replace(
            "function $THREAD_6_140623() {",
            "function $THREAD_6_140623() { const donor = () => {",
        )
        .replace("return $KEY_1;", "return $KEY_1; }; return otherClass;");
    denied(&nested, "$THREAD_6_140623");
}

#[test]
fn observable_factory_split_guards_refuse_changed_branch_values() {
    refuses(
        ERROR_PRELUDE_LOWERING,
        "$THREAD_3_97321",
        &[
            ("$AL = Symbol.iterator;", "$AL = Symbol.toStringTag;"),
            ("$AL = typeof Symbol !== \"undefined\";", "$AL = true;"),
            ("if ($AL &&", "if (other &&"),
        ],
    );
    refuses(
        SPLIT_OBSERVER_PRELUDE_LOWERING,
        "$THREAD_4_57213",
        &[
            ("$BN = $BL instanceof Promise;", "$BN = true;"),
            ("$BN = typeof Promise !== \"undefined\";", "$BN = true;"),
            ("if ($BN)", "if (other)"),
        ],
    );
}

#[test]
fn observable_factory_accepts_composed_normalization_and_array_guard_lowerings() {
    let expanded = varied(
        INLINE_COPY_INDEX,
        &[(
            "} else {\n\t\t\t\t\t\t$BW = $BT ? $BT.next : void 0;\n\t\t\t\t\t}",
            "} else if ($BT) { $BW = $BT.next; } else { $BW = void 0; }",
        )],
    );
    assert!(named(&expanded, "$THREAD_6_140623"));
    refuses(
        &expanded,
        "$THREAD_6_140623",
        &[
            ("else if ($BT)", "else if (other)"),
            ("$BW = $BT.next;", "$BW = $BT.error;"),
            ("$BW = void 0;", "$BW = other;"),
        ],
    );
    let partial = varied(INLINE_COPY_INDEX, &[
        ("error: typeof $BT === \"object\" ? $BT.error : $BU,", "error: void 0,"),
        ("complete: typeof $BT === \"object\" ? $BT.complete : $BV", "complete: void 0"),
        ("if (this._subscriber == null)", "$BW.error = typeof $BT === \"object\" ? $BT.error : $BU; $BW.complete = typeof $BT === \"object\" ? $BT.complete : $BV; if (this._subscriber == null)"),
    ]);
    assert!(named(&partial, "$THREAD_6_140623"));
    refuses(
        &partial,
        "$THREAD_6_140623",
        &[
            ("$BW.error = typeof", "other.error = typeof"),
            ("$BW.complete = typeof", "$BW.next = typeof"),
            (
                "$BW.complete = typeof",
                "$BW.error = 0; $BW.complete = typeof",
            ),
        ],
    );
    let complete_only = varied(INLINE_COPY_INDEX, &[
        ("complete: typeof $BT === \"object\" ? $BT.complete : $BV", "complete: void 0"),
        ("if (this._subscriber == null)", "$BW.complete = typeof $BT === \"object\" ? $BT.complete : $BV; if (this._subscriber == null)"),
    ]);
    assert!(named(&complete_only, "$THREAD_6_140623"));
    let array_guard = varied(INLINE_COPY_INDEX, &[(
        "if (typeof $KEY_3.length === \"number\" && typeof $KEY_3 !== \"function\")",
        "let arrayLike; if (typeof $KEY_3.length === \"number\") { arrayLike = typeof $KEY_3 !== \"function\"; } else { arrayLike = false; } if (arrayLike)",
    )]);
    assert!(named(&array_guard, "$THREAD_6_140623"));
    refuses(
        &array_guard,
        "$THREAD_6_140623",
        &[
            ("arrayLike = false;", "arrayLike = true;"),
            ("arrayLike = false;", "arrayLike = typeof $KEY_3.length === \"number\";"),
            ("arrayLike = typeof $KEY_3", "arrayLike = typeof other"),
            ("if (arrayLike)", "arrayLike = true; if (arrayLike)"),
        ],
    );
}

#[test]
fn observable_factory_new_proof_refuses_async_and_shape_only_impostors() {
    refuses(INLINE_COPY_INDEX, "$THREAD_6_140623", &[
        ("new $KEY_1($BT =>", "new $KEY_1(async $BT =>"),
        (".schedule(() =>", ".schedule(async () =>"),
        (".then($BW =>", ".then(async $BW =>"),
    ]);
    denied(r#"function $THREAD_6_140623() {
        class C {
            constructor(value) { this._subscriber = value; }
            static of() {} static empty() {} static from() {}
            subscribe() {} pipe() {} map() {} unorderedFlatMap() {}
        }
        let alias = C;
        return alias;
    } $THREAD_6_140623();"#, "$THREAD_6_140623");
}

// Complete input retains bootstrap, captured constructors and operators, carrier
// assignments, and real consumers. The class and linked function must be proved separately.
const INSTALLED_EPOCH: &str = include_str!("fixtures/program/thread-observable-lifecycle.js");
const INSTALLED_EPOCH_FACTORY: &str = "$THREAD_5_96502";
const INSTALLED_EPOCH_CLASS: &str = "$THREAD_4_5962";

fn installed_epoch_variant(from: &str, to: &str) -> String {
    let body = &INSTALLED_EPOCH[116014..120827];
    assert!(body.contains(from), "missing replacement {from}");
    format!("{}{}{}", &INSTALLED_EPOCH[..116014], body.replace(from, to), &INSTALLED_EPOCH[120827..])
}

fn installed_epoch_refuses(changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        let source = installed_epoch_variant(from, to);
        valid_model(&source);
        let result = name_threads(&source, true).expect("valid full-seed model");
        assert_ne!(target_outcome(&result, INSTALLED_EPOCH_FACTORY).evaluator.as_deref(), Some(EVALUATOR), "accepted mutation {from} -> {to}");
        let class = result.report.class_outcomes.iter().find(|o| o.binding.name == INSTALLED_EPOCH_CLASS).expect("original class outcome");
        assert!(!class.applied, "accepted class mutation {from} -> {to}");
    }
}

#[test]
fn observable_factory_installed_epoch_full_seed_proves_class_and_linked_function_separately() {
    valid_model(INSTALLED_EPOCH);
    let result = name_threads(INSTALLED_EPOCH, true).unwrap();
    let function = target_outcome(&result, INSTALLED_EPOCH_FACTORY);
    assert!(function.applied);
    assert_eq!(function.evaluator.as_deref(), Some(EVALUATOR));
    assert_eq!(function.proposed_name.as_deref(), Some(NAME));
    let class = result.report.class_outcomes.iter().find(|o| o.binding.name == INSTALLED_EPOCH_CLASS).unwrap();
    assert!(class.applied);
    assert_eq!(class.proposed_name.as_deref(), Some("Observable"));
    assert!(!result.report.outcomes.iter().any(|o| o.binding.name == INSTALLED_EPOCH_CLASS));
    // Private sentinel payloads and temporary spellings are not evidence.
    let variant = installed_epoch_variant("$AO = 32492;", "$AO = 9;")
        .replace("$AQ", "observableArrayAlias");
    valid_model(&variant);
    assert!(named(&variant, INSTALLED_EPOCH_FACTORY));
}

#[test]
fn observable_factory_installed_epoch_refuses_changed_test_effects_and_loop_order() {
    installed_epoch_refuses(&[
        ("i < $AQ.length", "i <= $AQ.length"),
        ("$AQ = $KEY_3;", "$AQ = other;"),
        ("j = $KEY_6", "j = other"),
        ("k = j.next", "k = j.error"),
        ("i_2 = $KEY_3[i]", "i_2 = $KEY_3[0]"),
        ("k.call(j, i_2)", "k.call(other, i_2)"),
        ("k.call(j, i_2)", "k.call(j, other)"),
        ("k.call(j, i_2)", "(() => k.call(j, i_2))"),
        ("i = Number(i) + 1", "i = Number(i) + 2"),
        ("i = Number(i) + 1, true", "i = Number(i) + 1, false"),
        ("let i = 0;\n\t\t\t\t\t\t\t\t\t\tlet $AQ", "let i = 1;\n\t\t\t\t\t\t\t\t\t\tlet $AQ"),
        ("$AO = 32492;", "(() => { $AO = 32492; });"),
        ("$AP = 3;", "$AP = void 0;"),
        ("$AO = 32492;", "$AO = 32492; other($AO);"),
        ("k.call(j, i_2), i = Number(i) + 1", "i = Number(i) + 1, k.call(j, i_2)"),
    ]);
}

#[test]
fn observable_factory_installed_epoch_refuses_discarded_or_changed_catch_and_sentinels() {
    installed_epoch_refuses(&[
        ("if ($AO === void 0)", "if ($AP === void 0)"),
        ("if ($AP === void 0)", "if ($AP !== void 0)"),
        ("Cannot read properties of undefined (reading '0')", "different failure"),
        ("Cannot read properties of undefined (reading 'undefined')", "different failure"),
        ("$KEY_6.error(e);", "$KEY_6.error(other);"),
        ("$KEY_6.error(e);", "(() => $KEY_6.error(e));"),
        ("$KEY_6.error(e);", "$KEY_6.error(e); return;"),
        ("$AO = 32492;", "$AO = 32492; $AP = 3;"),
        ("if ($AO === void 0) {\n\t\t\t\t\t\t\t\t\t\t\tthrow new TypeError(\"Cannot read properties of undefined (reading '0')\");\n\t\t\t\t\t\t\t\t\t\t}", ""),
        ("if ($AP === void 0) {\n\t\t\t\t\t\t\t\t\t\t\tthrow new TypeError(\"Cannot read properties of undefined (reading 'undefined')\");\n\t\t\t\t\t\t\t\t\t\t}", ""),
        ("let $AO, $AP;", "let $AO = 1, $AP;"),
        ("$AQ = $KEY_3;", "$AQ = $KEY_3; other(() => $AQ);"),
    ]);
}

#[test]
fn observable_factory_installed_epoch_refuses_changed_owner_api_and_captured_contracts() {
    installed_epoch_refuses(&[
        ("let $KEY_0 = $THREAD_4_5962;", "let $KEY_0 = other;"),
        ("return $KEY_0;", "return other;"),
        ("return $KEY_0;", "leak($KEY_0); return $KEY_0;"),
        ("this._subscriber = $AL;", "this._subscriber = other;"),
        ("new $KEY_227($AO)", "new other($AO)"),
        ("new $KEY_158($AP)", "new $KEY_158(other)"),
        ("$KEY_166(this._subscriber, $AR)", "$KEY_166(this._subscriber, other)"),
        ("$AQ[1].call($AQ[0], $AO, $AP)", "$AQ[1].call($AQ[0], other, $AP)"),
        ("$AR[1].call($AR[0], e)", "$AR[1].call($AR[0], other)"),
        ("$KEY_0.complete();", "$KEY_0.next();"),
        ("return this.pipe($KEY_143($AL));", "return this.pipe(other($AL));"),
    ]);
}
