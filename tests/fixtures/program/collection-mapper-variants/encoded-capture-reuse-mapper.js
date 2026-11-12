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