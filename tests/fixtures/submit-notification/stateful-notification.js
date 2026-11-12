function main(){let $KEY_240,$KEY_390,$KEY_215,$KEY_78,$KEY_267,$KEY_128,$KEY_59,$KEY_52,$KEY_411,$KEY_17,$KEY_434,$KEY_283;
function $THREAD_7_15988() {
					let $KEY_1, $KEY_4, $KEY_5;
					function $THREAD_9_60197($CC) {
						let $KEY_9 = $CC.target;
						if ($KEY_9 == null) {
							return void 0;
						}
						if ($KEY_9 != null && $KEY_9.nodeType === 1 && $KEY_9.nodeName === "FORM") {
							let $CD;
							if ($CC.event != null && !$CC.event.isTrusted && "originalTarget" in $CC.event) {
								$CD = $KEY_9;
							} else {
								let $CE = $KEY_9;
								let $CF;
								for (;;) {
									if ($CE.parentNode != null) {
										$CF = $CE.parentNode;
										$CF = $CF == null || $CF.nodeType !== 1 || $CF.nodeName !== "FORM";
									} else {
										$CF = false;
									}
									if (!$CF || $CE.parentNode === $KEY_9) break;
									$CE = $CE.parentNode;
								}
								$CD = $CE;
							}
							let $KEY_3 = $CD;
							let $KEY_7;
							function $THREAD_4_139072($CG) {
								try {
									$KEY_3.removeEventListener("submit", $KEY_7);
									if ($CG != null && $CG.defaultPrevented) {
										return void 0;
									} else {
										$KEY_240.notify({
											form: $KEY_9,
											event: $CG
										});
										return void 0;
									}
								} catch (e) {
									$KEY_390.notify("2.27:" + e);
									return void 0;
								}
							}
							$KEY_7 = $THREAD_4_139072;
							if ($CC.event == null) {
								$KEY_7();
								$KEY_5 = null;
								$KEY_4 = null;
								$KEY_1 = null;
								$KEY_3.removeEventListener("submit", $KEY_7);
								return void 0;
							} else {
								function $THREAD_1_86348($CG) {
									if ($CG.type !== "submit" || $CG.target !== $KEY_9) {
										return void 0;
									} else {
										$KEY_7($CG);
										$KEY_4 = null;
										return void 0;
									}
								}
								function $THREAD_7_110216($CG) {
									if ($CG.type !== "submit" || $CG.target !== $KEY_9) {
										return void 0;
									} else {
										$KEY_7($CG);
										$KEY_5 = null;
										return void 0;
									}
								}
								$KEY_5 = $THREAD_7_110216;
								$KEY_4 = $THREAD_1_86348;
								function $THREAD_7_116708($CG) {
									try {
										if (!$CG.isTrusted && !$CG.cancelable) {
											return void 0;
										}
										if ($CG.type !== "submit" || $CG.target !== $KEY_9) {
											return void 0;
										} else {
											$KEY_3.removeEventListener("submit", $KEY_7);
											$KEY_4 = null;
											$KEY_5 = null;
											$KEY_215.notify($KEY_9);
											return void 0;
										}
									} catch (e) {
										$KEY_390.notify("2.28:" + e);
										return void 0;
									}
								}
								$KEY_1 = $THREAD_7_116708;
								$KEY_3.addEventListener("submit", $KEY_7);
								return void 0;
							}
						} else {
							return void 0;
						}
					}
					let $KEY_3 = $THREAD_9_60197;
					if ($KEY_78) {
						return void 0;
					} else {
						try {
							function $THREAD_8_46474($CC) {
								let $CD = {
									event: void 0,
									target: void 0
								};
								$CD.event = $CC;
								$CD.target = $CC.target;
								return $KEY_3($CD);
							}
							$KEY_267 = $THREAD_8_46474;
							$KEY_128.addEventListener("submit", $THREAD_8_46474, true);
						} catch (e) {
							$KEY_390.notify("2.21:" + e);
							return void 0;
						}
						if ($KEY_59 != null && $KEY_59.onBeforeInvoke != null) {
							$KEY_59.onBeforeInvoke.register(($CC, $CD) => {
								try {
									if ($CD != null && $CD.bypassResult != null) {
										return $CD;
									} else {
										$KEY_3({ target: $CC.thisObj });
										return $CD;
									}
								} catch (e) {
									$KEY_390.notify("2.22:" + e);
									return $CD;
								}
							});
						}
						if ($KEY_52 != null && $KEY_52.onBeforeInvoke != null) {
							$KEY_52.onBeforeInvoke.register(($CC, $CD) => {
								try {
									if ($CD != null && $CD.bypassResult != null) {
										return $CD;
									} else {
										if ($KEY_5 != null) {
											$KEY_5($CC.thisObj);
										}
										return $CD;
									}
								} catch (e) {
									$KEY_390.notify("2.23:" + e);
									return $CD;
								}
							});
						}
						if ($KEY_411 != null && $KEY_411.onBeforeInvoke != null) {
							$KEY_411.onBeforeInvoke.register(($CC, $CD) => {
								try {
									if ($CD != null && $CD.bypassResult != null) {
										return $CD;
									} else {
										if ($KEY_4 != null) {
											$KEY_4($CC.thisObj);
										}
										return $CD;
									}
								} catch (e) {
									$KEY_390.notify("2.24:" + e);
									return $CD;
								}
							});
						}
						if ($KEY_17 != null && $KEY_17.onBeforeInvoke != null) {
							$KEY_17.onBeforeInvoke.register(($CC, $CD) => {
								try {
									if ($CD != null && $CD.bypassResult != null) {
										return $CD;
									} else {
										if ($KEY_1 != null) {
											$KEY_1($CC.thisObj);
										}
										return $CD;
									}
								} catch (e) {
									$KEY_390.notify("2.25:" + e);
									return $CD;
								}
							});
						}
						if ($KEY_434 != null && $KEY_434.onBeforeSet != null) {
							$KEY_434.onBeforeSet.register(($CC, $CD) => {
								try {
									if ($CD != null && $CD.bypassResult != null) {
										return $CD;
									}
									if ($CC.param == null) {
										return $CD;
									} else {
										$KEY_283.Event.prototype.stopPropagation.call($CC.thisObj);
										return $CD;
									}
								} catch (e) {
									$KEY_390.notify("2.26:" + e);
									return $CD;
								}
							});
						}
						$KEY_78 = true;
						return void 0;
					}
				}
}main();
