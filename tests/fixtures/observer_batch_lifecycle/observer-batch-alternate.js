let $KEY_72=opaqueConstructor, $KEY_88=opaqueResetValue;
function $THREAD_5_97537($AE, $AF, $AG, $AH, $AI, $AJ, $AK) {
					let $KEY_1, $KEY_12;
					function $THREAD_7_59581($AM) {
						try {
							$AM();
						} catch (e) {
							let $KEY_0 = e;
							$KEY_12.forEach($AN => $AN.error($KEY_0));
						}
					}
					let $KEY_7 = $THREAD_7_59581;
					$KEY_12 = [];
					let i = 0;
					for (; i < $AE.length; i++) {
						function $THREAD_1_93084($AM) {
							$KEY_12.push($AM);
						}
						new $KEY_72($THREAD_1_93084).subscribe($AE[i]);
					}
					let $KEY_10 = true;
					let $KEY_6;
					function $THREAD_2_2538($AM) {
						$KEY_6 = $AM;
					}
					let $KEY_21;
					function $THREAD_0_95659($AM, $AN) {
						$KEY_21 = $AM;
						$KEY_10 = $AN;
					}
					let $KEY_18;
					function $THREAD_7_14623($AM) {
						$KEY_18 = $AM;
					}
					let $KEY_15;
					function $THREAD_2_58611($AM) {
						$KEY_15 = $AM;
					}
					try {
						let $KEY_14;
						let $AL = $KEY_12;
						if ($AL) {
							$AF($AL, $AG, $THREAD_2_2538, $THREAD_0_95659, $THREAD_7_14623, $THREAD_2_58611);
							$KEY_14 = $KEY_21;
							if ($KEY_6 != null) {
								$KEY_7($KEY_6);
							}
							if ($KEY_14 != null) {
								if ($AJ) {
									if ($KEY_10) {
										$AI(() => {
											$KEY_7($KEY_14);
										});
									} else {
										$KEY_7($KEY_14);
									}
								} else {
									$AH(() => {
										$KEY_7($KEY_14);
									});
								}
							}
							if ($KEY_18 != null) {
								$AK(() => {
									$KEY_7($KEY_18);
								});
							}
							function $THREAD_1_132949() {
								$KEY_6 = $KEY_88;
								$KEY_18 = $KEY_88;
								if ($KEY_15 != null) {
									$KEY_15();
								}
							}
							return $THREAD_1_132949;
						} else {
							$AL(() => {
								throw new TypeError("Cannot read properties of undefined (reading 'value')");
							}, $AG, $THREAD_2_2538, $THREAD_0_95659, $THREAD_7_14623, $THREAD_2_58611);
							$KEY_14 = $KEY_21;
							if ($KEY_6 != null) {
								$KEY_7($KEY_6);
							}
							if ($KEY_14 != null) {
								if ($AJ) {
									if ($KEY_10) {
										$AI(() => {
											$KEY_7($KEY_14);
										});
									} else {
										$KEY_7($KEY_14);
									}
								} else {
									$AH(() => {
										$KEY_7($KEY_14);
									});
								}
							}
							if ($KEY_18 != null) {
								$AK(() => {
									$KEY_7($KEY_18);
								});
							}
							function $THREAD_1_132949_2() {
								$KEY_6 = $KEY_88;
								$KEY_18 = $KEY_88;
								if ($KEY_15 != null) {
									$KEY_15();
								}
							}
							return $THREAD_1_132949_2;
						}
					} catch (e) {
						$KEY_1 = e;
						$KEY_12.forEach($AM => $AM.error($KEY_1));
						function $THREAD_6_2719() {
						}
						return $THREAD_6_2719;
					}
				}