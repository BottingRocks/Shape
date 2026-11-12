function main(){let $KEY_12,$KEY_37,$KEY_92,$KEY_165,$KEY_176,$KEY_182,$KEY_233,$KEY_352,$KEY_399,$KEY_402,$KEY_403,$KEY_414;
function $THREAD_9_57189() {
					let $KEY_0, $KEY_1, $KEY_4;
					function $THREAD_5_110080($CB) {
						let $KEY_7 = $CB.target;
						if ($KEY_7 == null) {
							return void 0;
						}
						if ($KEY_7 != null && $KEY_7.nodeType === 1 && $KEY_7.nodeName === "FORM") {
							let $CC;
							if ($CB.event != null && !$CB.event.isTrusted && "originalTarget" in $CB.event) {
								$CC = $KEY_7;
							} else {
								let $CD = $KEY_7;
								let $CE = 9;
								for (;;) {
									if ($CD.parentNode != null) {
										$CE = $CD.parentNode;
										$CE = $CE == null || $CE.nodeType !== 1 || $CE.nodeName !== "FORM";
									} else {
										$CE = false;
									}
									if (!$CE || $CD.parentNode === $KEY_7) break;
									$CD = $CD.parentNode;
								}
								$CC = $CD;
							}
							let $KEY_3 = $CC;
							let $KEY_8;
							function $THREAD_8_52378($CF) {
								try {
									$KEY_3.removeEventListener("submit", $KEY_8);
									if ($CF != null && $CF.defaultPrevented) {
										return void 0;
									} else {
										$KEY_176.notify({
											form: $KEY_7,
											event: $CF
										});
										return void 0;
									}
								} catch (e) {
									$KEY_92.notify("2.27:" + e);
									return void 0;
								}
							}
							$KEY_8 = $THREAD_8_52378;
							if ($CB.event == null) {
								$THREAD_8_52378();
								$KEY_0 = null;
								$KEY_4 = null;
								$KEY_1 = null;
								$KEY_3.removeEventListener("submit", $KEY_8);
								return void 0;
							} else {
								function $THREAD_5_46332($CF) {
									if ($CF.type !== "submit" || $CF.target !== $KEY_7) {
										return void 0;
									} else {
										$KEY_8($CF);
										$KEY_0 = null;
										return void 0;
									}
								}
								$KEY_0 = $THREAD_5_46332;
								function $THREAD_0_121572($CF) {
									if ($CF.type !== "submit" || $CF.target !== $KEY_7) {
										return void 0;
									} else {
										$KEY_8($CF);
										$KEY_4 = null;
										return void 0;
									}
								}
								$KEY_4 = $THREAD_0_121572;
								function $THREAD_7_25094($CF) {
									try {
										if (!$CF.isTrusted && !$CF.cancelable) {
											return void 0;
										}
										if ($CF.type !== "submit" || $CF.target !== $KEY_7) {
											return void 0;
										} else {
											$KEY_3.removeEventListener("submit", $KEY_8);
											$KEY_4 = null;
											$KEY_0 = null;
											$KEY_37.notify($KEY_7);
											return void 0;
										}
									} catch (e) {
										$KEY_92.notify("2.28:" + e);
										return void 0;
									}
								}
								$KEY_1 = $THREAD_7_25094;
								$KEY_3.addEventListener("submit", $KEY_8);
								return void 0;
							}
						} else {
							return void 0;
						}
					}
					let $KEY_2 = $THREAD_5_110080;
					if ($KEY_402) {
						return void 0;
					} else {
						try {
							function $THREAD_3_154475($CB) {
								let $CC = {
									event: void 0,
									target: void 0
								};
								$CC.event = $CB;
								$CC.target = $CB.target;
								return $KEY_2($CC);
							}
							$KEY_352 = $THREAD_3_154475;
							$KEY_182.addEventListener("submit", $THREAD_3_154475, true);
						} catch (e) {
							$KEY_92.notify("2.21:" + e);
							return void 0;
						}
						if ($KEY_399 != null && $KEY_399.onBeforeInvoke != null) {
							$KEY_399.onBeforeInvoke.register(($CB, $CC) => {
								try {
									if ($CC != null && $CC.bypassResult != null) {
										return $CC;
									} else {
										$KEY_2({ target: $CB.thisObj });
										return $CC;
									}
								} catch (e) {
									$KEY_92.notify("2.22:" + e);
									return $CC;
								}
							});
						}
						if ($KEY_233 != null && $KEY_233.onBeforeInvoke != null) {
							$KEY_233.onBeforeInvoke.register(($CB, $CC) => {
								try {
									if ($CC != null && $CC.bypassResult != null) {
										return $CC;
									} else {
										if ($KEY_0 != null) {
											$KEY_0($CB.thisObj);
										}
										return $CC;
									}
								} catch (e) {
									$KEY_92.notify("2.23:" + e);
									return $CC;
								}
							});
						}
						if ($KEY_165 != null && $KEY_165.onBeforeInvoke != null) {
							$KEY_165.onBeforeInvoke.register(($CB, $CC) => {
								try {
									if ($CC != null && $CC.bypassResult != null) {
										return $CC;
									} else {
										if ($KEY_4 != null) {
											$KEY_4($CB.thisObj);
										}
										return $CC;
									}
								} catch (e) {
									$KEY_92.notify("2.24:" + e);
									return $CC;
								}
							});
						}
						if ($KEY_414 != null && $KEY_414.onBeforeInvoke != null) {
							$KEY_414.onBeforeInvoke.register(($CB, $CC) => {
								try {
									if ($CC != null && $CC.bypassResult != null) {
										return $CC;
									} else {
										if ($KEY_1 != null) {
											$KEY_1($CB.thisObj);
										}
										return $CC;
									}
								} catch (e) {
									$KEY_92.notify("2.25:" + e);
									return $CC;
								}
							});
						}
						if ($KEY_403 != null && $KEY_403.onBeforeSet != null) {
							$KEY_403.onBeforeSet.register(($CB, $CC) => {
								try {
									if ($CC != null && $CC.bypassResult != null) {
										return $CC;
									}
									if ($CB.param == null) {
										return $CC;
									} else {
										$KEY_12.Event.prototype.stopPropagation.call($CB.thisObj);
										return $CC;
									}
								} catch (e) {
									$KEY_92.notify("2.26:" + e);
									return $CC;
								}
							});
						}
						$KEY_402 = true;
						return void 0;
					}
				}
}main();
