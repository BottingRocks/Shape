function $THREAD_5_16368($DF, $DG) {
									let $KEY_5 = $DF < 0 || $DF === 0 && 1 / $DF < 0;
									let $DH = Math.abs($DF);
									let $DI, $DJ;
									if ($DH === 0) {
										$DI = 0;
										$DJ = 1023;
									} else if ($DH >= Math.pow(2, -1022)) {
										let $KEY_4 = Math.min(Math.floor(Math.log($DH) / Math.LN2), 1023);
										let $DK = $DH / Math.pow(2, $KEY_4);
										let $DL = $DK < 1;
										$DK = $DL ? $DK * 2 : $DK;
										$DL = $DL ? $KEY_4 - 1 : $KEY_4;
										if ($DK >= 2) {
											++$DL;
											$DK = $DK / 2;
										}
										let $KEY_6 = Math.pow(2, 52);
										let $KEY_9 = Math.floor($DK * $KEY_6);
										$DK = ($DK * $KEY_6 - $KEY_9 < .5 ? $KEY_9 : $DK * $KEY_6 - $KEY_9 > .5 ? $KEY_9 + 1 : $KEY_9 % 2 ? $KEY_9 + 1 : $KEY_9) - $KEY_6;
										$DL += 1023;
										$KEY_6 = $DK / $KEY_6 >= 1;
										$DJ = $KEY_6 ? $DL + 1 : $DL;
										if ($DJ > 2046) {
											$DI = 0;
											$DJ = 2047;
										} else {
											$DI = $KEY_6 ? 0 : $DK;
										}
									} else {
										let $DM = $DH / Math.pow(2, -1074);
										$DI = Math.floor($DM);
										if (!($DM - $DI < .5)) {
											if ($DM - $DI > .5) {
												++$DI;
											} else {
												$DI = [$DI, $DI % 2];
												$DI = $DI[1] ? $DI[0] + 1 : $DI[0];
											}
										}
										$DJ = 0;
									}
									let $DN = $KEY_357.DOUBLE64;
									let $DO, $DP;
									if (($DI & 536870911) === 0 && Math.abs($DJ - 1023) < 256) {
										$DN = $KEY_357.FLOAT32;
										$DI = $DI / 536870911;
										$DJ = $DJ + 1023 - 127;
										if ($DH < Math.pow(2, -126)) {
											$DH = $DH / Math.pow(2, 1 - 127 - 23);
											$DI = Math.floor($DH);
											if (!($DH - $DI < .5)) {
												if ($DH - $DI > .5) {
													++$DI;
												} else {
													$DI = [$DI, $DI % 2];
													$DI = $DI[1] ? $DI[0] + 1 : $DI[0];
												}
											}
											$DJ = 0;
										}
										$DO = 8;
										$DP = 23;
									} else {
										$DO = 11;
										$DP = 52;
									}
									let $DQ = [];
									let i = $DP - 1;
									for (; i >= 0; i--) {
										$DQ.unshift($DI & 1);
										$DI = Math.floor($DI / 2);
									}
									while ($DO > 0) {
										$DQ.unshift($DJ & 1);
										--$DO;
										$DJ = Math.floor($DJ / 2);
									}
									$DQ.unshift($KEY_5 ? 1 : 0);
									$DG.push($DN);
									let $DR = 0;
									while ($DR < $DQ.length) {
										$DG.push($DQ[$DR] << 7 | $DQ[$DR + 1] << 6 | $DQ[$DR + 2] << 5 | $DQ[$DR + 3] << 4 | $DQ[$DR + 4] << 3 | $DQ[$DR + 5] << 2 | $DQ[$DR + 6] << 1 | $DQ[$DR + 7]);
										$DR += 8;
									}
								}