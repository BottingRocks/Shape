function main(){let $KEY_1,$KEY_6,$KEY_4,$KEY_328,$KEY_363,$KEY_0,$KEY_5,$KEY_3,$KEY_33;
function $THREAD_9_124646($AX) {
						let $KEY_1 = $AX.target;
						if ($KEY_1 == null) {
							return void 0;
						}
						if ($KEY_1 != null && $KEY_1.nodeType === 1 && $KEY_1.nodeName === "FORM") {
							let $AY;
							if ($AX.event != null && !$AX.event.isTrusted && "originalTarget" in $AX.event) {
								$AY = $KEY_1;
							} else {
								let $AZ = $KEY_1;
								let $BA;
								for (;;) {
									if ($AZ.parentNode != null) {
										$BA = $AZ.parentNode;
										$BA = $BA == null || $BA.nodeType !== 1 || $BA.nodeName !== "FORM";
									} else {
										$BA = false;
									}
									if (!$BA || $AZ.parentNode === $KEY_1) break;
									$AZ = $AZ.parentNode;
								}
								$AY = $AZ;
							}
							let $KEY_6 = $AY;
							let $KEY_4;
							function $THREAD_7_28660($BB) {
								try {
									$KEY_6.removeEventListener("submit", $KEY_4);
									if ($BB != null && $BB.defaultPrevented) {
										return void 0;
									} else {
										$KEY_328.notify({
											form: $KEY_1,
											event: $BB
										});
										return void 0;
									}
								} catch (e) {
									$KEY_363.notify("2.27:" + e);
									return void 0;
								}
							}
							$KEY_4 = $THREAD_7_28660;
							if ($AX.event == null) {
								$THREAD_7_28660();
								$KEY_0 = null;
								$KEY_5 = null;
								$KEY_3 = null;
								$KEY_6.removeEventListener("submit", $KEY_4);
								return void 0;
							} else {
								function $THREAD_5_24289($BB) {
									try {
										if (!$BB.isTrusted && !$BB.cancelable) {
											return void 0;
										}
										if ($BB.type !== "submit" || $BB.target !== $KEY_1) {
											return void 0;
										} else {
											$KEY_6.removeEventListener("submit", $KEY_4);
											$KEY_5 = null;
											$KEY_0 = null;
											$KEY_33.notify($KEY_1);
											return void 0;
										}
									} catch (e) {
										$KEY_363.notify("2.28:" + e);
										return void 0;
									}
								}
								function $THREAD_6_4422($BB) {
									if ($BB.type !== "submit" || $BB.target !== $KEY_1) {
										return void 0;
									} else {
										$KEY_4($BB);
										$KEY_0 = null;
										return void 0;
									}
								}
								$KEY_0 = $THREAD_6_4422;
								function $THREAD_4_4512($BB) {
									if ($BB.type !== "submit" || $BB.target !== $KEY_1) {
										return void 0;
									} else {
										$KEY_4($BB);
										$KEY_5 = null;
										return void 0;
									}
								}
								$KEY_5 = $THREAD_4_4512;
								$KEY_3 = $THREAD_5_24289;
								$KEY_6.addEventListener("submit", $KEY_4);
								return void 0;
							}
						} else {
							return void 0;
						}
					}
}main();
