function $THREAD_0_19371($DI) {
							let $KEY_5 = $DI.getChannelData(0);
							let $DJ = Math.random() * 4294967296;
							let $DK = [];
							let $DL = [($DJ | 0) >>> 24, ($DJ | 0) >> 16 & 255, ($DJ | 0) >> 8 & 255, ($DJ | 0) & 255];
							$DJ = $KEY_132(($DJ | 0) ^ 1694101466);
							let $DM = $KEY_337($DJ, $KEY_30[0], $DL, 3);
							$KEY_5 = Array.from($KEY_5);
							let $DN = $KEY_5.length;
							$DM = $KEY_337($DJ, $DN, $DL, $DM);
							while ($DN - 1 >= 0) {
								$DM = $KEY_337($DJ, $KEY_5[$DN - 1], $DL, $DM);
								--$DN;
							}
							$KEY_27.next([$DK, $DL]);
							$KEY_27.complete();
						}
