function main(){let $KEY_6,$KEY_3,$KEY_390,$KEY_78,$KEY_5,$KEY_7,$KEY_10,$KEY_15;
function $THREAD_9_102987() {
							if ($KEY_6 == null) {
								return void 0;
							}
							if ($KEY_3 == null) {
								return void 0;
							} else {
								let $BN = $KEY_3;
								let $BO = $KEY_390.concat($KEY_78);
								let i = 0;
								for (; i < $BO.length; i++) {
									delete $BN[$BO[i]];
								}
								let $BP, $BQ;
								if ($KEY_5 == null) {
									$BP = $KEY_5 == null;
									$BQ = void 0;
								} else {
									$BQ = $KEY_5.onBeforeInvoke;
									$BP = $BQ == null;
								}
								if (!$BP) {
									$BQ.unregister($KEY_10);
								}
								let $BR, $BS;
								if ($KEY_7 == null) {
									$BR = $KEY_7 == null;
									$BS = void 0;
								} else {
									$BS = $KEY_7.onBeforeInvoke;
									$BR = $BS == null;
								}
								if (!$BR) {
									$BS.unregister($KEY_15);
								}
								return void 0;
							}
						}
return $THREAD_9_102987;}main();
