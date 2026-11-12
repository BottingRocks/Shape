function $THREAD_2_8552($BY) {
						let $KEY_5 = $BY === void 0 ? {} : $BY;
						let $BZ;
						if ($KEY_5.signalFilter === void 0) {
							$BZ = $KEY_6;
						} else {
							$BZ = $KEY_5.signalFilter;
						}
						if ($KEY_5.notifyLastChance === void 0 || $KEY_5.notifyLastChance) {
							$BZ.forEach($CJ => {
								if ($KEY_15[$CJ] != null) {
									$KEY_15[$CJ].call(void 0);
								}
							});
						}
						let $CA;
						if ($KEY_5.includeDebugNames !== void 0 && $KEY_5.includeDebugNames) {
							$CA = $BZ.map($CJ => {
								return {
									value: $KEY_38[$CJ],
									signal: $KEY_313[$CJ]
								};
							});
						} else {
							$CA = $BZ.map($CJ => $KEY_38[$CJ]);
						}
						let $CB = [];
						let $CC = [];
						let $CD = [];
						let i = 0;
						for (; i < $CA.length; i++) {
							let $CE = $KEY_459[$BZ[i]];
							let $CF, $CG, $CH;
							if (Object.prototype.hasOwnProperty.call($CE, "name")) {
								$CF = typeof $CE.name === "number" ? $CC : $CB;
								$CG = $CF.push;
								$CH = [$CE.name, $CA[i]];
							} else {
								$CG = $CD.push;
								$CF = $CD;
								$CH = $CA[i];
							}
							$CG.call($CF, $CH);
						}
						return {
							signals: $CB,
							positionalSignals: $CD,
							keyedSignals: $CC,
							errors: $KEY_387.toArray()
						};
					}