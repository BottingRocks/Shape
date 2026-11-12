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