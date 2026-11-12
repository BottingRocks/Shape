//                       Observable protocol with owned operator epochs.
// Captured dependencies are local callable stubs; no dependency behavior is evidence.
function bootstrap(){
class Observer {}
class Subscription {}
function teardown(a,b){}
function mapOperator(a){}
function flatOperator(a){}
const $KEY_351 = Observer, $KEY_168 = Subscription;
const $KEY_108 = teardown, $KEY_433 = mapOperator, $KEY_437 = flatOperator;
function firstMaker() {
					class $THREAD_2_127309 {
						constructor($AL) {
							if (typeof $AL !== "function") {
								throw new TypeError();
							} else {
								this._subscriber = $AL;
								return void 0;
							}
						}
						@($THREAD_2_113070(x)(source))
static of() {
							let $KEY_4;
							let $KEY_1 = arguments.length;
							$KEY_4 = new Array($KEY_1);
							let i = 0;
							for (; i < $KEY_1; i++) {
								$KEY_4[i] = arguments[i];
							}
							return new $KEY_0($AL => {
								let $KEY_0 = $AL;
								$KEY_4.forEach($AM => $KEY_0.next($AM));
								$KEY_0.complete();
							});
						}
						static empty() {
							return new $KEY_0($AL => {
								$AL.complete();
							});
						}
					}
					let $KEY_0 = $THREAD_2_127309;
					function $THREAD_9_146693($AL, $AM) {
						let $KEY_2 = $AM;
						let $KEY_3 = $AL;
						if ($AL != null) {
							if ($AL instanceof $KEY_0) {
								return $AL;
							}
							if (typeof Promise !== "undefined" && $AL instanceof Promise) {
								return new $KEY_0($AN => {
									let $KEY_0 = $AN;
									$KEY_3.then($AO => {
										$KEY_0.next($AO);
										$KEY_0.complete();
									}).catch($AO => {
										$KEY_0.error($AO);
									});
								});
							}
							if (typeof $KEY_3.length === "number" && typeof $KEY_3 !== "function") {
								let $KEY_1 = $KEY_3;
								return new $KEY_0($AN => {
									let $KEY_3 = $AN;
									if ($KEY_2 != null) {
										let $KEY_0 = 0;
										$KEY_2.schedule(() => {
											try {
												if ($KEY_0 >= $KEY_1.length) {
													$KEY_3.complete();
													return false;
												} else {
													$KEY_3.next($KEY_1[$KEY_0]);
													++$KEY_0;
													return true;
												}
											} catch (e) {
												$KEY_3.error(e);
												return false;
											}
										});
									} else {
										try {
											let $AO = 0;
											while ($AO < $KEY_1.length) {
												$KEY_3.next($KEY_1[$AO]);
												++$AO;
											}
											$KEY_3.complete();
										} catch (e) {
											$KEY_3.error(e);
										}
									}
								});
							}
							if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_3[Symbol.iterator]) {
								return new $KEY_0($AN => {
									let $KEY_1 = $AN;
									let $KEY_5;
									try {
										$KEY_5 = $KEY_3[Symbol.iterator]();
										if ($KEY_2 != null) {
											$KEY_2.schedule(() => {
												try {
													let $AP = $KEY_5.next();
													if ($AP.done) {
														$KEY_1.complete();
														return false;
													} else {
														$KEY_1.next($AP.value);
														return true;
													}
												} catch (e) {
													$KEY_1.error(e);
													return false;
												}
											});
										} else {
											let $AO, i;
											do {
												$AO = $KEY_5.next();
											} while (!$AO.done && (i = $KEY_1, (($PARAMA, $PARAMB, $PARAMC, $PARAMD) => {
										$PARAMB.call($PARAMA, $PARAMC[$PARAMD]);
									})(i, (i.next), $AO, "value"), true));
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
					$KEY_0.from = $THREAD_9_146693;
					function $THREAD_9_152576($AL, $AM, $AN) {
						"use strict";
						let $AO;
						if (typeof $AL === "function") {
							$AO = $AL;
						} else {
							$AO = $AL ? $AL.next : void 0;
						}
						let $AP = {
							start: typeof $AL === "object" ? $AL.start : void 0,
							next: void 0,
							error: void 0,
							complete: void 0
						};
						$AP.next = $AO;
						$AP.error = typeof $AL === "object" ? $AL.error : $AM;
						$AP.complete = typeof $AL === "object" ? $AL.complete : $AN;
						if (this._subscriber == null) {
							throw new TypeError();
						} else {
							let $AQ = new $KEY_351($AP);
							if (typeof $AP.start === "function") {
								$AP.start($AQ);
							}
							let $AR = new $KEY_168($AQ);
							try {
								$AQ._cleanup = $KEY_108(this._subscriber, $AR);
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
					$KEY_0.prototype.subscribe = $THREAD_9_152576;
					function $THREAD_6_23756() {
						"use strict";
						let $KEY_0 = arguments.length;
						let $AL = new Array($KEY_0);
						let i = 0;
						for (; i < $KEY_0; i++) {
							$AL[i] = arguments[i];
						}
						return $AL.reduce(($AM, $AN) => $AN($AM), this);
					}
					$KEY_0.prototype.pipe = $THREAD_6_23756;
					function $THREAD_0_111722($AL) {
						"use strict";
						return this.pipe($KEY_433($AL));
					}
					$KEY_0.prototype.map = $THREAD_0_111722;
					function $THREAD_7_146447($AL) {
						"use strict";
						return this.pipe($KEY_437($AL));
					}
					$KEY_0.prototype.unorderedFlatMap = $THREAD_7_146447;
					return $KEY_0;
				}
function secondMaker() {
					class $THREAD_2_127309 {
						constructor($AL) {
							if (typeof $AL !== "function") {
								throw new TypeError();
							} else {
								this._subscriber = $AL;
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
							return new $KEY_0($AL => {
								let $KEY_0 = $AL;
								$KEY_4.forEach($AM => $KEY_0.next($AM));
								$KEY_0.complete();
							});
						}
						static empty() {
							return new $KEY_0($AL => {
								$AL.complete();
							});
						}
					}
					let $KEY_0 = $THREAD_2_127309;
					function $THREAD_9_146693($AL, $AM) {
						let $KEY_2 = $AM;
						let $KEY_3 = $AL;
						if ($AL != null) {
							if ($AL instanceof $KEY_0) {
								return $AL;
							}
							if (typeof Promise !== "undefined" && $AL instanceof Promise) {
								return new $KEY_0($AN => {
									let $KEY_0 = $AN;
									$KEY_3.then($AO => {
										$KEY_0.next($AO);
										$KEY_0.complete();
									}).catch($AO => {
										$KEY_0.error($AO);
									});
								});
							}
							if (typeof $KEY_3.length === "number" && typeof $KEY_3 !== "function") {
								let $KEY_1 = $KEY_3;
								return new $KEY_0($AN => {
									let $KEY_3 = $AN;
									if ($KEY_2 != null) {
										let $KEY_0 = 0;
										$KEY_2.schedule(() => {
											try {
												if ($KEY_0 >= $KEY_1.length) {
													$KEY_3.complete();
													return false;
												} else {
													$KEY_3.next($KEY_1[$KEY_0]);
													++$KEY_0;
													return true;
												}
											} catch (e) {
												$KEY_3.error(e);
												return false;
											}
										});
									} else {
										try {
											let $AO = 0;
											while ($AO < $KEY_1.length) {
												$KEY_3.next($KEY_1[$AO]);
												++$AO;
											}
											$KEY_3.complete();
										} catch (e) {
											$KEY_3.error(e);
										}
									}
								});
							}
							if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_3[Symbol.iterator]) {
								return new $KEY_0($AN => {
									let $KEY_1 = $AN;
									let $KEY_5;
									try {
										$KEY_5 = $KEY_3[Symbol.iterator]();
										if ($KEY_2 != null) {
											$KEY_2.schedule(() => {
												try {
													let $AP = $KEY_5.next();
													if ($AP.done) {
														$KEY_1.complete();
														return false;
													} else {
														$KEY_1.next($AP.value);
														return true;
													}
												} catch (e) {
													$KEY_1.error(e);
													return false;
												}
											});
										} else {
											let $AO, i;
											do {
												$AO = $KEY_5.next();
											} while (!$AO.done && (i = $KEY_1, (($PARAMA, $PARAMB, $PARAMC, $PARAMD) => {
										$PARAMB.call($PARAMA, $PARAMC[$PARAMD]);
									})(i, (i.next), $AO, "value"), true));
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
					$KEY_0.from = $THREAD_9_146693;
					function $THREAD_9_152576($AL, $AM, $AN) {
						"use strict";
						let $AO;
						if (typeof $AL === "function") {
							$AO = $AL;
						} else {
							$AO = $AL ? $AL.next : void 0;
						}
						let $AP = {
							start: typeof $AL === "object" ? $AL.start : void 0,
							next: void 0,
							error: void 0,
							complete: void 0
						};
						$AP.next = $AO;
						$AP.error = typeof $AL === "object" ? $AL.error : $AM;
						$AP.complete = typeof $AL === "object" ? $AL.complete : $AN;
						if (this._subscriber == null) {
							throw new TypeError();
						} else {
							let $AQ = new $KEY_351($AP);
							if (typeof $AP.start === "function") {
								$AP.start($AQ);
							}
							let $AR = new $KEY_168($AQ);
							try {
								$AQ._cleanup = $KEY_108(this._subscriber, $AR);
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
					$KEY_0.prototype.subscribe = $THREAD_9_152576;
					function $THREAD_6_23756() {
						"use strict";
						let $KEY_0 = arguments.length;
						let $AL = new Array($KEY_0);
						let i = 0;
						for (; i < $KEY_0; i++) {
							$AL[i] = arguments[i];
						}
						return $AL.reduce(($AM, $AN) => $AN($AM), this);
					}
					$KEY_0.prototype.pipe = $THREAD_6_23756;
					function $THREAD_0_111722($AL) {
						"use strict";
						return this.pipe($KEY_433($AL));
					}
					$KEY_0.prototype.map = $THREAD_0_111722;
					function $THREAD_7_146447($AL) {
						"use strict";
						return this.pipe($KEY_437($AL));
					}
					$KEY_0.prototype.unorderedFlatMap = $THREAD_7_146447;
					return $KEY_0;
				}
function thirdMaker() {
					class $THREAD_2_127309 {
						constructor($AL) {
							if (typeof $AL !== "function") {
								throw new TypeError();
							} else {
								this._subscriber = $AL;
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
							return new $KEY_0($AL => {
								let $KEY_0 = $AL;
								$KEY_4.forEach($AM => $KEY_0.next($AM));
								$KEY_0.complete();
							});
						}
						static empty() {
							return new $KEY_0($AL => {
								$AL.complete();
							});
						}
					}
					let $KEY_0 = $THREAD_2_127309;
					function $THREAD_9_146693($AL, $AM) {
						let $KEY_2 = $AM;
						let $KEY_3 = $AL;
						if ($AL != null) {
							if ($AL instanceof $KEY_0) {
								return $AL;
							}
							if (typeof Promise !== "undefined" && $AL instanceof Promise) {
								return new $KEY_0($AN => {
									let $KEY_0 = $AN;
									$KEY_3.then($AO => {
										$KEY_0.next($AO);
										$KEY_0.complete();
									}).catch($AO => {
										$KEY_0.error($AO);
									});
								});
							}
							if (typeof $KEY_3.length === "number" && typeof $KEY_3 !== "function") {
								let $KEY_1 = $KEY_3;
								return new $KEY_0($AN => {
									let $KEY_3 = $AN;
									if ($KEY_2 != null) {
										let $KEY_0 = 0;
										$KEY_2.schedule(() => {
											try {
												if ($KEY_0 >= $KEY_1.length) {
													$KEY_3.complete();
													return false;
												} else {
													$KEY_3.next($KEY_1[$KEY_0]);
													++$KEY_0;
													return true;
												}
											} catch (e) {
												$KEY_3.error(e);
												return false;
											}
										});
									} else {
										try {
											let $AO = 0;
											while ($AO < $KEY_1.length) {
												$KEY_3.next($KEY_1[$AO]);
												++$AO;
											}
											$KEY_3.complete();
										} catch (e) {
											$KEY_3.error(e);
										}
									}
								});
							}
							if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_3[Symbol.iterator]) {
								return new $KEY_0($AN => {
									let $KEY_1 = $AN;
									let $KEY_5;
									try {
										$KEY_5 = $KEY_3[Symbol.iterator]();
										if ($KEY_2 != null) {
											$KEY_2.schedule(() => {
												try {
													let $AP = $KEY_5.next();
													if ($AP.done) {
														$KEY_1.complete();
														return false;
													} else {
														$KEY_1.next($AP.value);
														return true;
													}
												} catch (e) {
													$KEY_1.error(e);
													return false;
												}
											});
										} else {
											let $AO, i;
											do {
												$AO = $KEY_5.next();
											} while (!$AO.done && (i = $KEY_1, (($PARAMA, $PARAMB, $PARAMC, $PARAMD) => {
										$PARAMB.call($PARAMA, $PARAMC[$PARAMD]);
									})(i, (i.next), $AO, "value"), true));
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
					$KEY_0.from = $THREAD_9_146693;
					function $THREAD_9_152576($AL, $AM, $AN) {
						"use strict";
						let $AO;
						if (typeof $AL === "function") {
							$AO = $AL;
						} else {
							$AO = $AL ? $AL.next : void 0;
						}
						let $AP = {
							start: typeof $AL === "object" ? $AL.start : void 0,
							next: void 0,
							error: void 0,
							complete: void 0
						};
						$AP.next = $AO;
						$AP.error = typeof $AL === "object" ? $AL.error : $AM;
						$AP.complete = typeof $AL === "object" ? $AL.complete : $AN;
						if (this._subscriber == null) {
							throw new TypeError();
						} else {
							let $AQ = new $KEY_351($AP);
							if (typeof $AP.start === "function") {
								$AP.start($AQ);
							}
							let $AR = new $KEY_168($AQ);
							try {
								$AQ._cleanup = $KEY_108(this._subscriber, $AR);
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
					$KEY_0.prototype.subscribe = $THREAD_9_152576;
					function $THREAD_6_23756() {
						"use strict";
						let $KEY_0 = arguments.length;
						let $AL = new Array($KEY_0);
						let i = 0;
						for (; i < $KEY_0; i++) {
							$AL[i] = arguments[i];
						}
						return $AL.reduce(($AM, $AN) => $AN($AM), this);
					}
					$KEY_0.prototype.pipe = $THREAD_6_23756;
					function $THREAD_0_111722($AL) {
						"use strict";
						return this.pipe($KEY_433($AL));
					}
					$KEY_0.prototype.map = $THREAD_0_111722;
					function $THREAD_7_146447($AL) {
						"use strict";
						return this.pipe($KEY_437($AL));
					}
					$KEY_0.prototype.unorderedFlatMap = $THREAD_7_146447;
					return $KEY_0;
				}
let Constructor;
function $THREAD_2_113070($DF) {
					let $KEY_0 = $DF;
					function $THREAD_5_41873($DG) {
						let $KEY_1 = $DG;
						return new Constructor($DH => {
							function $THREAD_6_83118() {
								$KEY_4.complete();
							}
							let $KEY_4 = $DH;
							function $THREAD_1_134500($DJ) {
								try {
									$KEY_4.next($KEY_0($DJ));
								} catch (e) {
									$KEY_4.error(e);
								}
							}
							let $DI = { next: $THREAD_1_134500 };
							function $THREAD_2_50453($DJ) {
								$KEY_4.error($DJ);
							}
							$DI.error = $THREAD_2_50453;
							$DI.complete = $THREAD_6_83118;
							let $KEY_3 = $KEY_1.subscribe($DI);
							function $THREAD_0_37257() {
								return $KEY_3.unsubscribe();
							}
							return $THREAD_0_37257;
						});
					}
					return $THREAD_5_41873;
				}function $THREAD_2_127320($DF) {
					let $KEY_0 = $DF;
					function $THREAD_6_80930() {
						let $KEY_4 = arguments.length;
						let $KEY_1 = new Array($KEY_4);
						let i = 0;
						for (; i < $KEY_4; i++) {
							$KEY_1[i] = arguments[i];
						}
						return new Constructor($DG => {
							let $KEY_2 = $DG;
							if (typeof $KEY_0 !== "function") {
								$KEY_2.error(new TypeError());
								return void 0;
							} else {
								$KEY_0.apply(null, $KEY_1.concat($DH => {
									$KEY_2.next($DH);
									$KEY_2.complete();
								}, $DH => {
									$KEY_2.error($DH);
								}));
								return void 0;
							}
						});
					}
					return $THREAD_6_80930;
				}if (branch) { Constructor = firstMaker(); } else { Constructor = secondMaker(); } Constructor = thirdMaker(); publish($THREAD_2_113070);}
bootstrap();