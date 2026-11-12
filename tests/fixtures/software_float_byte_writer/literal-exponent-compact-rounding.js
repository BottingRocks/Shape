function $THREAD_1_78994_2($CB, $CC) {
							let $KEY_1 = $CB < 0 || $CB === 0 && 1 / $CB < 0;
							let $CD = Math.abs($CB);
							let $CE, $CF;
							if ($CD === 0) {
								$CE = 0;
								$CF = 1023;
							} else if ($CD >= Math.pow(2, -1022)) {
								$CF = Math.min(Math.floor(Math.log($CD) / Math.LN2), 1023);
								let $CG = $CD / Math.pow(2, $CF);
								let $CH = $CG < 1;
								$CF = $CH ? $CF - 1 : $CF;
								$CH = $CH ? $CG * 2 : $CG;
								let $CI = $CH >= 2;
								$CF = $CI ? $CF + 1 : $CF;
								let $KEY_9 = Math.pow(2, 52);
								let $KEY_8 = Math.floor(($CI ? $CH / 2 : $CH) * $KEY_9);
								let $CJ = ((($CI ? $CH / 2 : $CH) * $KEY_9 - $KEY_8 < .5 ? $KEY_8 : ($CI ? $CH / 2 : $CH) * $KEY_9 - $KEY_8 > .5 ? $KEY_8 + 1 : $KEY_8 % 2 ? $KEY_8 + 1 : $KEY_8) - $KEY_9) / $KEY_9 >= 1;
								$CF = $CJ ? $CF + 1024 : $CF + 1023;
								if ($CF > 2046) {
									$CE = 0;
									$CF = 2047;
								} else {
									$CE = $CJ ? 0 : (($CI ? $CH / 2 : $CH) * $KEY_9 - $KEY_8 < .5 ? $KEY_8 : ($CI ? $CH / 2 : $CH) * $KEY_9 - $KEY_8 > .5 ? $KEY_8 + 1 : $KEY_8 % 2 ? $KEY_8 + 1 : $KEY_8) - $KEY_9;
								}
							} else {
								let $CK = $CD / Math.pow(2, -1074);
								let $KEY_11 = Math.floor($CK);
								$CE = $CK - $KEY_11 < .5 ? $KEY_11 : $CK - $KEY_11 > .5 ? $KEY_11 + 1 : $KEY_11 % 2 ? $KEY_11 + 1 : $KEY_11;
								$CF = 0;
							}
							let $CL = $KEY_311.DOUBLE64;
							let $CM, $CN, $CO, $CP;
							if (($CE & 536870911) === 0 && Math.abs($CF - 1023) < 256) {
								$CL = $KEY_311.FLOAT32;
								$CE = $CE / 536870911;
								$CF = $CF + 1023 - 127;
								if ($CD < Math.pow(2, 1 - 127)) {
									$CD = $CD / Math.pow(2, 1 - 127 - 23);
									let $KEY_6 = Math.floor($CD);
									$CM = $CD - $KEY_6 < .5 ? $KEY_6 : $CD - $KEY_6 > .5 ? $KEY_6 + 1 : $KEY_6 % 2 ? $KEY_6 + 1 : $KEY_6;
									$CO = 0;
								} else {
									$CM = $CE;
									$CO = $CF;
								}
								$CN = 8;
								$CP = 23;
							} else {
								$CM = $CE;
								$CN = 11;
								$CO = $CF;
								$CP = 52;
							}
							let $CQ = [];
							let i = $CP - 1;
							for (; i >= 0; i--) {
								$CQ.unshift($CM & 1);
								$CM = Math.floor($CM / 2);
							}
							while ($CN > 0) {
								$CQ.unshift($CO & 1);
								--$CN;
								$CO = Math.floor($CO / 2);
							}
							$CQ.unshift($KEY_1 ? 1 : 0);
							$CC.push($CL);
							let $CR = 0;
							while ($CR < $CQ.length) {
								$CC.push($CQ[$CR] << 7 | $CQ[$CR + 1] << 6 | $CQ[$CR + 2] << 5 | $CQ[$CR + 3] << 4 | $CQ[$CR + 4] << 3 | $CQ[$CR + 5] << 2 | $CQ[$CR + 6] << 1 | $CQ[$CR + 7]);
								$CR += 8;
							}
						}