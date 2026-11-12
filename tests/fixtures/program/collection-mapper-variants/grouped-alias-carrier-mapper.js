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