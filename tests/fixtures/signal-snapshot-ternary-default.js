function $THREAD_8_57294($CH) {
				let $KEY_2 = $CH === void 0 ? {} : $CH;
				let $CI = $KEY_2.signalFilter === void 0 ? $KEY_225 : $KEY_2.signalFilter;
				if ($KEY_2.notifyLastChance === void 0 || $KEY_2.notifyLastChance) {
					$CI.forEach($CS => {
						if ($KEY_14[$CS] != null) {
							$KEY_14[$CS].call(void 0);
						}
					});
				}
				let $CJ;
				if ($KEY_2.includeDebugNames !== void 0 && $KEY_2.includeDebugNames) {
					$CJ = $CI.map($CS => {
						return {
							value: $KEY_324[$CS],
							signal: $KEY_411[$CS]
						};
					});
				} else {
					$CJ = $CI.map($CS => $KEY_324[$CS]);
				}
				let $CK = [];
				let $CL = [];
				let $CM = [];
				let i = 0;
				for (; i < $CJ.length; i++) {
					let $CN = $KEY_165[$CI[i]];
					let $CO, $CP, $CQ;
					if (Object.prototype.hasOwnProperty.call($CN, "name")) {
						$CP = (typeof $CN.name === "number" ? $CL : $CM).push;
						$CQ = [$CN.name, $CJ[i]];
						$CO = typeof $CN.name === "number" ? $CL : $CM;
					} else {
						$CO = $CK;
						$CP = $CK.push;
						$CQ = $CJ[i];
					}
					$CP.call($CO, $CQ);
				}
				return {
					signals: $CM,
					positionalSignals: $CK,
					keyedSignals: $CL,
					errors: $KEY_416.toArray()
				};
			}