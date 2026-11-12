function $THREAD_9_62625($CT) {
							let $KEY_4 = $CT.getChannelData(0);
							let $CU = Math.random() * 4294967296;
							let $CV = [];
							let $CW = [($CU | 0) >>> 24, ($CU | 0) >> 16 & 255, ($CU | 0) >> 8 & 255, ($CU | 0) & 255];
							let $KEY_2 = $KEY_390(($CU | 0) ^ 2099515294);
							let $CX = $KEY_243($KEY_2, $KEY_8[0], $CW, 3);
							$KEY_4 = Array.from($KEY_4);
							let $CY = $KEY_4.length;
							$CX = $KEY_243($KEY_2, $CY, $CW, $CX);
							while ($CY - 1 >= 0) {
								$CX = $KEY_243($KEY_2, $KEY_4[$CY - 1], $CW, $CX);
								--$CY;
							}
							$KEY_15.next([$CV, $CW]);
							$KEY_15.complete();
						}
