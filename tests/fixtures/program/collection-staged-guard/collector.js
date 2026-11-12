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
