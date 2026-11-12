function main() {
let $KEY_2 = null, $KEY_300 = null, $KEY_120 = false;
function $THREAD_6_57781() {
						let $AZ = $KEY_2.persistentId.key;
						try {
							if ($KEY_300 != null) {
								if ($KEY_300.getItem($AZ) == null) {
									let $BA = $KEY_300.getItem("f");
									if ($BA != null && typeof $BA === "string") {
										let $BB = {};
										let i = 0;
										for (; i < "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=".length; i++) {
											$BB["ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_="[i]] = i;
										}
										let $BC = [];
										let $BD = 0;
										while ($BD < $BA.length) {
											if ($BB[$BA[$BD + 2]] === 64 && $BB[$BA[$BD + 3]] !== 64) {
												throw new Error("The string to be decoded is not correctly padded.");
											} else {
												$BC.push($BB[$BA[$BD]] << 2 | $BB[$BA[$BD + 1]] >> 4);
												if ($BB[$BA[$BD + 2]] !== 64 && $BD + 2 < $BA.length) {
													$BC.push(($BB[$BA[$BD + 1]] & 15) << 4 | $BB[$BA[$BD + 2]] >> 2);
												}
												if ($BB[$BA[$BD + 3]] !== 64 && $BD + 3 < $BA.length) {
													$BC.push(($BB[$BA[$BD + 2]] & 3) << 6 | $BB[$BA[$BD + 3]]);
												}
												$BD += 4;
												continue;
											}
										}
										if ($BC[5] >= 90 && $BC[5] <= 190 && $BC[6] === 1 && $BC[7] === 0 && $BC[8] === 0) {
											$KEY_300.removeItem("f");
											try {
												$KEY_120 = false;
												if ($KEY_300) {
													if (!$KEY_300.getItem($AZ)) {
														$KEY_300.setItem($AZ, $BA);
													}
													$KEY_120 = true;
												}
											} catch (e) {}
										}
									}
								}
							}
						} catch (e_2) {}
						let $BE = $KEY_2.persistentId.key;
						let $BF = $KEY_2.transactionToken;
						try {
							$KEY_120 = false;
							if ($KEY_300) {
								if (typeof $KEY_2.persistentId.resetKey !== "undefined") {
									$KEY_300.removeItem($KEY_2.persistentId.resetKey);
								}
								if (!$KEY_300.getItem($BE)) {
									$KEY_300.setItem($BE, $BF);
								}
								$KEY_120 = true;
							}
						} catch (e_3) {}
						try {
							if ($KEY_300) {
								$KEY_287 = "l_" + $KEY_2.persistentId.key;
								$KEY_202 = "" + Date.now() + $KEY_127();
								if (!$KEY_300.getItem($KEY_287)) {
									$KEY_300.setItem($KEY_287, $KEY_202);
								}
							}
						} catch (e_4) {}
						$KEY_61 = $KEY_26($KEY_2.transactionToken);
						$KEY_33();
						try {
							$KEY_7.forEach($BH => {
								$BH.init(new $KEY_247($BI => {
									let $KEY_1 = $BI;
									$KEY_161.push($BJ => $KEY_1.next($BJ));
								}));
								if ($BH.afterReady != null) {
									if (typeof setTimeout !== "function") {
										$BH.afterReady.call(void 0);
									} else if ($KEY_122) {
										setTimeout($BH.afterReady, 0);
									} else if ($KEY_220) {
										$KEY_220.push($BH.afterReady);
									}
								}
							});
						} catch (e_5) {
							let $BG = {};
							$BG.message = "3.1:" + e_5;
							$KEY_298($BG);
						}
					}
$THREAD_6_57781();
}
