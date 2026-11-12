function main(){let $KEY_1,$KEY_15,$KEY_96,$KEY_330,$KEY_8,$KEY_18,$KEY_12,$KEY_16;
function $THREAD_0_107547() {
							if ($KEY_1 == null) {
								return void 0;
							}
							if ($KEY_15 == null) {
								return void 0;
							} else {
								let $CI = $KEY_15;
								let $CJ = $KEY_96.concat($KEY_330);
								let i = 0;
								for (; i < $CJ.length; i++) {
									delete $CI[$CJ[i]];
								}
								let $CK, $CL;
								if ($KEY_8 == null) {
									$CK = $KEY_8 == null;
									$CL = void 0;
								} else {
									$CL = $KEY_8.onBeforeInvoke;
									$CK = $CL == null;
								}
								if (!$CK) {
									$CL.unregister($KEY_12);
								}
								let $CM, $CN;
								if ($KEY_18 == null) {
									$CM = $KEY_18 == null;
									$CN = void 0;
								} else {
									$CN = $KEY_18.onBeforeInvoke;
									$CM = $CN == null;
								}
								if (!$CM) {
									$CN.unregister($KEY_16);
								}
								return void 0;
							}
						}
return $THREAD_0_107547;}main();
