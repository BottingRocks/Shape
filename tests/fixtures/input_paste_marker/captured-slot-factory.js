function $THREAD_8_124098($AI, $AJ, $AK, $AL, $AM, $AN) {
			let $KEY_12 = $AJ;
			let $KEY_13, $KEY_14, $KEY_16, $KEY_18;
			$KEY_13 = $AI[0];
			let $KEY_8 = $AI[1];
			let $KEY_6 = $AI[2];
			let $KEY_11 = $AI[3];
			let $KEY_10 = $AI[4];
			$KEY_18 = $KEY_12.global.document;
			$KEY_16 = null;
			let $KEY_5 = $KEY_12.global;
			let $KEY_3 = $KEY_12.persistentId.key;
			let $KEY_0 = false;
			$KEY_14 = $KEY_12.global;
			let $KEY_4 = 0;
			$AK(() => {
				let $AO = Math.random() * 4294967296;
				let $KEY_19 = Object.keys($KEY_12.global.location).length;
				let $AP = [($AO | 0) >>> 24, ($AO | 0) >> 16 & 255, ($AO | 0) >> 8 & 255, ($AO | 0) & 255];
				let $AQ = [];
				function $THREAD_5_85895($BP) {
					try {
						if ($BP.target.tagName === "INPUT") {
							if ($BP.inputType === "insertFromPaste") {
								++$KEY_4;
							} else if ($BP.target !== $KEY_18.activeElement) {
								$KEY_0 = true;
							}
						}
					} catch (e) {
						$KEY_13.error(e);
					}
				}
				$KEY_16 = $THREAD_5_85895;
				let $AR = $KEY_12.global.location;
				let $AS;
				if ($AR !== null && $AR !== void 0) {
					$AS = "";
					let $AT = 0;
					for (const $AU in $AR) {
						let $AV = $KEY_391[$AU];
						++$AT;
						if ($AT > ($KEY_19 < 30 ? $KEY_19 : 30)) {
							let $AW, $AX;
							if (!$KEY_265) {
								$AW = null;
								$AX = 1;
							} else if ($KEY_256) {
								$AW = null;
								try {
									$AW = $KEY_287 ? $KEY_287.getItem($KEY_3) : null;
								} catch (e) {}
								$AX = 3;
							} else {
								$AW = null;
								$AX = 2;
							}
							$AO = $KEY_420(($AO | 0) ^ 4111443479);
							let $AY;
							if ($AW == null) {
								let $AZ = Math.random() * 4294967296 | 0;
								$AY = [$AZ >>> 24, $AZ >> 16 & 255, $AZ >> 8 & 255, $AZ & 255];
								$AZ = $KEY_420($AZ ^ 1131554097);
								$KEY_328($AZ);
								$AY[$KEY_224($AZ, $AX, $AY, 3) + 1] = $AZ() ^ 130;
							} else {
								let $BA = Math.random() * 4294967296 | 0;
								$AY = [$BA >>> 24, $BA >> 16 & 255, $BA >> 8 & 255, $BA & 255];
								$BA = $KEY_420($BA ^ 1131554097);
								$KEY_328($BA)($AW, $AY, $KEY_224($BA, $AX, $AY, 3));
							}
							$KEY_10.next([[], $AY]);
							let $BB = $KEY_328($AO);
							let $BC = $KEY_14.location.hostname;
							if ($BC == null) {
								$AP[4] = $AO() ^ 130;
								$BC = 4;
							} else {
								$BC = $BB($BC, $AP, 3);
							}
							let $BD = Math.random() * 4294967296 | 0;
							let $BE = $KEY_14.location.port;
							if ($BE == null) {
								$BE = $BC + 1;
								$AP[$BE] = $AO() ^ 131;
							} else {
								$BE = $BB($BE, $AP, $BC);
							}
							$BE = $BB($KEY_14.location.href.substring(0, 100), $AP, $BE);
							$KEY_18.addEventListener("input", $KEY_16, true);
							let $BF = [$BD >>> 24, $BD >> 16 & 255, $BD >> 8 & 255, $BD & 255];
							$BB($KEY_14.location.protocol, $AP, $BE);
							$BD = $KEY_420($BD ^ 3679248011);
							$KEY_11.next([$AQ, $AP]);
							$KEY_328($BD)($AS + (($AV || "u") + "\0"), $BF, 3);
							$KEY_6.next([[], $BF]);
							$KEY_6.complete();
							$KEY_11.complete();
							$KEY_10.complete();
							return void 0;
						}
						$AS = $AS + (($AV || "u") + "\0");
					}
				} else {
					$AS = "";
				}
				let $BG, $BH;
				if (!$KEY_265) {
					$BG = null;
					$BH = 1;
				} else if ($KEY_256) {
					$BG = null;
					try {
						$BG = $KEY_287 ? $KEY_287.getItem($KEY_3) : null;
					} catch (e_2) {}
					$BH = 3;
				} else {
					$BG = null;
					$BH = 2;
				}
				$AO = $KEY_420(($AO | 0) ^ 4111443479);
				let $BI;
				if ($BG == null) {
					let $KEY_9 = Math.random() * 4294967296 | 0;
					$BI = [$KEY_9 >>> 24, $KEY_9 >> 16 & 255, $KEY_9 >> 8 & 255, $KEY_9 & 255];
					let $KEY_5 = $KEY_420($KEY_9 ^ 1131554097);
					$KEY_328($KEY_5);
					$BI[$KEY_224($KEY_5, $BH, $BI, 3) + 1] = $KEY_5() ^ 130;
				} else {
					let $BJ = Math.random() * 4294967296 | 0;
					$BI = [$BJ >>> 24, $BJ >> 16 & 255, $BJ >> 8 & 255, $BJ & 255];
					$BJ = $KEY_420($BJ ^ 1131554097);
					$KEY_328($BJ)($BG, $BI, $KEY_224($BJ, $BH, $BI, 3));
				}
				$KEY_10.next([[], $BI]);
				let $KEY_20 = $KEY_328($AO);
				let $BK = $KEY_14.location.hostname;
				if ($BK == null) {
					$AP[4] = $AO() ^ 130;
					$BK = 4;
				} else {
					$BK = $KEY_20($BK, $AP, 3);
				}
				let $BL = Math.random() * 4294967296 | 0;
				let $BM = $KEY_14.location.port;
				if ($BM == null) {
					$BM = $BK + 1;
					$AP[$BM] = $AO() ^ 131;
				} else {
					$BM = $KEY_20($BM, $AP, $BK);
				}
				$BM = $KEY_20($KEY_14.location.href.substring(0, 100), $AP, $BM);
				$KEY_18.addEventListener("input", $KEY_16, true);
				let $BN = [$BL >>> 24, $BL >> 16 & 255, $BL >> 8 & 255, $BL & 255];
				$KEY_20($KEY_14.location.protocol, $AP, $BM);
				$BL = $KEY_420($BL ^ 3679248011);
				$KEY_11.next([$AQ, $AP]);
				$KEY_328($BL)($AS, $BN, 3);
				$KEY_6.next([[], $BN]);
				$KEY_6.complete();
				$KEY_11.complete();
				$KEY_10.complete();
			});
			$AL(() => {
				let $KEY_0 = [];
				let $AO = $KEY_425($AQ => {
					let $AR = $KEY_0;
					let i = 0;
					let $AS = $KEY_5;
					for (; i < $AQ.length; i++) {
						$AS = $AS[$AQ[i]] == null || typeof $AS[$AQ[i]] !== "object" && typeof $AS[$AQ[i]] !== "function" ? Object.create(null) : $AS[$AQ[i]];
					}
					let $AT = [];
					if ($AS !== null && $AS !== void 0) {
						for (const j in $AS) {
							$AT.push(j);
						}
					}
					$AR.push($KEY_134($AT.join("\0"), 45).pipe($KEY_410($AU => $AU + 2147483648)));
				}, 45, [["Navigator", "prototype"], ["CanvasRenderingContext2D", "prototype"], ["chrome", "app"], ["safari"], ["ApplePaySetup", "prototype"]]);
				let $AP = [$AO, $AO.onEmpty];
				$AP[1].call($AP[0], () => {
					let $AQ = $KEY_281($KEY_0);
					function $THREAD_3_147216($AS) {
						let $KEY_2 = Math.random() * 4294967296 | 0;
						let $KEY_0 = [$KEY_2 >>> 24, $KEY_2 >> 16 & 255, $KEY_2 >> 8 & 255, $KEY_2 & 255];
						$KEY_2 = $KEY_420($KEY_2 ^ 3642107714);
						let $AT = $KEY_224($KEY_2, $AS[3], $KEY_0, 3);
						$AT = $KEY_224($KEY_2, $AS[0], $KEY_0, $AT);
						$AT = $KEY_224($KEY_2, $AS[1], $KEY_0, $AT);
						$AT = $KEY_224($KEY_2, $AS[4], $KEY_0, $AT);
						$KEY_224($KEY_2, $AS[2], $KEY_0, $AT);
						$KEY_8.next([[], $KEY_0]);
						$KEY_8.complete();
					}
					let $AR = { next: $THREAD_3_147216 };
					function $THREAD_7_5311($AS) {
						$KEY_8.error($AS);
					}
					$AR.error = $THREAD_7_5311;
					$AQ.subscribe($AR);
				});
				$AO.start();
			}, false);
			$AM(() => {
				let $KEY_2 = Math.random() * 4294967296 | 0;
				let $AO = [$KEY_2 >>> 24, $KEY_2 >> 16 & 255, $KEY_2 >> 8 & 255, $KEY_2 & 255];
				let $KEY_5 = $KEY_420($KEY_2 ^ 2474455138);
				$AO[$KEY_224($KEY_5, $KEY_4, $AO, 3) + 1] = ($KEY_0 ? 37 : 30) ^ $KEY_5();
				$KEY_13.next([[], $AO]);
			});
			$AN(() => {
				if ($KEY_16 != null) {
					$KEY_18.removeEventListener("input", $KEY_16, true);
				}
			});
		}