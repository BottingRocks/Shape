function main(){let $KEY_9,$KEY_2,$KEY_7,$KEY_269,$KEY_412,$KEY_6,$KEY_0,$KEY_3,$KEY_431;
function $THREAD_0_152170($BW) {
						let $KEY_9 = $BW.target;
						if ($KEY_9 == null) {
							return void 0;
						}
						if ($KEY_9 != null && $KEY_9.nodeType === 1 && $KEY_9.nodeName === "FORM") {
							let $BX;
							if ($BW.event != null && !$BW.event.isTrusted && "originalTarget" in $BW.event) {
								$BX = $KEY_9;
							} else {
								let $BY = $KEY_9;
								let $BZ;
								for (;;) {
									if ($BY.parentNode != null) {
										$BZ = $BY.parentNode;
										$BZ = $BZ == null || $BZ.nodeType !== 1 || $BZ.nodeName !== "FORM";
									} else {
										$BZ = false;
									}
									if (!$BZ || $BY.parentNode === $KEY_9) break;
									$BY = $BY.parentNode;
								}
								$BX = $BY;
							}
							let $KEY_2 = $BX;
							let $KEY_7;
							function $THREAD_5_100374($CA) {
								try {
									$KEY_2.removeEventListener("submit", $KEY_7);
									if ($CA != null && $CA.defaultPrevented) {
										return void 0;
									} else {
										$KEY_269.notify({
											form: $KEY_9,
											event: $CA
										});
										return void 0;
									}
								} catch (e) {
									$KEY_412.notify("2.27:" + e);
									return void 0;
								}
							}
							$KEY_7 = $THREAD_5_100374;
							if ($BW.event == null) {
								$THREAD_5_100374();
								$KEY_6 = null;
								$KEY_0 = null;
								$KEY_3 = null;
								$KEY_2.removeEventListener("submit", $KEY_7);
								return void 0;
							} else {
								function $THREAD_5_57355($CA) {
									try {
										if (!$CA.isTrusted && !$CA.cancelable) {
											return void 0;
										}
										if ($CA.type !== "submit" || $CA.target !== $KEY_9) {
											return void 0;
										} else {
											$KEY_2.removeEventListener("submit", $KEY_7);
											$KEY_0 = null;
											$KEY_6 = null;
											$KEY_431.notify($KEY_9);
											return void 0;
										}
									} catch (e) {
										$KEY_412.notify("2.28:" + e);
										return void 0;
									}
								}
								function $THREAD_3_108742($CA) {
									if ($CA.type !== "submit" || $CA.target !== $KEY_9) {
										return void 0;
									} else {
										$KEY_7($CA);
										$KEY_6 = null;
										return void 0;
									}
								}
								$KEY_6 = $THREAD_3_108742;
								function $THREAD_8_46728($CA) {
									if ($CA.type !== "submit" || $CA.target !== $KEY_9) {
										return void 0;
									} else {
										$KEY_7($CA);
										$KEY_0 = null;
										return void 0;
									}
								}
								$KEY_0 = $THREAD_8_46728;
								$KEY_3 = $THREAD_5_57355;
								$KEY_2.addEventListener("submit", $KEY_7);
								return void 0;
							}
						} else {
							return void 0;
						}
					}
}main();
