function main(){let $KEY_13,$KEY_2,$KEY_281,$KEY_264,$KEY_14,$KEY_16,$KEY_17,$KEY_4;
function $THREAD_8_105747() {
							if ($KEY_13 == null) {
								return void 0;
							}
							if ($KEY_2 == null) {
								return void 0;
							} else {
								let $CN = $KEY_2;
								let $CO = $KEY_281.concat($KEY_264);
								let i = 0;
								for (; i < $CO.length; i++) {
									delete $CN[$CO[i]];
								}
								let $CP, $CQ;
								if ($KEY_14 == null) {
									$CP = $KEY_14 == null;
									$CQ = void 0;
								} else {
									$CQ = $KEY_14.onBeforeInvoke;
									$CP = $CQ == null;
								}
								if (!$CP) {
									$CQ.unregister($KEY_17);
								}
								let $CR, $CS;
								if ($KEY_16 == null) {
									$CR = $KEY_16 == null;
									$CS = void 0;
								} else {
									$CS = $KEY_16.onBeforeInvoke;
									$CR = $CS == null;
								}
								if (!$CR) {
									$CS.unregister($KEY_4);
								}
								return void 0;
							}
						}
return $THREAD_8_105747;}main();
