function $THREAD_1_44111($AI, $AJ, $AK, $AL) {
			let $KEY_2 = $AL;
			let $KEY_4 = $AJ;
			let $KEY_5, $KEY_8;
			let $KEY_9 = $AK;
			let $KEY_1;
			function $THREAD_6_58643() {
				if ($KEY_1.ns_collect !== $KEY_5) {
					$KEY_1.ns_collect = $KEY_5;
				}
				if ($KEY_1[$KEY_134] !== $KEY_8) {
					$KEY_1[$KEY_134] = $KEY_8;
				}
			}
			function $THREAD_0_76762() {
				$KEY_5 = $KEY_1.ns_collect;
				function $THREAD_1_8190($AN) {
					if ($AN == null || $AN.url != null && $AN.method != null && $KEY_127($AN)) {
						let $AO = $KEY_141(void 0);
						let $AP = Object.create(null);
						if ($AO !== null && $AO !== void 0) {
							let $AQ = void 0;
							for (const i in $AO) {
								if (Object.prototype.hasOwnProperty.call($AO, i)) {
									let $AR = $KEY_348("X-y8S6k3DB-" + i, $AO[i], 7900);
									if ($AR !== null && $AR !== void 0) {
										for (const j in $AR) {
											$AQ = j;
											if (!!Object.prototype.hasOwnProperty.call($AR, $AQ)) {
												$AP[$AQ] = $AR[$AQ];
											}
										}
									}
								}
							}
						}
						return $AP;
					} else {
						return null;
					}
				}
				$KEY_1.ns_collect = $THREAD_1_8190;
				if ($KEY_134) {
					$KEY_8 = $KEY_1[$KEY_134];
					function $THREAD_9_79372($AN) {
						let $KEY_1 = $AN;
						let $AO;
						if ($AN == null) {
							$AO = $KEY_417;
						} else {
							$AO = $KEY_417.filter($AP => $AP.a === $KEY_1);
						}
						return $AO.map($AP => {
							let $KEY_3 = $KEY_206($KEY_4, $AP.c.map($AS => $KEY_216[$AS]), $KEY_9, $KEY_2);
							let $AQ = {
								beaconEndpoint: $AP.a,
								beaconDescriptor: $AP.d,
								payload: void 0
							};
							let $AR = {
								signalFilter: $AP.b,
								notifyLastChance: true
							};
							$AR.encode = $KEY_3;
							$AR.descriptor = $AP.d;
							$AQ.payload = $KEY_141($AR);
							return $AQ;
						});
					}
					$KEY_1[$KEY_134] = $THREAD_9_79372;
					return void 0;
				} else {
					return void 0;
				}
			}
			$KEY_1 = $AI.global;
			let $AM = { init: $THREAD_0_76762 };
			$AM.teardown = $THREAD_6_58643;
			return $AM;
		}