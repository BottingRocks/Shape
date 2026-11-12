function $THREAD_0_44765($AJ, $AK, $AL, $AM) {
			let $KEY_1 = $AM;
			let $KEY_3 = $AL;
			let $KEY_4, $KEY_7;
			let $KEY_8 = $AK;
			let $KEY_0;
			function $THREAD_2_109193() {
				$KEY_4 = $KEY_0.ns_collect;
				function $THREAD_9_94056($AO) {
					if ($AO == null || $AO.url != null && $AO.method != null && $KEY_60($AO)) {
						let $AP = $KEY_427(void 0);
						let $AQ = Object.create(null);
						if ($AP !== null && $AP !== void 0) {
							let $AR;
							for (const i in $AP) {
								if (Object.prototype.hasOwnProperty.call($AP, i)) {
									let $AS = $KEY_268("X-y8S6k3DB-" + i, $AP[i], 7900);
									if ($AS !== null && $AS !== void 0) {
										for (const j in $AS) {
											$AR = j;
											if (!!Object.prototype.hasOwnProperty.call($AS, $AR)) {
												$AQ[$AR] = $AS[$AR];
											}
										}
									}
								}
							}
						}
						return $AQ;
					} else {
						return null;
					}
				}
				$KEY_0.ns_collect = $THREAD_9_94056;
				if ($KEY_283) {
					$KEY_7 = $KEY_0[$KEY_283];
					function $THREAD_5_36339($AO) {
						let $KEY_0 = $AO;
						let $AP;
						if ($AO == null) {
							$AP = $KEY_118;
						} else {
							$AP = $KEY_118.filter($AQ => $AQ.a === $KEY_0);
						}
						return $AP.map($AQ => {
							let $AR = $KEY_100($KEY_8, $AQ.c.map($AT => $KEY_155[$AT]), $KEY_3, $KEY_1);
							let $AS = {
								beaconEndpoint: $AQ.a,
								beaconDescriptor: $AQ.d,
								payload: void 0
							};
							$AR = {
								signalFilter: $AQ.b,
								notifyLastChance: true,
								encode: $AR,
								descriptor: $AQ.d
							};
							$AS.payload = $KEY_427($AR);
							return $AS;
						});
					}
					$KEY_0[$KEY_283] = $THREAD_5_36339;
					return void 0;
				} else {
					return void 0;
				}
			}
			function $THREAD_4_14381() {
				if ($KEY_0.ns_collect !== $KEY_4) {
					$KEY_0.ns_collect = $KEY_4;
				}
				if ($KEY_0[$KEY_283] !== $KEY_7) {
					$KEY_0[$KEY_283] = $KEY_7;
				}
			}
			$KEY_0 = $AJ.global;
			let $AN = { init: $THREAD_2_109193 };
			$AN.teardown = $THREAD_4_14381;
			return $AN;
		}