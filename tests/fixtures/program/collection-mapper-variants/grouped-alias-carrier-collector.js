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