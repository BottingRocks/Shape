function $THREAD_1_88373_2($CC, $CD) {
						let $KEY_5 = $CC < 0 || $CC === 0 && 1 / $CC < 0;
						let $CE = Math.abs($CC);
						let $CF, $KEY_9;
						if ($CE === 0) {
							$CF = 1023;
							$KEY_9 = 0;
						} else if ($CE >= Math.pow(2, 1 - 1023)) {
							let $KEY_6 = Math.min(Math.floor(Math.log($CE) / Math.LN2), 1023);
							let $CG = $CE / Math.pow(2, $KEY_6);
							let $CH = $CG < 1;
							$KEY_6 = $CH ? $KEY_6 - 1 : $KEY_6;
							$CH = $CH ? $CG * 2 : $CG;
							let $CI = $CH >= 2;
							let $KEY_8 = Math.pow(2, 52);
							let $CJ = Math.floor(($CI ? $CH / 2 : $CH) * $KEY_8);
							if (!(($CI ? $CH / 2 : $CH) * $KEY_8 - $CJ < .5)) if (($CI ? $CH / 2 : $CH) * $KEY_8 - $CJ > .5) {
								++$CJ;
							} else {
								$CJ = [$CJ, $CJ % 2];
								$CJ = $CJ[1] ? $CJ[0] + 1 : $CJ[0];
							}
							$CI = ($CI ? $KEY_6 + 1 : $KEY_6) + 1023;
							$KEY_9 = $CJ - $KEY_8;
							$KEY_8 = $KEY_9 / $KEY_8 >= 1;
							$CF = $KEY_8 ? $CI + 1 : $CI;
							if ($CF > 2046) {
								$CF = (1 << 11) - 1;
								$KEY_9 = 0;
							} else {
								$KEY_9 = $KEY_8 ? 0 : $KEY_9;
							}
						} else {
							let $CK = $CE / Math.pow(2, -1074);
							$KEY_9 = Math.floor($CK);
							if (!($CK - $KEY_9 < .5)) if ($CK - $KEY_9 > .5) {
								++$KEY_9;
							} else {
								$KEY_9 = [$KEY_9, $KEY_9 % 2];
								$KEY_9 = $KEY_9[1] ? $KEY_9[0] + 1 : $KEY_9[0];
							}
							$CF = 0;
						}
						let $CL = $KEY_200.DOUBLE64;
						let $CM, $CN, $CO, $CP, $CQ;
						if (($KEY_9 & 536870911) === 0 && Math.abs($CF - 1023) < 256) {
							$CO = $KEY_200.FLOAT32;
							$CF += 1023;
							$CP = $KEY_9 / 536870911;
							if ($CE < Math.pow(2, -126)) {
								$CE = $CE / Math.pow(2, -149);
								let $KEY_4 = Math.floor($CE);
								$CN = 0;
								$CP = $CE - $KEY_4 < .5 ? $KEY_4 : $CE - $KEY_4 > .5 ? $KEY_4 + 1 : $KEY_4 % 2 ? $KEY_4 + 1 : $KEY_4;
							} else {
								$CN = $CF - 127;
							}
							$CM = 23;
							$CQ = 8;
						} else {
							$CM = 52;
							$CN = $CF;
							$CO = $CL;
							$CP = $KEY_9;
							$CQ = 11;
						}
						let $CR = [];
						while ($CM - 1 >= 0) {
							$CR.unshift($CP & 1);
							$CP = Math.floor($CP / 2);
							--$CM;
						}
						while ($CQ > 0) {
							$CR.unshift($CN & 1);
							--$CQ;
							$CN = Math.floor($CN / 2);
						}
						$CR.unshift($KEY_5 ? 1 : 0);
						$CD.push($CO);
						for (let i = 0; i < $CR.length; i = i + 8) {
							let $CS = $CD;
							$CD.push.call($CS, $CR[i] << 7 | $CR[i + 1] << 6 | $CR[i + 2] << 5 | $CR[i + 3] << 4 | $CR[i + 4] << 3 | $CR[i + 5] << 2 | $CR[i + 6] << 1 | $CR[i + 7]);
						}
					}