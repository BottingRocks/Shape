function $THREAD_2_79370() {
									if ($KEY_6.global.crypto != null && typeof $KEY_6.global.crypto.subtle === "object" && typeof $KEY_6.global.crypto.subtle.digest === "function" && $KEY_6.scrBytes.sidx.length > 0) {
										let $BH = $KEY_200($KEY_3, $KEY_6.scrBytes);
										if ($BH != null) {
											$BH.then($BI => {
												let $BJ, $BK, $BL, $BM;
												if ($BI?.hash == null) {
													$BK = $KEY_15.replace;
													$BJ = $KEY_15;
													$BL = RegExp("&c=.+");
													$BM = $KEY_2;
												} else {
													$BJ = $KEY_15;
													$BK = $BJ.replace;
													$BL = RegExp("&c=.+");
													$BM = $BI?.hash;
												}
												$KEY_15 = $BK.call($BJ, $BL, "&c=" + $BM);
												$KEY_14($KEY_15, $BI?.sidx || []);
											}).catch(() => {
											});
										} else {
											$KEY_14($KEY_15, []);
										}
									} else {
										$KEY_14($KEY_15, []);
									}
								}