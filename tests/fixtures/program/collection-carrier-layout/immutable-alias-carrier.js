function $THREAD_0_25672($AL, $AM, $AN, $AO) {
			let $KEY_0 = $AM;
			let $KEY_1;
			let $KEY_2 = $AN;
			let $KEY_3 = $AO;
			let $KEY_4, $KEY_6;
			function $THREAD_7_143361() {
				$KEY_6 = $KEY_1.ns_collect;
				function $THREAD_6_76040($AQ) {
					if ($AQ == null || $AQ.url != null && $AQ.method != null && $KEY_264($AQ)) {
						let $AR = $KEY_57(void 0);
						let $AS = Object.create(null);
						if ($AR !== null && $AR !== void 0) {
							let $AT = void 0;
							for (const i in $AR) {
								if (Object.prototype.hasOwnProperty.call($AR, i)) {
									let $AU = $KEY_244("X-y8S6k3DB-" + i, $AR[i], 7900);
									if ($AU !== null && $AU !== void 0) {
										for (const j in $AU) {
											$AT = j;
											if (!!Object.prototype.hasOwnProperty.call($AU, $AT)) {
												$AS[$AT] = $AU[$AT];
											}
										}
									}
								}
							}
						}
						return $AS;
					} else {
						return null;
					}
				}
				$KEY_1.ns_collect = $THREAD_6_76040;
				if ($KEY_341) {
					$KEY_4 = $KEY_1[$KEY_341];
					function $THREAD_7_97278($AQ) {
						let $KEY_1 = $AQ;
						let $AR;
						if ($AQ == null) {
							$AR = $KEY_371;
						} else {
							$AR = $KEY_371.filter($AS => $AS.a === $KEY_1);
						}
						return $AR.map($AS => {
							let $KEY_5 = $KEY_10($KEY_0, $AS.c.map($AV => $KEY_426[$AV]), $KEY_2, $KEY_3);
							let $AT = {
								beaconEndpoint: $AS.a,
								beaconDescriptor: $AS.d,
								payload: void 0
							};
							let $AU = {
								signalFilter: $AS.b,
								notifyLastChance: true
							};
							$AU.encode = $KEY_5;
							$AU.descriptor = $AS.d;
							$AT.payload = $KEY_57($AU);
							return $AT;
						});
					}
					$KEY_1[$KEY_341] = $THREAD_7_97278;
					return void 0;
				} else {
					return void 0;
				}
			}
			function $THREAD_3_9833() {
				if ($KEY_1.ns_collect !== $KEY_6) {
					$KEY_1.ns_collect = $KEY_6;
				}
				if ($KEY_1[$KEY_341] !== $KEY_4) {
					$KEY_1[$KEY_341] = $KEY_4;
				}
			}
			$KEY_1 = $AL.global;
			let $AP = { init: $THREAD_7_143361 };
			$AP.teardown = $THREAD_3_9833;
			return $AP;
		}