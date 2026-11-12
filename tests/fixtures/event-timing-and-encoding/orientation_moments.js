function $THREAD_8_51153_2($DG) {
					try {
						let $DH = $KEY_190();
						++$KEY_22;
						if ($KEY_22 <= 1) {
							$KEY_30 = $DH;
							$KEY_29 = $DG.alpha;
							$KEY_26 = $DG.alpha * $DG.alpha;
							$KEY_21 = $DG.beta;
							$KEY_15 = $DG.beta * $DG.beta;
							$KEY_10 = $DG.gamma;
							$KEY_1 = $DG.gamma * $DG.gamma;
						} else {
							$KEY_9 = $KEY_9 * (($KEY_22 - 2) / ($KEY_22 - 1)) + ($DH - $KEY_30) * (1 / ($KEY_22 - 1));
							$KEY_0 = $KEY_0 * (($KEY_22 - 2) / ($KEY_22 - 1)) + ($DH - $KEY_30) * ($DH - $KEY_30) * (1 / ($KEY_22 - 1));
							$KEY_30 = $DH;
							$KEY_29 = $KEY_29 * (($KEY_22 - 1) / $KEY_22) + $DG.alpha * (1 / $KEY_22);
							$KEY_21 = $KEY_21 * (($KEY_22 - 1) / $KEY_22) + $DG.beta * (1 / $KEY_22);
							$KEY_10 = $KEY_10 * (($KEY_22 - 1) / $KEY_22) + $DG.gamma * (1 / $KEY_22);
							$KEY_26 = $KEY_26 * (($KEY_22 - 1) / $KEY_22) + $DG.alpha * $DG.alpha * (1 / $KEY_22);
							$KEY_15 = $KEY_15 * (($KEY_22 - 1) / $KEY_22) + $DG.beta * $DG.beta * (1 / $KEY_22);
							$KEY_1 = $KEY_1 * (($KEY_22 - 1) / $KEY_22) + $DG.gamma * $DG.gamma * (1 / $KEY_22);
						}
					} catch (e) {
						$KEY_11.error(e);
					}
				}