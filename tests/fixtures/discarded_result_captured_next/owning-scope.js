function $THREAD_0_2955($BO, $BP, $BQ, $BR) {
			let $KEY_5 = $BP;
			let $KEY_8, $KEY_10, $KEY_11, $KEY_12, $KEY_13, $KEY_14;
			$KEY_11 = $BO[0];
			$KEY_12 = $BO[1];
			$KEY_8 = $BO[2];
			$KEY_10 = $BO[3];
			let $KEY_4 = $BO[4];
			let $KEY_2 = $BP.global;
			$KEY_14 = $BP.outerArgs;
			let $KEY_3 = false;
			$KEY_13 = $BP.global;
			$BR(() => {
				let $BS = -1;
				let $BT = null;
				try {
					let $BU = new Intl.DateTimeFormat("ZZ").resolvedOptions();
					$BT = "" + $BU.locale;
					$BS = $KEY_399($BU.timeZone);
				} catch (e) {}
				let $BV = null;
				try {
					$BV = "" + new Intl.Collator("ZZ").resolvedOptions().locale;
				} catch (e_2) {}
				let $BW = null;
				try {
					$BW = "" + new Intl.DisplayNames([], { type: "language" }).resolvedOptions().locale;
				} catch (e_3) {}
				let $BX = null;
				try {
					$BX = "" + new Intl.Segmenter([]).resolvedOptions().locale;
				} catch (e_4) {}
				let $KEY_16 = $KEY_13.external;
				let $BY = -1;
				let $BZ = -1;
				let $CA = -1;
				let $CB = -1;
				let $CC = -1;
				let $CD = -1;
				try {
					$CA = Intl.supportedValuesOf("calendar").length;
					$CC = Intl.supportedValuesOf("collation").length;
					$BY = Intl.supportedValuesOf("currency").length;
					$CB = Intl.supportedValuesOf("numberingSystem").length;
					$CD = Intl.supportedValuesOf("timeZone").length;
					$BZ = Intl.supportedValuesOf("unit").length;
				} catch (e_5) {}
				let $CE = Math.random() * 4294967296;
				let $KEY_0 = [($CE | 0) >>> 24, ($CE | 0) >> 16 & 255, ($CE | 0) >> 8 & 255, ($CE | 0) & 255];
				let $KEY_3 = $KEY_83(($CE | 0) ^ 247123185);
				let $CF = $KEY_254($KEY_3);
				let $CG = $KEY_197($KEY_3, new Date().getTimezoneOffset(), $KEY_0, 3);
				$BY = $KEY_197($KEY_3, $BY, $KEY_0, $CG);
				$CC = $KEY_197($KEY_3, $CC, $KEY_0, $BY);
				$CB = $KEY_197($KEY_3, $CB, $KEY_0, $CC);
				if ($KEY_16 != null && $KEY_16.getHostEnvironmentValue != null) {
					try {
						let $CH = $KEY_16.getHostEnvironmentValue(["browser-name", "browser-platform-version", "browser-version", "device-sku", "device-dpi", "device-logical-processors", "device-max-touch-points", "device-pixel-height", "device-pixel-width", "device-role", "os-architecture", "os-build", "os-language", "os-manufacturer", "os-mode", "os-sku"]);
						let $CI = Math.random() * 4294967296;
						let $CJ = [($CI | 0) >>> 24, ($CI | 0) >> 16 & 255, ($CI | 0) >> 8 & 255, ($CI | 0) & 255];
						$CI = $KEY_83(($CI | 0) ^ 2962043072);
						$CJ[$KEY_254($CI)($CH, $CJ, 3) + 1] = $CI() ^ 131;
						$KEY_10.next([[], $CJ]);
					} catch (e_6) {
						let $CK = Math.random() * 4294967296;
						let $CL = [($CK | 0) >>> 24, ($CK | 0) >> 16 & 255, ($CK | 0) >> 8 & 255, ($CK | 0) & 255];
						$CK = $KEY_83(($CK | 0) ^ 2962043072);
						let $CM = $KEY_254($CK);
						$CL[4] = $CK() ^ 130;
						$CM(e_6.message || "No message", $CL, 4);
						$KEY_10.next([[], $CL]);
					}
				}
				if ($BT == null) {
					$BT = $CB + 1;
					$KEY_0[$BT] = $KEY_3() ^ 130;
				} else {
					$BT = $CF($BT, $KEY_0, $CB);
				}
				if ($BW == null) {
					$BW = $BT + 1;
					$KEY_0[$BW] = $KEY_3() ^ 131;
				} else {
					$BW = $CF($BW, $KEY_0, $BT);
				}
				$CA = $KEY_197($KEY_3, $CA, $KEY_0, $BW);
				if ($BX == null) {
					$BX = $CA + 1;
					$KEY_0[$BX] = $KEY_3() ^ 132;
				} else {
					$BX = $CF($BX, $KEY_0, $CA);
				}
				if ($BV == null) {
					$CF = $BX + 1;
					$KEY_0[$CF] = $KEY_3() ^ 133;
				} else {
					$CF = $CF($BV, $KEY_0, $BX);
				}
				$BZ = $KEY_197($KEY_3, $BZ, $KEY_0, $CF);
				$BS = $KEY_197($KEY_3, $BS, $KEY_0, $BZ);
				$KEY_197($KEY_3, $CD, $KEY_0, $BS);
				$KEY_11.next([[], $KEY_0]);
				$KEY_11.complete();
				$KEY_10.complete();
			}, false);
			$BQ(() => {
				function vmPostToWorker(first, second, third, operand, callback) {
					if (!vmInitializeWorker()) {
						return false;
					}
					vmWorkerCallbacks.push(callback);
					vmWorker.postMessage([first, second, third, operand]);
					return true;
				}
				let $BS = null;
				try {
					let $BT = JSON.stringify($KEY_14);
					if ($BT != null) {
						$BT = [$BT, $BT.slice];
						$BS = $BT[1].call($BT[0], 0, 256);
					}
				} catch (e) {
					$KEY_3 = true;
				}
				let $KEY_0 = Object.keys($KEY_5.global.location).length;
				let $KEY_11 = Math.random() * 4294967296 | 0;
				let $KEY_9 = [$KEY_11 >>> 24, $KEY_11 >> 16 & 255, $KEY_11 >> 8 & 255, $KEY_11 & 255];
				if (!($KEY_0 < 30)) {
					$KEY_0 = 30;
				}
				let $BU = [];
				let $BV = $KEY_5.global.location;
				let $BW;
				if ($BV !== null && $BV !== void 0) {
					let $BX = 0;
					$BW = "";
					for (const $BY in $BV) {
						let $BZ = $KEY_252[$BY];
						++$BX;
						if ($BX > $KEY_0) {
							let $CA = Math.random() * 4294967296;
							let $CB = [($CA | 0) >>> 24, ($CA | 0) >> 16 & 255, ($CA | 0) >> 8 & 255, ($CA | 0) & 255];
							function $THREAD_0_19761($CI) {
								$KEY_8.next($CI);
							}
							vmPostToWorker(7, 93340, {
								a: $KEY_5.transactionToken,
								b: void 0
							}, 461, $THREAD_0_19761);
							$KEY_11 = $KEY_83($KEY_11 ^ 666231455);
							let $CC = $KEY_254($KEY_11);
							$CA = $KEY_83(($CA | 0) ^ 4139766376);
							let $CD = $KEY_2.process !== void 0 | 0 | ($KEY_14 !== void 0) << 1;
							$CA = $KEY_254($CA);
							if ($BS == null) {
								$KEY_9[4] = $KEY_11() ^ 130;
								$CC = 4;
							} else {
								$CC = $CC($BS, $KEY_9, 3);
							}
							$CA($BW + (($BZ || "u") + "\0"), $CB, 3);
							$KEY_12.next([[], $CB]);
							$KEY_9[$CC + 1] = ($CD | $KEY_3 << 2 | ($KEY_2.global === $KEY_2) << 3) ^ $KEY_11();
							$KEY_4.next([$BU, $KEY_9]);
							$KEY_12.complete();
							$KEY_4.complete();
							return void 0;
						}
						$BW = $BW + (($BZ || "u") + "\0");
					}
				} else {
					$BW = "";
				}
				let $CE = Math.random() * 4294967296;
				let $CF = [($CE | 0) >>> 24, ($CE | 0) >> 16 & 255, ($CE | 0) >> 8 & 255, ($CE | 0) & 255];
				vmPostToWorker(7, 93340, {
					a: $KEY_5.transactionToken,
					b: void 0
				}, 461, $CI => {
					$KEY_8.next($CI);
				});
				$KEY_11 = $KEY_83($KEY_11 ^ 666231455);
				let $CG = $KEY_254($KEY_11);
				$CE = $KEY_83(($CE | 0) ^ 4139766376);
				let $CH = $KEY_2.process !== void 0 | 0 | ($KEY_14 !== void 0) << 1;
				$KEY_9 = $BS;
				$CE = $KEY_254($CE);
				if ($KEY_9 == null) {
					$KEY_9[4] = $KEY_11() ^ 130;
					$CG = 4;
				} else {
					$CG = $CG($KEY_9, $KEY_9, 3);
				}
				$CE($BW, $CF, 3);
				$KEY_12.next([[], $CF]);
				$KEY_9[$CG + 1] = ($CH | $KEY_3 << 2 | ($KEY_2.global === $KEY_2) << 3) ^ $KEY_11();
				$KEY_4.next([$BU, $KEY_9]);
				$KEY_12.complete();
				$KEY_4.complete();
			});
		}