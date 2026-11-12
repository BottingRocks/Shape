function $THREAD_1_28126() {
					class $THREAD_0_14271 {
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
							let $KEY_0 = arguments.length;
							$KEY_6 = new Array($KEY_0);
							let i = 0;
							for (; i < $KEY_0; i++) {
								$KEY_6[i] = arguments[i];
							}
							return new $KEY_1($AI => {
								let $KEY_0 = $AI;
								$KEY_6.forEach($AJ => $KEY_0.next($AJ));
								$KEY_0.complete();
							});
						}
						static empty() {
							return new $KEY_1($AI => {
								$AI.complete();
							});
						}
					}
					let $KEY_1 = $THREAD_0_14271;
					function $THREAD_7_90329($AI, $AJ) {
						let $KEY_2;
						let $KEY_3 = $AI;
						let $KEY_4 = $AJ;
						if ($AI != null) {
							if ($AI instanceof $KEY_1) {
								return $AI;
							}
							if (typeof Promise !== "undefined" && $AI instanceof Promise) {
								return new $KEY_1($AK => {
									let $KEY_0 = $AK;
									$KEY_3.then($AL => {
										$KEY_0.next($AL);
										$KEY_0.complete();
									}).catch($AL => {
										$KEY_0.error($AL);
									});
								});
							}
							if (typeof $KEY_3.length === "number" && typeof $KEY_3 !== "function") {
								$KEY_2 = $KEY_3;
								return new $KEY_1($AK => {
									let $KEY_1 = $AK;
									let $KEY_5;
									if ($KEY_4 != null) {
										$KEY_5 = 0;
										$KEY_4.schedule(() => {
											try {
												if ($KEY_5 >= $KEY_2.length) {
													$KEY_1.complete();
													return false;
												} else {
													$KEY_1.next($KEY_2[$KEY_5]);
													++$KEY_5;
													return true;
												}
											} catch (e) {
												$KEY_1.error(e);
												return false;
											}
										});
									} else {
										let $AL, $AM;
										try {
											let i = 0;
											let j;
											do {
												$AL = 72388;
												$AM = 2;
											} while (i < $KEY_2.length && (j = $KEY_1, (($PARAMA, $PARAMB, $PARAMC, $PARAMD) => {
										$PARAMB.call($PARAMA, $PARAMC[$PARAMD]);
									})(j, (j.next), $KEY_2, i), i = Number(i) + 1, true));
											$KEY_1.complete();
										} catch (e) {
											$KEY_1.error(e);
											if ($AL === void 0) {
												throw new TypeError("Cannot read properties of undefined (reading '0')");
											}
											if ($AM === void 0) {
												throw new TypeError("Cannot read properties of undefined (reading 'undefined')");
											}
										}
									}
								});
							}
							if (typeof Symbol !== "undefined" && Symbol.iterator && $KEY_3[Symbol.iterator]) {
								return new $KEY_1($AK => {
									let $KEY_0 = $AK;
									let $KEY_2;
									try {
										$KEY_2 = $KEY_3[Symbol.iterator]();
										if ($KEY_4 != null) {
											$KEY_4.schedule(() => {
												try {
													let $AM = $KEY_2.next();
													if ($AM.done) {
														$KEY_0.complete();
														return false;
													} else {
														$KEY_0.next($AM.value);
														return true;
													}
												} catch (e) {
													$KEY_0.error(e);
													return false;
												}
											});
										} else {
											for (;;) {
												let $AL = $KEY_2.next();
												if ($AL.done) break;
												$KEY_0.next($AL.value);
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
					$KEY_1.from = $THREAD_7_90329;
					function $THREAD_8_102965($AI, $AJ, $AK) {
						"use strict";
						let $AL = {
							start: typeof $AI === "object" ? $AI.start : void 0,
							next: typeof $AI === "function" ? $AI : $AI ? $AI.next : void 0,
							error: void 0,
							complete: void 0
						};
						$AL.error = typeof $AI === "object" ? $AI.error : $AJ;
						$AL.complete = typeof $AI === "object" ? $AI.complete : $AK;
						if (this._subscriber == null) {
							throw new TypeError();
						} else {
							let $AM = new $KEY_16($AL);
							if (typeof $AL.start === "function") {
								let $AN = [$AL.start, $AL.start.call];
								$AN[1].call($AN[0], $AL, $AM);
							}
							let $AO = new $KEY_223($AM);
							try {
								$AM._cleanup = $KEY_56(this._subscriber, $AO);
							} catch (e) {
								$AO = [$AO, $AO.error];
								$AO[1].call($AO[0], e);
							}
							if ($AM._observer === void 0) {
								let $AP = $AM._cleanup;
								if ($AP !== void 0) {
									$AM._cleanup = void 0;
									try {
										$AP();
									} catch (e_2) {}
								}
							}
							return $AM;
						}
					}
					$KEY_1.prototype.subscribe = $THREAD_8_102965;
					function $THREAD_1_66922() {
						"use strict";
						let $KEY_4 = arguments.length;
						let $AI = new Array($KEY_4);
						let i = 0;
						for (; i < $KEY_4; i++) {
							$AI[i] = arguments[i];
						}
						return $AI.reduce(($AJ, $AK) => $AK($AJ), this);
					}
					$KEY_1.prototype.pipe = $THREAD_1_66922;
					function $THREAD_0_55348($AI) {
						"use strict";
						return this.pipe($KEY_30($AI));
					}
					$KEY_1.prototype.map = $THREAD_0_55348;
					function $THREAD_9_4556($AI) {
						"use strict";
						return this.pipe($KEY_145($AI));
					}
					$KEY_1.prototype.unorderedFlatMap = $THREAD_9_4556;
					return $KEY_1;
				}