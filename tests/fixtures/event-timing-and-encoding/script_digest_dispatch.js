function $THREAD_5_98650_2() {
									if ($KEY_28.global.crypto != null && typeof $KEY_28.global.crypto.subtle === "object" && typeof $KEY_28.global.crypto.subtle.digest === "function" && $KEY_28.scrBytes.sidx.length > 0) {
										let $KEY_0 = $KEY_280($KEY_12, $KEY_28.scrBytes);
										if ($KEY_0 != null) {
											$KEY_0.then($DG => {
												let $DH, $DI, $DJ, $DK;
												if ($DG?.hash == null) {
													$DI = $KEY_45.replace;
													$DH = $KEY_45;
													$DJ = RegExp("&c=.+");
													$DK = $KEY_32;
												} else {
													$DH = $KEY_45;
													$DI = $DH.replace;
													$DJ = RegExp("&c=.+");
													$DK = $DG?.hash;
												}
												$KEY_45 = $DI.call($DH, $DJ, "&c=" + $DK);
												const $DL = $DG?.sidx || [];
												$KEY_38($KEY_45, $DL);
											}).catch(() => {
											});
										} else {
											$KEY_38($KEY_45, []);
										}
									} else {
										$KEY_38($KEY_45, []);
									}
								}