function $THREAD_0_109096($CK) {
				const $CL = $CK === void 0 ? {
					signalFilter: $KEY_287,
					includeDebugNames: false,
					notifyLastChance: true
				} : $CK;
				if ($CL.notifyLastChance == null || $CL.notifyLastChance) {
					$CL.signalFilter.forEach($CV => {
						if ($KEY_17[$CV] != null) {
							$KEY_17[$CV].call(void 0);
						}
					});
				}
				let $CM;
				if ($CL.includeDebugNames) {
					$CM = $KEY_60.map(($CV, $CW) => {
						return {
							value: $CV,
							signal: $KEY_152[$CW]
						};
					});
				} else {
					$CM = $CL.signalFilter.map($CV => $KEY_60[$CV]);
				}
				let $CN = [];
				let $CO = [];
				let $CP = [];
				for (let i = 0; i < $CM.length; i++) {
					let $CQ = $KEY_94[$CL.signalFilter[i]];
					let $CR, $CS, $CT;
					if (Object.prototype.hasOwnProperty.call($CQ, "name")) {
						$CR = typeof $CQ.name === "number" ? $CP : $CN;
						$CS = $CR.push;
						$CT = [$CQ.name, $CM[i]];
					} else {
						$CS = $CO.push;
						$CR = $CO;
						$CT = $CM[i];
					}
					$CS.call($CR, $CT);
				}
				return {
					signals: $CN,
					positionalSignals: $CO,
					keyedSignals: $CP,
					errors: $KEY_221.toArray()
				};
			}
