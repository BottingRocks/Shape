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