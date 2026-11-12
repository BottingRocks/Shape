function $THREAD_0_122622_2($AW, $AX) {
						let $KEY_2 = $AW < 0 || $AW === 0 && 1 / $AW < 0;
						let $AY = Math.abs($AW);
						let $AZ, $BA;
						if ($AY === 0) {
							$AZ = 0;
							$BA = 1023;
						} else if ($AY >= Math.pow(2, 1 - 1023)) {
							let $KEY_5 = Math.min(Math.floor(Math.log($AY) / Math.LN2), 1023);
							let $BB = $AY / Math.pow(2, $KEY_5);
							let $BC = $BB < 1;
							$BB = $BC ? $BB * 2 : $BB;
							$BC = $BC ? $KEY_5 - 1 : $KEY_5;
							if ($BB >= 2) {
								++$BC;
								$BB = $BB / 2;
							}
							let $KEY_9 = Math.pow(2, 52);
							$BB = $BB * $KEY_9;
							let $KEY_0 = Math.floor($BB);
							$KEY_0 = $BB - $KEY_0 < .5 ? $KEY_0 : $BB - $KEY_0 > .5 ? $KEY_0 + 1 : $KEY_0 % 2 ? $KEY_0 + 1 : $KEY_0;
							$BC += 1023;
							let $BD = ($KEY_0 - $KEY_9) / $KEY_9 >= 1;
							$BA = $BD ? $BC + 1 : $BC;
							if ($BA > 2046) {
								$AZ = 0;
								$BA = 2047;
							} else {
								$AZ = $BD ? 0 : $KEY_0 - $KEY_9;
							}
						} else {
							let $BE = $AY / Math.pow(2, -1074);
							let $BF = Math.floor($BE);
							$AZ = $BE - $BF < .5 ? $BF : $BE - $BF > .5 ? $BF + 1 : $BF % 2 ? $BF + 1 : $BF;
							$BA = 0;
						}
						let $BG = $KEY_88.DOUBLE64;
						let $BH, $BI, $BJ;
						if (($AZ & 536870911) === 0 && Math.abs($BA - 1023) < 256) {
							$BG = $KEY_88.FLOAT32;
							$AZ = $AZ / 536870911;
							$BA = $BA + 1023 - 127;
							if ($AY < Math.pow(2, -126)) {
								$AY = $AY / Math.pow(2, -126 - 23);
								let $KEY_7 = Math.floor($AY);
								$BH = $AY - $KEY_7 < .5 ? $KEY_7 : $AY - $KEY_7 > .5 ? $KEY_7 + 1 : $KEY_7 % 2 ? $KEY_7 + 1 : $KEY_7;
								$BA = 0;
							} else {
								$BH = $AZ;
							}
							$BI = 23;
							$BJ = 8;
						} else {
							$BH = $AZ;
							$BI = 52;
							$BJ = 11;
						}
						let $BK = [];
						while ($BI - 1 >= 0) {
							$BK.unshift($BH & 1);
							$BH = Math.floor($BH / 2);
							--$BI;
						}
						while ($BJ > 0) {
							$BK.unshift($BA & 1);
							--$BJ;
							$BA = Math.floor($BA / 2);
						}
						$BK.unshift($KEY_2 ? 1 : 0);
						$AX.push($BG);
						for (let i = 0; i < $BK.length; i = i + 8) {
							$AX.push($BK[i] << 7 | $BK[i + 1] << 6 | $BK[i + 2] << 5 | $BK[i + 3] << 4 | $BK[i + 4] << 3 | $BK[i + 5] << 2 | $BK[i + 6] << 1 | $BK[i + 7]);
						}
					}