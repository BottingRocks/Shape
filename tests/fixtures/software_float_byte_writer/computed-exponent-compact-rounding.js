function $THREAD_4_69065_3($GP, $GQ) {
						let $KEY_5 = $GP < 0 || $GP === 0 && 1 / $GP < 0;
						let $KEY_2 = Math.abs($GP);
						let $GR, $GS;
						if ($KEY_2 === 0) {
							$GR = 0;
							$GS = 1023;
						} else if ($KEY_2 >= Math.pow(2, 1 - 1023)) {
							let $KEY_9 = Math.min(Math.floor(Math.log($KEY_2) / Math.LN2), 1023);
							let $GT = $KEY_2 / Math.pow(2, $KEY_9);
							let $GU = $GT < 1;
							$KEY_9 = $GU ? $KEY_9 - 1 : $KEY_9;
							$GU = $GU ? $GT * 2 : $GT;
							let $GV = $GU >= 2;
							$GU = $GV ? $GU / 2 : $GU;
							let $KEY_4 = Math.pow(2, 52);
							let $KEY_3 = Math.floor($GU * $KEY_4);
							$GV = ($GV ? $KEY_9 + 1 : $KEY_9) + 1023;
							let $GW = (($GU * $KEY_4 - $KEY_3 < .5 ? $KEY_3 : $GU * $KEY_4 - $KEY_3 > .5 ? $KEY_3 + 1 : $KEY_3 % 2 ? $KEY_3 + 1 : $KEY_3) - $KEY_4) / $KEY_4 >= 1;
							$GS = $GW ? $GV + 1 : $GV;
							if ($GS > 2046) {
								$GR = 0;
								$GS = (1 << 11) - 1;
							} else {
								$GR = $GW ? 0 : ($GU * $KEY_4 - $KEY_3 < .5 ? $KEY_3 : $GU * $KEY_4 - $KEY_3 > .5 ? $KEY_3 + 1 : $KEY_3 % 2 ? $KEY_3 + 1 : $KEY_3) - $KEY_4;
							}
						} else {
							let $GX = $KEY_2 / Math.pow(2, 1 - 1023 - 52);
							$GR = Math.floor($GX);
							if (!($GX - $GR < .5)) if ($GX - $GR > .5) {
								++$GR;
							} else {
								$GR = [$GR, $GR % 2];
								$GR = $GR[1] ? $GR[0] + 1 : $GR[0];
							}
							$GS = 0;
						}
						let $GY = $KEY_125.DOUBLE64;
						let $GZ, $HA, $HB, $HC;
						if (($GR & 536870911) === 0 && Math.abs($GS - 1023) < 256) {
							$HA = $KEY_125.FLOAT32;
							$GZ = $GR / 536870911;
							if ($KEY_2 < Math.pow(2, -126)) {
								$KEY_2 = $KEY_2 / Math.pow(2, -126 - 23);
								$GZ = Math.floor($KEY_2);
								if (!($KEY_2 - $GZ < .5)) if ($KEY_2 - $GZ > .5) {
									++$GZ;
								} else {
									$GZ = [$GZ, $GZ % 2];
									$GZ = $GZ[1] ? $GZ[0] + 1 : $GZ[0];
								}
								$GS = 0;
							} else {
								$GS = $GS + 1023 - 127;
							}
							$HB = 23;
							$HC = 8;
						} else {
							$GZ = $GR;
							$HA = $GY;
							$HB = 52;
							$HC = 11;
						}
						let $HD = [];
						while ($HB - 1 >= 0) {
							$HD.unshift($GZ & 1);
							$GZ = Math.floor($GZ / 2);
							--$HB;
						}
						while ($HC > 0) {
							$HD.unshift($GS & 1);
							--$HC;
							$GS = Math.floor($GS / 2);
						}
						$HD.unshift($KEY_5 ? 1 : 0);
						$GQ.push($HA);
						for (let i = 0; i < $HD.length; i = i + 8) {
							$GQ.push($HD[i] << 7 | $HD[i + 1] << 6 | $HD[i + 2] << 5 | $HD[i + 3] << 4 | $HD[i + 4] << 3 | $HD[i + 5] << 2 | $HD[i + 6] << 1 | $HD[i + 7]);
						}
					}