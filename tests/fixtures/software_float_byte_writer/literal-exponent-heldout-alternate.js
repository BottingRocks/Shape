function $THREAD_7_114922_2($AX, $AY) {
						let $KEY_9 = $AX < 0 || $AX === 0 && 1 / $AX < 0;
						let $KEY_5 = Math.abs($AX);
						let $AZ, $BA;
						if ($KEY_5 === 0) {
							$AZ = 0;
							$BA = 1023;
						} else if ($KEY_5 >= Math.pow(2, -1022)) {
							let $KEY_6 = Math.min(Math.floor(Math.log($KEY_5) / Math.LN2), 1023);
							let $BB = $KEY_5 / Math.pow(2, $KEY_6);
							let $BC = $BB < 1;
							$BB = $BC ? $BB * 2 : $BB;
							$BC = $BC ? $KEY_6 - 1 : $KEY_6;
							if ($BB >= 2) {
								++$BC;
								$BB = $BB / 2;
							}
							let $KEY_0 = Math.pow(2, 52);
							$BB = $BB * $KEY_0;
							let $BD = Math.floor($BB);
							if (!($BB - $BD < .5)) if ($BB - $BD > .5) {
								++$BD;
							} else {
								$BD = [$BD, $BD % 2];
								$BD = $BD[1] ? $BD[0] + 1 : $BD[0];
							}
							$BC += 1023;
							let $KEY_3 = $BD - $KEY_0;
							$KEY_0 = $KEY_3 / $KEY_0 >= 1;
							$BA = $KEY_0 ? $BC + 1 : $BC;
							if ($BA > 2046) {
								$AZ = 0;
								$BA = 2047;
							} else {
								$AZ = $KEY_0 ? 0 : $KEY_3;
							}
						} else {
							let $BE = $KEY_5 / Math.pow(2, -1022 - 52);
							$AZ = Math.floor($BE);
							if (!($BE - $AZ < .5)) if ($BE - $AZ > .5) {
								++$AZ;
							} else {
								$AZ = [$AZ, $AZ % 2];
								$AZ = $AZ[1] ? $AZ[0] + 1 : $AZ[0];
							}
							$BA = 0;
						}
						let $BF = $KEY_151.DOUBLE64;
						let $BG, $BH, $BI, $BJ, $BK;
						if (($AZ & 536870911) === 0 && Math.abs($BA - 1023) < 256) {
							$BI = $KEY_151.FLOAT32;
							$BH = $AZ / 536870911;
							if ($KEY_5 < Math.pow(2, -126)) {
								$KEY_5 = $KEY_5 / Math.pow(2, 1 - 127 - 23);
								let $BL = Math.floor($KEY_5);
								$BH = $KEY_5 - $BL < .5 ? $BL : $KEY_5 - $BL > .5 ? $BL + 1 : $BL % 2 ? $BL + 1 : $BL;
								$BJ = 0;
							} else {
								$BJ = $BA + 1023 - 127;
							}
							$BG = 23;
							$BK = 8;
						} else {
							$BG = 52;
							$BH = $AZ;
							$BI = $BF;
							$BJ = $BA;
							$BK = 11;
						}
						let $BM = [];
						while ($BG - 1 >= 0) {
							$BM.unshift($BH & 1);
							$BH = Math.floor($BH / 2);
							--$BG;
						}
						while ($BK > 0) {
							$BM.unshift($BJ & 1);
							--$BK;
							$BJ = Math.floor($BJ / 2);
						}
						$BM.unshift($KEY_9 ? 1 : 0);
						$AY.push($BI);
						for (let i = 0; i < $BM.length; i = i + 8) {
							$AY.push($BM[i] << 7 | $BM[i + 1] << 6 | $BM[i + 2] << 5 | $BM[i + 3] << 4 | $BM[i + 4] << 3 | $BM[i + 5] << 2 | $BM[i + 6] << 1 | $BM[i + 7]);
						}
					}