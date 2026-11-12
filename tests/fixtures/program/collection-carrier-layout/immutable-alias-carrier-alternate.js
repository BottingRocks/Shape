function $THREAD_8_71882($AQ, $AR, $AS, $AT) {
			let $KEY_1 = $AT;
			let $KEY_2;
			let $KEY_3 = $AS;
			let $KEY_7;
			let $KEY_8 = $AR;
			let $KEY_0;
			function $THREAD_3_476() {
				$KEY_7 = $KEY_0.ns_collect;
				function $THREAD_7_15125($AU) {
					if ($AU == null || $AU.url != null && $AU.method != null && $KEY_402($AU)) {
						let $AV = $KEY_118(void 0);
						let $AW = Object.create(null);
						if ($AV !== null && $AV !== void 0) {
							let $AX;
							for (const i in $AV) {
								if (Object.prototype.hasOwnProperty.call($AV, i)) {
									let $AY = $KEY_174("X-y8S6k3DB-" + i, $AV[i], 7900);
									if ($AY !== null && $AY !== void 0) {
										for (const j in $AY) {
											$AX = j;
											if (!!Object.prototype.hasOwnProperty.call($AY, $AX)) {
												$AW[$AX] = $AY[$AX];
											}
										}
									}
								}
							}
						}
						return $AW;
					} else {
						return null;
					}
				}
				$KEY_0.ns_collect = $THREAD_7_15125;
				if ($KEY_283) {
					$KEY_2 = $KEY_0[$KEY_283];
					function $THREAD_4_49099($AU) {
						let $KEY_2 = $AU;
						let $AV;
						if ($AU == null) {
							$AV = $KEY_128;
						} else {
							$AV = $KEY_128.filter($AW => $AW.a === $KEY_2);
						}
						return $AV.map($AW => {
							let $KEY_0 = $KEY_98($KEY_8, $AW.c.map($AZ => $KEY_105[$AZ]), $KEY_3, $KEY_1);
							let $AX = {
								beaconEndpoint: $AW.a,
								beaconDescriptor: $AW.d,
								payload: void 0
							};
							let $AY = {
								signalFilter: $AW.b,
								notifyLastChance: true
							};
							$AY.encode = $KEY_0;
							$AY.descriptor = $AW.d;
							$AX.payload = $KEY_118($AY);
							return $AX;
						});
					}
					$KEY_0[$KEY_283] = $THREAD_4_49099;
					return void 0;
				} else {
					return void 0;
				}
			}
			function $THREAD_1_42949() {
				if ($KEY_0.ns_collect !== $KEY_7) {
					$KEY_0.ns_collect = $KEY_7;
				}
				if ($KEY_0[$KEY_283] !== $KEY_2) {
					$KEY_0[$KEY_283] = $KEY_2;
				}
			}
			$KEY_0 = $AQ.global;
			return {
				init: $THREAD_3_476,
				teardown: $THREAD_1_42949
			};
		}