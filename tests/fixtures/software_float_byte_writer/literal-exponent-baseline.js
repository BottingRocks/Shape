function $THREAD_5_99070($BW, $BX) {
							let $KEY_3 = $BW < 0 || $BW === 0 && 1 / $BW < 0;
							let $KEY_7 = Math.abs($BW);
							let $BY, $BZ;
							if ($KEY_7 === 0) {
								$BY = 0;
								$BZ = 1023;
							} else if ($KEY_7 >= Math.pow(2, -1022)) {
								let $KEY_6 = Math.min(Math.floor(Math.log($KEY_7) / Math.LN2), 1023);
								let $CA = $KEY_7 / Math.pow(2, $KEY_6);
								let $CB = $CA < 1;
								$CA = $CB ? $CA * 2 : $CA;
								$CB = $CB ? $KEY_6 - 1 : $KEY_6;
								if ($CA >= 2) {
									++$CB;
									$CA = $CA / 2;
								}
								let $KEY_5 = Math.pow(2, 52);
								$CA = $CA * $KEY_5;
								let $CC = Math.floor($CA);
								if (!($CA - $CC < .5)) if ($CA - $CC > .5) {
									++$CC;
								} else {
									$CC = [$CC, $CC % 2];
									$CC = $CC[1] ? $CC[0] + 1 : $CC[0];
								}
								$CC = $CC - $KEY_5;
								$CB += 1023;
								$KEY_5 = $CC / $KEY_5 >= 1;
								$BZ = $KEY_5 ? $CB + 1 : $CB;
								if ($BZ > 2046) {
									$BY = 0;
									$BZ = 2047;
								} else {
									$BY = $KEY_5 ? 0 : $CC;
								}
							} else {
								let $CD = $KEY_7 / Math.pow(2, -1074);
								$BY = Math.floor($CD);
								if (!($CD - $BY < .5)) if ($CD - $BY > .5) {
									++$BY;
								} else {
									$BY = [$BY, $BY % 2];
									$BY = $BY[1] ? $BY[0] + 1 : $BY[0];
								}
								$BZ = 0;
							}
							let $CE = $KEY_26.DOUBLE64;
							let $CF, $CG, $CH, $CI, $CJ;
							if (($BY & 536870911) === 0 && Math.abs($BZ - 1023) < 256) {
								$CG = $KEY_26.FLOAT32;
								$CH = $BY / 536870911;
								$CI = $BZ + 1023 - 127;
								if ($KEY_7 < Math.pow(2, -126)) {
									$KEY_7 = $KEY_7 / Math.pow(2, -149);
									let $CK = Math.floor($KEY_7);
									$CH = $KEY_7 - $CK < .5 ? $CK : $KEY_7 - $CK > .5 ? $CK + 1 : $CK % 2 ? $CK + 1 : $CK;
									$CI = 0;
								}
								$CF = 23;
								$CJ = 8;
							} else {
								$CF = 52;
								$CG = $CE;
								$CH = $BY;
								$CI = $BZ;
								$CJ = 11;
							}
							let $CL = [];
							while ($CF - 1 >= 0) {
								$CL.unshift($CH & 1);
								$CH = Math.floor($CH / 2);
								--$CF;
							}
							while ($CJ > 0) {
								$CL.unshift($CI & 1);
								--$CJ;
								$CI = Math.floor($CI / 2);
							}
							$CL.unshift($KEY_3 ? 1 : 0);
							$BX.push($CG);
							for (let i = 0; i < $CL.length; i = i + 8) {
								let $CM = $BX;
								$BX.push.call($CM, $CL[i] << 7 | $CL[i + 1] << 6 | $CL[i + 2] << 5 | $CL[i + 3] << 4 | $CL[i + 4] << 3 | $CL[i + 5] << 2 | $CL[i + 6] << 1 | $CL[i + 7]);
							}
						}