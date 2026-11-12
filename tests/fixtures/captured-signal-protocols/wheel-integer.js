function $THREAD_7_71183() {
					try {
						let $BL = $KEY_428();
						if ($BL - $KEY_13 > 500) {
							++$KEY_8;
							$KEY_13 = $BL;
						}
						let $BM = Math.random() * 4294967296 | 0;
						let $BN = [$BM >>> 24, $BM >> 16 & 255, $BM >> 8 & 255, $BM & 255];
						$BM = $KEY_254($BM ^ 3245311812);
						let $BO = $KEY_110($BM, $KEY_8, $BN, 3);
						let $BP = $KEY_110;
						++$KEY_6;
						$BP($BM, $KEY_6, $BN, $BO);
						$KEY_23.next([[], $BN]);
					} catch (e) {
						$KEY_23.error(e);
						if ($KEY_0 != null) {
							$KEY_17.removeEventListener("wheel", $KEY_0, false);
						}
					}
				}