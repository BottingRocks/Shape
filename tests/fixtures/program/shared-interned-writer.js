function main(){
function $THREAD_8_51922($AP) {
			let $KEY_1 = $AP;
			let $KEY_2;
			let $KEY_0 = Object.create(null);
			$KEY_2 = 0;
			function $THREAD_1_19915($AQ, $AR, $AS) {
				let $KEY_5;
				if ($KEY_0[$AQ] != null) {
					$KEY_5 = $AS;
					$KEY_5 = Number($AS) + 1;
					$AR[$KEY_5] = $KEY_1() ^ 129;
					$KEY_5 = $KEY_231($KEY_1, $KEY_0[$AQ], $AR, $KEY_5);
				} else {
					let $AT = $AQ.length;
					let $AU = 0;
					let $AV = $AS;
					while ($AU < $AT) {
						let $AW = $KEY_184($AQ[$AU], 0);
						if ($AW === 0) {
							$AR[++$AV] = $KEY_1() ^ 192;
							$AW = 128;
						} else if (!($AW < 128)) if ($AW < 2048) {
							$AR[++$AV] = ($AW >> 6 | 192) ^ $KEY_1();
							$AW = $AW & 63 | 128;
						} else {
							let $AX = $AW >= 55296 && $AW <= 57343 && $AW < 56320 && $AU + 1 < $AT ? $KEY_184($AQ[$AU + 1], 0) : null;
							if ($AX == null || $AX < 56320 || $AX > 57343) {
								$AR[++$AV] = ($AW >> 12 | 224) ^ $KEY_1();
								$AR[++$AV] = ($AW >> 6 & 63 | 128) ^ $KEY_1();
								$AW = $AW & 63 | 128;
							} else {
								++$AU;
								$AR[++$AV] = (65536 + ($AW - 55296 << 10) + ($AX - 56320) >> 18 | 240) ^ $KEY_1();
								$AR[++$AV] = (65536 + ($AW - 55296 << 10) + ($AX - 56320) >> 12 & 63 | 128) ^ $KEY_1();
								$AR[++$AV] = (65536 + ($AW - 55296 << 10) + ($AX - 56320) >> 6 & 63 | 128) ^ $KEY_1();
								$AW = 65536 + ($AW - 55296 << 10) + ($AX - 56320) & 63 | 128;
							}
						}
						$AR[++$AV] = $AW ^ $KEY_1();
						++$AU;
					}
					$KEY_5 = Number($AV) + 1;
					$AR[$KEY_5] = $KEY_1();
					let $AY = $KEY_0;
					let $AZ = Number($KEY_2);
					$KEY_2 = $AZ + 1;
					$AY[$AQ] = $AZ;
				}
				return $KEY_5;
			}
			return $THREAD_1_19915;
		}
return $THREAD_8_51922;
}main();
