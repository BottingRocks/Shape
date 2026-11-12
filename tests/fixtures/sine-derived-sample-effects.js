function $THREAD_9_13908($AN) {
					let $AO = 1;
					let $AP = 8;
					let $AQ = $AN[0] * 63 + $AN[1];
					while ($AO <= $AP) {
						$AQ = Math.abs(Math.fround(Math.sin($AQ + $AO)) * 2);
						if ($AQ >= 1) {
							if ($AP < 50) {
								++$AP;
								++$AO;
								continue;
							}
							$AQ = ($AP - $AO) / $AP;
						}
						$AN.push($AQ);
						++$AO;
					}
					$AN[1] = Number($AN[1]) + 1;
				}