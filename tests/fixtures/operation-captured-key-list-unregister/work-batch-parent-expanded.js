function main(){let $KEY_20,$KEY_6,$KEY_12,$KEY_61,$KEY_14,$KEY_3,$KEY_4,$KEY_15;
function $THREAD_6_43037() {
										if ($KEY_20 == null) {
											return void 0;
										}
										if ($KEY_6 == null) {
											return void 0;
										} else {
											let $HE = $KEY_6;
											let $HF = $KEY_12.concat($KEY_61);
											let i = 0;
											for (; i < $HF.length; i++) {
												delete $HE[$HF[i]];
											}
											let $HG, $HH;
											if ($KEY_14 == null) {
												$HG = $KEY_14 == null;
												$HH = void 0;
											} else {
												$HH = $KEY_14.onBeforeInvoke;
												$HG = $HH == null;
											}
											if (!$HG) {
												$HH.unregister($KEY_4);
											}
											let $HI, $HJ;
											if ($KEY_3 == null) {
												$HI = $KEY_3 == null;
												$HJ = void 0;
											} else {
												$HJ = $KEY_3.onBeforeInvoke;
												$HI = $HJ == null;
											}
											if (!$HI) {
												$HJ.unregister($KEY_15);
											}
											return void 0;
										}
									}
return $THREAD_6_43037;}main();
