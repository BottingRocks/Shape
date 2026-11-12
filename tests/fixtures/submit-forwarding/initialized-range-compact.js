function main(){let $KEY_8,$KEY_7,$KEY_6,$KEY_350,$KEY_360,$KEY_4,$KEY_2,$KEY_3,$KEY_311;
function $THREAD_1_119230($V) {
						let $KEY_8 = $V.target;
						if ($KEY_8 == null) {
							return void 0;
						}
						if ($KEY_8 != null && $KEY_8.nodeType === 1 && $KEY_8.nodeName === "FORM") {
							let $KEY_7;
							if ($V.event != null && !$V.event.isTrusted && "originalTarget" in $V.event) {
								$KEY_7 = $KEY_8;
							} else {
								let $W = $KEY_8;
								for (;;) {
									let $X;
									if ($W.parentNode != null) {
										$X = $W.parentNode == null || $W.parentNode.nodeType !== 1 || $W.parentNode.nodeName !== "FORM";
									} else {
										$X = false;
									}
									if (!$X || $W.parentNode === $KEY_8) break;
									$W = $W.parentNode;
								}
								$KEY_7 = $W;
							}
							let $KEY_6;
							function $THREAD_2_131206($Y) {
								try {
									$KEY_7.removeEventListener("submit", $KEY_6);
									if ($Y != null && $Y.defaultPrevented) {
										return void 0;
									} else {
										$KEY_350.notify({
											form: $KEY_8,
											event: $Y
										});
										return void 0;
									}
								} catch (e) {
									$KEY_360.notify("2.27:" + e);
									return void 0;
								}
							}
							$KEY_6 = $THREAD_2_131206;
							if ($V.event == null) {
								$THREAD_2_131206();
								$KEY_4 = null;
								$KEY_2 = null;
								$KEY_3 = null;
								$KEY_7.removeEventListener("submit", $KEY_6);
								return void 0;
							} else {
								function $THREAD_4_70572($Y) {
									try {
										if (!$Y.isTrusted && !$Y.cancelable) {
											return void 0;
										}
										if ($Y.type !== "submit" || $Y.target !== $KEY_8) {
											return void 0;
										} else {
											$KEY_7.removeEventListener("submit", $KEY_6);
											$KEY_2 = null;
											$KEY_4 = null;
											$KEY_311.notify($KEY_8);
											return void 0;
										}
									} catch (e) {
										$KEY_360.notify("2.28:" + e);
										return void 0;
									}
								}
								function $THREAD_8_80862($Y) {
									if ($Y.type !== "submit" || $Y.target !== $KEY_8) {
										return void 0;
									} else {
										$KEY_6($Y);
										$KEY_4 = null;
										return void 0;
									}
								}
								$KEY_4 = $THREAD_8_80862;
								function $THREAD_5_23082($Y) {
									if ($Y.type !== "submit" || $Y.target !== $KEY_8) {
										return void 0;
									} else {
										$KEY_6($Y);
										$KEY_2 = null;
										return void 0;
									}
								}
								$KEY_2 = $THREAD_5_23082;
								$KEY_3 = $THREAD_4_70572;
								$KEY_7.addEventListener("submit", $KEY_6);
								return void 0;
							}
						} else {
							return void 0;
						}
					}
}main();
