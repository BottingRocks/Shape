function $THREAD_5_140787($AQ, $AR, $AS, $AT) {
			let $KEY_0 = $AR;
			let $KEY_1 = $AT;
			let $KEY_4, $KEY_5;
			let $KEY_6 = $AS;
			let $KEY_7;
			function $THREAD_7_140685() {
				$KEY_5 = $KEY_4.ns_collect;
				function $THREAD_5_92214($AV) {
					let $AW;
					if ($AV == null) {
						$AW = true;
					} else {
						$AW = $AV.url != null && $AV.method != null;
						if ($AW) {
							$AW = $KEY_322($AV);
						}
					}
					if ($AW) {
						let $AX = $KEY_152(void 0);
						let $AY = Object.create(null);
						if ($AX !== null && $AX !== void 0) {
							let $AZ;
							for (const i in $AX) {
								if (Object.prototype.hasOwnProperty.call($AX, i)) {
									let $BA = $KEY_153("X-y8S6k3DB-" + i, $AX[i], 7900);
									if ($BA !== null && $BA !== void 0) {
										for (const j in $BA) {
											$AZ = j;
											if (!!Object.prototype.hasOwnProperty.call($BA, $AZ)) {
												$AY[$AZ] = $BA[$AZ];
											}
										}
									}
								}
							}
						}
						return $AY;
					} else {
						return null;
					}
				}
				$KEY_4.ns_collect = $THREAD_5_92214;
				if ($KEY_231) {
					$KEY_7 = $KEY_4[$KEY_231];
					function $THREAD_5_3964($AV) {
						let $KEY_3 = $AV;
						let $AW;
						if ($AV == null) {
							$AW = $KEY_263;
						} else {
							$AW = $KEY_263.filter($AX => $AX.a === $KEY_3);
						}
						return $AW.map($AX => {
							let $KEY_4 = $KEY_418($KEY_0, $AX.c.map($BA => $KEY_238[$BA]), $KEY_6, $KEY_1);
							let $AY = {
								beaconEndpoint: $AX.a,
								beaconDescriptor: $AX.d,
								payload: void 0
							};
							let $AZ = {
								signalFilter: $AX.b,
								notifyLastChance: true
							};
							$AZ.encode = $KEY_4;
							$AZ.descriptor = $AX.d;
							$AY.payload = $KEY_152($AZ);
							return $AY;
						});
					}
					$KEY_4[$KEY_231] = $THREAD_5_3964;
					return void 0;
				} else {
					return void 0;
				}
			}
			function $THREAD_5_92808() {
				if ($KEY_4.ns_collect !== $KEY_5) {
					$KEY_4.ns_collect = $KEY_5;
				}
				if ($KEY_4[$KEY_231] !== $KEY_7) {
					$KEY_4[$KEY_231] = $KEY_7;
				}
			}
			$KEY_4 = $AQ.global;
			let $AU = { init: $THREAD_7_140685 };
			$AU.teardown = $THREAD_5_92808;
			return $AU;
		}
