function $THREAD_7_39465($AJ, $AK, $AL, $AM) {
			let $KEY_2;
			if (($AK | 0) === $AK) {
				let $AN;
				if ($AK < 0) {
					$AN = $AK < 0;
				} else if ($AK === 0) {
					$AN = 1 / $AK < 0;
				} else {
					$AN = false;
				}
				let $AO, $AP;
				if ($AN) {
					$AO = -$AK;
					$AP = 64;
				} else {
					$AO = $AK;
					$AP = 0;
				}
				if ($AO < 32) {
					$KEY_2 = Number($AM) + 1;
					$AL[$KEY_2] = ($AP | $AO) ^ $AJ();
				} else if ($AO < 4096) {
					$KEY_2 = Number($AM) + 1;
					$AL[$KEY_2] = ($AP | ($AO & 31 | 32)) ^ $AJ();
					$AL[++$KEY_2] = $AO >> 5 ^ $AJ();
				} else if ($AO < 524288) {
					$KEY_2 = Number($AM) + 1;
					$AL[$KEY_2] = ($AP | ($AO & 31 | 32)) ^ $AJ();
					$AL[++$KEY_2] = ($AO >> 5 & 127 | 128) ^ $AJ();
					$AL[++$KEY_2] = $AO >>> 12 ^ $AJ();
				} else if ($AO < 67108864) {
					$KEY_2 = Number($AM) + 1;
					$AL[$KEY_2] = ($AP | ($AO & 31 | 32)) ^ $AJ();
					$AL[++$KEY_2] = ($AO >> 5 & 127 | 128) ^ $AJ();
					$AL[++$KEY_2] = ($AO >>> 12 & 127 | 128) ^ $AJ();
					$AL[++$KEY_2] = $AO >>> 19 ^ $AJ();
				} else {
					$KEY_2 = Number($AM) + 1;
					$AL[$KEY_2] = ($AP | ($AO & 31 | 32)) ^ $AJ();
					$AL[++$KEY_2] = ($AO >> 5 & 127 | 128) ^ $AJ();
					$AL[++$KEY_2] = ($AO >>> 12 & 127 | 128) ^ $AJ();
					$AL[++$KEY_2] = ($AO >>> 19 & 127 | 128) ^ $AJ();
					$AL[++$KEY_2] = $AO >>> 26 ^ $AJ();
				}
			} else {
				$KEY_2 = $AM;
				$KEY_2 = Number($AM) + 1;
				$AL[$KEY_2] = $AJ() ^ 128;
				let $AQ;
				if ($KEY_84) {
					$KEY_84[0] = $AK;
					$AQ = new $KEY_225($KEY_84.buffer);
				} else if ($AK !== $AK) {
					$AQ = $KEY_126;
				} else {
					let $KEY_6 = $AK < 0 || $AK === 0 && 1 / $AK < 0;
					let $AR = Math.abs($AK);
					let $AS, $AT;
					if ($AR >= Math.pow(2, -1022)) {
						let $KEY_9 = Math.min(Math.floor(Math.log($AR) / Math.LN2), 1023);
						$AR = $AR / Math.pow(2, $KEY_9);
						$AT = $AR < 1;
						$AR = $AT ? $AR * 2 : $AR;
						$AT = $AT ? $KEY_9 - 1 : $KEY_9;
						if ($AR >= 2) {
							++$AT;
							$AR = $AR / 2;
						}
						let $KEY_1 = Math.pow(2, 52);
						$AR = $AR * $KEY_1;
						let $AU = Math.floor($AR);
						if (!($AR - $AU < .5)) if ($AR - $AU > .5) {
							++$AU;
						} else {
							$AU = [$AU, $AU % 2];
							$AU = $AU[1] ? $AU[0] + 1 : $AU[0];
						}
						let $KEY_7 = $AU - $KEY_1;
						$KEY_1 = $KEY_7 / $KEY_1 >= 1;
						$AT = $KEY_1 ? $AT + 1024 : $AT + 1023;
						if ($AT > 2046) {
							$AS = 0;
							$AT = 2047;
						} else {
							$AS = $KEY_1 ? 0 : $KEY_7;
						}
					} else {
						$AR = $AR / Math.pow(2, -1074);
						$AS = Math.floor($AR);
						if (!($AR - $AS < .5)) if ($AR - $AS > .5) {
							++$AS;
						} else {
							$AS = [$AS, $AS % 2];
							$AS = $AS[1] ? $AS[0] + 1 : $AS[0];
						}
						$AT = 0;
					}
					let $AV = [];
					let k = 51;
					for (; k >= 0; k--) {
						$AV.unshift(!!($AS & 1));
						$AS = Math.floor($AS / 2);
					}
					let i = 11;
					let $AW, $AX;
					for (;; i--) {
						$AW = $AV;
						$AX = $AV;
						if (i <= 0) break;
						$AX.unshift.call($AW, !!($AT & 1));
						$AT = Math.floor($AT / 2);
					}
					$AX.unshift.call($AW, $KEY_6);
					$AQ = [];
					let j = 56;
					for (; j >= 0; j = j - 8) {
						$AQ.push(Number($AV[j]) << 7 | Number($AV[j + 1]) << 6 | Number($AV[j + 2]) << 5 | Number($AV[j + 3]) << 4 | Number($AV[j + 4]) << 3 | Number($AV[j + 5]) << 2 | Number($AV[j + 6]) << 1 | Number($AV[j + 7]));
					}
				}
				let i_2 = 0;
				for (; i_2 < 8; i_2++) {
					$AL[++$KEY_2] = $AQ[i_2] ^ $AJ();
				}
			}
			return $KEY_2;
		}