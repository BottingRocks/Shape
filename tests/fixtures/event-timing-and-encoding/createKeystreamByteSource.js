function $THREAD_0_107316($AJ) {
			let $KEY_0, $KEY_2, $KEY_4, $KEY_5;
			function $THREAD_1_9772() {
				let $AK = $KEY_2;
				$KEY_2 = ($AK + 1) % 4;
				if (($AK + 1) % 4 === 0) {
					$KEY_4 = $KEY_5 + $KEY_0;
					let $AL = $KEY_0;
					$KEY_0 = $AL ^ $KEY_5;
					$KEY_5 = $AL ^ $KEY_5 ^ ($KEY_5 << 26 | $KEY_5 >>> 6) ^ $KEY_0 << 9;
					$KEY_0 = $KEY_0 << 13 | $KEY_0 >>> 19;
					return $KEY_4 & 255;
				}
				if (($AK + 1) % 4 === 1) {
					return $KEY_4 >> 8 & 255;
				}
				if (($AK + 1) % 4 === 2) {
					return $KEY_4 >> 16 & 255;
				}
				if (($AK + 1) % 4 === 3) {
					return $KEY_4 >>> 24 & 255;
				} else {
					return void 0;
				}
			}
			$KEY_5 = $KEY_289;
			$KEY_0 = $AJ;
			$KEY_2 = -1;
			$THREAD_1_9772();
			$THREAD_1_9772();
			$THREAD_1_9772();
			$THREAD_1_9772();
			return $THREAD_1_9772;
		}