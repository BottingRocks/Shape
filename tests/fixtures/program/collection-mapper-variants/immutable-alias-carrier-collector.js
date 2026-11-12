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