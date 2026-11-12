function $THREAD_0_16412() {
					try {
						let $AZ = $KEY_340();
						if ($AZ - $KEY_26 > 500) {
							++$KEY_20;
							$KEY_26 = $AZ;
						}
						let $BA = Math.random() * 4294967296 | 0;
						let $BB = [$BA >>> 24, $BA >> 16 & 255, $BA >> 8 & 255, $BA & 255];
						$BA = $KEY_356($BA ^ 3201877289);
						let $BC = $KEY_115;
						++$KEY_11;
						$BC = $BC($BA, $KEY_11, $BB, 3);
						$KEY_115($BA, $KEY_20, $BB, $BC);
						$KEY_25.next([[], $BB]);
					} catch (e) {
						$KEY_25.error(e);
						if ($KEY_24 != null) {
							$KEY_1.removeEventListener("wheel", $KEY_24, false);
						}
					}
				}