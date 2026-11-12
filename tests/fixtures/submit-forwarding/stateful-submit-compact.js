function main(){let $KEY_8,$KEY_11,$KEY_12,$KEY_229,$KEY_3,$KEY_139,$KEY_0,$KEY_6,$KEY_1,$KEY_2;
function $THREAD_8_14726($DH) {
						let $KEY_8 = $DH.target;
						if ($KEY_8 == null) {
							return void 0;
						}
						if ($KEY_8 != null && $KEY_8.nodeType === 1 && $KEY_8.nodeName === "FORM") {
							let $DI, $DJ;
							if ($DH.event != null && !$DH.event.isTrusted) {
								$DJ = $DH.event;
								$DI = "originalTarget" in $DJ;
							} else {
								$DI = false;
								$DJ = $KEY_8;
							}
							let $KEY_11;
							if ($DI) {
								$KEY_11 = $KEY_8;
							} else {
								let $DK = $KEY_8;
								for (;;) {
									let $DL = $DK.parentNode;
									if ($DL == null || $DL != null && $DL.nodeType === 1 && $DL.nodeName === "FORM" || $DL === $KEY_8) break;
									$DK = $DL;
									$DJ = $DL ?? $DJ;
								}
								$KEY_11 = $DK;
							}
							let $KEY_12;
							function $THREAD_0_106290($DM) {
								try {
									$KEY_11.removeEventListener("submit", $KEY_12);
									if ($DM != null && $DM.defaultPrevented) {
										return void 0;
									} else {
										$KEY_229.notify({
											form: $KEY_8,
											event: $DM
										});
										$KEY_3 = true;
										return void 0;
									}
								} catch (e) {
									$KEY_139.notify("2.27:" + e);
									return void 0;
								}
							}
							$KEY_12 = $THREAD_0_106290;
							if ($DH.event == null) {
								$KEY_12();
								$KEY_0 = null;
								$KEY_6 = null;
								$KEY_1 = null;
								$KEY_11.removeEventListener("submit", $KEY_12);
								return void 0;
							} else {
								function $THREAD_8_108359($DM) {
									if ($DM.type !== "submit" || $DM.target !== $KEY_8) {
										return void 0;
									} else {
										$KEY_12($DM);
										$KEY_0 = null;
										return void 0;
									}
								}
								$KEY_0 = $THREAD_8_108359;
								function $THREAD_0_65125($DM) {
									if ($DM.type !== "submit" || $DM.target !== $KEY_8) {
										return void 0;
									} else {
										$KEY_12($DM);
										$KEY_6 = null;
										return void 0;
									}
								}
								$KEY_6 = $THREAD_0_65125;
								function $THREAD_2_153105($DM) {
									try {
										if (!$DM.isTrusted && !$DM.cancelable) {
											return void 0;
										}
										if ($DM.type !== "submit" || $DM.target !== $KEY_8) {
											return void 0;
										} else {
											$KEY_11.removeEventListener("submit", $KEY_12);
											$KEY_6 = null;
											$KEY_0 = null;
											$KEY_2.notify($KEY_8);
											$KEY_3 = false;
											return void 0;
										}
									} catch (e) {
										$KEY_139.notify("2.28:" + e);
										return void 0;
									}
								}
								$KEY_1 = $THREAD_2_153105;
								$KEY_11.addEventListener("submit", $KEY_12);
								return void 0;
							}
						} else {
							return void 0;
						}
					}
}main();
