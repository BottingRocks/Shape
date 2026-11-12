function main(){let $KEY_11,$KEY_146,$KEY_108,$KEY_216,$KEY_176,$KEY_341,$KEY_0;function $THREAD_1_2($AS) {
					let $AT;
					if ($AS != null) {
						$AT = $AS;
					} else {
						$AT = Object.create(null);
					}
					let $KEY_11 = $AT.signalFilter === void 0 ? $KEY_146 : $AT.signalFilter;
					const $AU = typeof ($AT.encode === void 0 ? null : $AT.encode) === "function" ? $AT.encode === void 0 ? null : $AT.encode : $KEY_108;
					let $AV = Object.create(null);
					$KEY_11 = {
						signalFilter: $KEY_11,
						includeDebugNames: false
					};
					$KEY_11.notifyLastChance = ($AT.notifyLastChance === void 0 || $AT.notifyLastChance) ?? true;
					$KEY_11 = $AU($KEY_216($KEY_11));
					let $AW = $KEY_176 ?? "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=";
					let $AX = $KEY_11.length - $KEY_11.length % 3;
					let $AY = 0;
					let $AZ = "";
					while ($AY < $AX) {
						let $BA = $KEY_11[$AY + 2];
						$AZ = $AZ + ($AW.charAt($KEY_11[$AY] >> 2) + $AW.charAt(($KEY_11[$AY] & 3) << 4 | $KEY_11[$AY + 1] >> 4) + $AW.charAt(($KEY_11[$AY + 1] & 15) << 2 | $BA >> 6) + $AW.charAt($BA & 63));
						$AY += 3;
					}
					if ($KEY_11.length % 3 === 2) {
						$AZ = $AZ + ($AW.charAt($KEY_11[$AY] >> 2) + $AW.charAt(($KEY_11[$AY] & 3) << 4 | $KEY_11[$AY + 1] >> 4) + $AW.charAt(($KEY_11[$AY + 1] & 15) << 2));
					} else if ($KEY_11.length % 3 === 1) {
						$KEY_11 = $KEY_11[$AY];
						$AZ = $AZ + ($AW.charAt($KEY_11 >> 2) + $AW.charAt(($KEY_11 & 3) << 4));
					}
					$AV.f = $KEY_341;
					$AV.b = $KEY_0($KEY_341 + $AZ).toString(36);
					$AV.c = "AGCoTXChAQAA9h7EGRae99EH0eFeTxDfu-Q1hb204ffXKxAlpbQa5GUaSfsN";
					$AV.d = $AT.descriptor === void 0 ? "ABYAhICBAKGAgQGAAYAQgISigaIAwAGAyPpCxg_12ocAgICHALzTmAgQtBrkZRpJ-w3_____2K6iCAD4AK4GN3Ve0wsujuqIJzwk" : $AT.descriptor;
					$AV.z = "q";
					$AV.a = $AZ;
					return $AV;
				}return $THREAD_1_2;}main();