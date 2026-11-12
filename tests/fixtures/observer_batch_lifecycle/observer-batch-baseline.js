let $KEY_176=opaqueConstructor, $KEY_171=opaqueResetValue;
function $THREAD_6_11724($AI, $AJ, $AK, $AL, $AM, $AN, $AO) {
					let $KEY_0;
					function $THREAD_9_114209($AQ) {
						try {
							$AQ();
						} catch (e) {
							let $KEY_2 = e;
							$KEY_0.forEach($AR => $AR.error($KEY_2));
						}
					}
					let $KEY_19 = $THREAD_9_114209;
					$KEY_0 = [];
					let i = 0;
					for (; i < $AI.length; i++) {
						function $THREAD_6_118253($AQ) {
							$KEY_0.push($AQ);
						}
						new $KEY_176($THREAD_6_118253).subscribe($AI[i]);
					}
					let $KEY_1;
					function $THREAD_2_113416($AQ) {
						$KEY_1 = $AQ;
					}
					let $KEY_4, $KEY_8;
					function $THREAD_6_78478($AQ, $AR) {
						$KEY_8 = $AQ;
						$KEY_4 = $AR;
					}
					$KEY_4 = true;
					let $KEY_12;
					function $THREAD_5_65859($AQ) {
						$KEY_12 = $AQ;
					}
					let $KEY_10;
					function $THREAD_1_28962($AQ) {
						$KEY_10 = $AQ;
					}
					try {
						let $KEY_21;
						let $AP = $KEY_0;
						if ($AP) {
							$AJ($AP, $AK, $THREAD_5_65859, $THREAD_6_78478, $THREAD_1_28962, $THREAD_2_113416);
							$KEY_21 = $KEY_8;
							if ($KEY_12 != null) {
								$KEY_19($KEY_12);
							}
							if ($KEY_21 != null) {
								if ($AN) {
									if ($KEY_4) {
										$AM(() => {
											$KEY_19($KEY_21);
										});
									} else {
										$KEY_19($KEY_21);
									}
								} else {
									$AL(() => {
										$KEY_19($KEY_21);
									});
								}
							}
							if ($KEY_10 != null) {
								$AO(() => {
									$KEY_19($KEY_10);
								});
							}
							function $THREAD_1_104567() {
								$KEY_12 = $KEY_171;
								$KEY_10 = $KEY_171;
								if ($KEY_1 != null) {
									$KEY_1();
								}
							}
							return $THREAD_1_104567;
						} else {
							$AP(() => {
								throw new TypeError("Cannot read properties of undefined (reading 'value')");
							}, $AK, $THREAD_5_65859, $THREAD_6_78478, $THREAD_1_28962, $THREAD_2_113416);
							$KEY_21 = $KEY_8;
							if ($KEY_12 != null) {
								$KEY_19($KEY_12);
							}
							if ($KEY_21 != null) {
								if ($AN) {
									if ($KEY_4) {
										$AM(() => {
											$KEY_19($KEY_21);
										});
									} else {
										$KEY_19($KEY_21);
									}
								} else {
									$AL(() => {
										$KEY_19($KEY_21);
									});
								}
							}
							if ($KEY_10 != null) {
								$AO(() => {
									$KEY_19($KEY_10);
								});
							}
							function $THREAD_1_104567_2() {
								$KEY_12 = $KEY_171;
								$KEY_10 = $KEY_171;
								if ($KEY_1 != null) {
									$KEY_1();
								}
							}
							return $THREAD_1_104567_2;
						}
					} catch (e) {
						let $KEY_6 = e;
						$KEY_0.forEach($AQ => $AQ.error($KEY_6));
						function $THREAD_4_45749() {
						}
						return $THREAD_4_45749;
					}
				}