function $THREAD_5_53665_2() {
					try {
						let $AZ = $KEY_245();
						if ($AZ - $KEY_16 > 500) {
							++$KEY_22;
							$KEY_16 = $AZ;
						}
						let $BA = Math.random() * 4294967296;
						let $BB = [($BA | 0) >>> 24, ($BA | 0) >> 16 & 255, ($BA | 0) >> 8 & 255, ($BA | 0) & 255];
						$BA = $KEY_120(($BA | 0) ^ 4256838423);
						let $BC = $KEY_191($BA, $KEY_22, $BB, 3);
						let $BD = $KEY_191;
						++$KEY_1;
						$BD($BA, $KEY_1, $BB, $BC);
						$KEY_5.next([[], $BB]);
					} catch (e) {
						$KEY_5.error(e);
						if ($KEY_4 != null) {
							$KEY_18.removeEventListener("wheel", $KEY_4, false);
						}
					}
				}