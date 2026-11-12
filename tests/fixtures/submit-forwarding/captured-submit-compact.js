function main(){let $KEY_3,$KEY_4,$KEY_5,$KEY_110,$KEY_47,$KEY_1,$KEY_2,$KEY_0,$KEY_336;
function $THREAD_0_130838($AW) {
						let $KEY_3 = $AW.target;
						if ($KEY_3 == null) {
							return void 0;
						}
						if ($KEY_3 != null && $KEY_3.nodeType === 1 && $KEY_3.nodeName === "FORM") {
							let $AX;
							if ($AW.event != null && !$AW.event.isTrusted && "originalTarget" in $AW.event) {
								$AX = $KEY_3;
							} else {
								let $AY = $KEY_3;
								let $AZ;
								for (;;) {
									if ($AY.parentNode != null) {
										$AZ = $AY.parentNode;
										$AZ = $AZ == null || $AZ.nodeType !== 1 || $AZ.nodeName !== "FORM";
									} else {
										$AZ = $AY.parentNode != null;
									}
									if (!$AZ || $AY.parentNode === $KEY_3) break;
									$AY = $AY.parentNode;
								}
								$AX = $AY;
							}
							let $KEY_4 = $AX;
							let $KEY_5;
							function $THREAD_5_47205($BA) {
								try {
									$KEY_4.removeEventListener("submit", $KEY_5);
									if ($BA != null && $BA.defaultPrevented) {
										return void 0;
									} else {
										$KEY_110.notify({
											form: $KEY_3,
											event: $BA
										});
										return void 0;
									}
								} catch (e) {
									$KEY_47.notify("2.27:" + e);
									return void 0;
								}
							}
							$KEY_5 = $THREAD_5_47205;
							if ($AW.event == null) {
								$KEY_5();
								$KEY_1 = null;
								$KEY_2 = null;
								$KEY_0 = null;
								$KEY_4.removeEventListener("submit", $KEY_5);
								return void 0;
							} else {
								function $THREAD_5_61777($BA) {
									try {
										if (!$BA.isTrusted && !$BA.cancelable) {
											return void 0;
										}
										if ($BA.type !== "submit" || $BA.target !== $KEY_3) {
											return void 0;
										} else {
											$KEY_4.removeEventListener("submit", $KEY_5);
											$KEY_2 = null;
											$KEY_1 = null;
											$KEY_336.notify($KEY_3);
											return void 0;
										}
									} catch (e) {
										$KEY_47.notify("2.28:" + e);
										return void 0;
									}
								}
								function $THREAD_0_31141($BA) {
									if ($BA.type !== "submit" || $BA.target !== $KEY_3) {
										return void 0;
									} else {
										$KEY_5($BA);
										$KEY_1 = null;
										return void 0;
									}
								}
								$KEY_1 = $THREAD_0_31141;
								function $THREAD_1_76374($BA) {
									if ($BA.type !== "submit" || $BA.target !== $KEY_3) {
										return void 0;
									} else {
										$KEY_5($BA);
										$KEY_2 = null;
										return void 0;
									}
								}
								$KEY_2 = $THREAD_1_76374;
								$KEY_0 = $THREAD_5_61777;
								$KEY_4.addEventListener("submit", $KEY_5);
								return void 0;
							}
						} else {
							return void 0;
						}
					}
}main();
