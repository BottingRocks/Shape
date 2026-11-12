// Verbatim callback and its own resolved factories; source/input pins in artifacts.
function main(){
let $KEY_0 = null, $KEY_2 = null, $KEY_3 = null, $KEY_5 = null, $KEY_328 = null, $KEY_1 = null, $KEY_11 = null, $KEY_61 = null, $KEY_306 = null, $KEY_25 = null, $KEY_55 = null, $KEY_8 = null, $KEY_43 = null, $KEY_37 = null, $KEY_35 = null;
function $THREAD_1_115090($BR) {
					let $KEY_0, $KEY_2, $KEY_3, $KEY_5;
					function $THREAD_5_98843() {
						let $BS = $KEY_5;
						$KEY_5 = ($BS + 1) % 4;
						if (($BS + 1) % 4 === 0) {
							$KEY_0 = $KEY_3 + $KEY_2;
							let $BT = $KEY_2;
							$KEY_2 = $BT ^ $KEY_3;
							$KEY_3 = $BT ^ $KEY_3 ^ ($KEY_3 << 26 | $KEY_3 >>> 6) ^ $KEY_2 << 9;
							$KEY_2 = $KEY_2 << 13 | $KEY_2 >>> 19;
							return $KEY_0 & 255;
						}
						if (($BS + 1) % 4 === 1) {
							return $KEY_0 >> 8 & 255;
						}
						if (($BS + 1) % 4 === 2) {
							return $KEY_0 >> 16 & 255;
						}
						if (($BS + 1) % 4 === 3) {
							return $KEY_0 >>> 24 & 255;
						} else {
							return void 0;
						}
					}
					$KEY_3 = $KEY_328;
					$KEY_2 = $BR;
					$KEY_5 = -1;
					$THREAD_5_98843();
					$THREAD_5_98843();
					$THREAD_5_98843();
					$THREAD_5_98843();
					return $THREAD_5_98843;
				}
let $KEY_342 = $THREAD_1_115090;
function $THREAD_9_71496($BR) {
					let $KEY_0 = $BR;
					let $KEY_1, $KEY_2;
					$KEY_1 = Object.create(null);
					$KEY_2 = 0;
					function $THREAD_4_28910($BS, $BT, $BU) {
						let $KEY_11;
						if ($KEY_1[$BS] != null) {
							$KEY_11 = $BU;
							$KEY_11 = Number($BU) + 1;
							$BT[$KEY_11] = $KEY_0() ^ 129;
							$KEY_11 = $KEY_61($KEY_0, $KEY_1[$BS], $BT, $KEY_11);
						} else {
							let $BV = $BS.length;
							let $BW = 0;
							let $BX = $BU;
							while ($BW < $BV) {
								let $BY = $KEY_306($BS[$BW], 0);
								let $BZ;
								if ($BY === 0) {
									$BT[++$BX] = $KEY_0() ^ 192;
									$BY = $KEY_0();
									$BZ = 128;
								} else {
									if (!($BY < 128)) if ($BY < 2048) {
										$BT[++$BX] = ($BY >> 6 | 192) ^ $KEY_0();
										$BY = $BY & 63 | 128;
									} else {
										const $CA = $BY >= 55296 && $BY <= 57343 && $BY < 56320 && $BW + 1 < $BV ? $KEY_306($BS[$BW + 1], 0) : null;
										if ($CA == null || $CA < 56320 || $CA > 57343) {
											$BT[++$BX] = ($BY >> 12 | 224) ^ $KEY_0();
											$BT[++$BX] = ($BY >> 6 & 63 | 128) ^ $KEY_0();
											$BY = $BY & 63 | 128;
										} else {
											++$BW;
											$BT[++$BX] = (65536 + ($BY - 55296 << 10) + ($CA - 56320) >> 18 | 240) ^ $KEY_0();
											$BT[++$BX] = (65536 + ($BY - 55296 << 10) + ($CA - 56320) >> 12 & 63 | 128) ^ $KEY_0();
											$BT[++$BX] = (65536 + ($BY - 55296 << 10) + ($CA - 56320) >> 6 & 63 | 128) ^ $KEY_0();
											$BY = 65536 + ($BY - 55296 << 10) + ($CA - 56320) & 63 | 128;
										}
									}
									$BZ = $KEY_0();
								}
								$BT[++$BX] = $BY ^ $BZ;
								++$BW;
							}
							$KEY_11 = Number($BX) + 1;
							$BT[$KEY_11] = $KEY_0();
							let $CB = $KEY_1;
							let $CC = Number($KEY_2);
							$KEY_2 = $CC + 1;
							$CB[$BS] = $CC;
						}
						return $KEY_11;
					}
					return $THREAD_4_28910;
				}
let $KEY_426 = $THREAD_9_71496;
function $THREAD_9_91527($BX) {
						try {
							if ($BX.key === null || $BX.key === $KEY_25 && $BX.newValue === null) {
								$KEY_55 = true;
								$KEY_8 = $BX.oldValue ?? null;
								let $BY = Math.random() * 4294967296;
								let $BZ = [($BY | 0) >>> 24, ($BY | 0) >> 16 & 255, ($BY | 0) >> 8 & 255, ($BY | 0) & 255];
								$BY = $KEY_342(($BY | 0) ^ 2554166702);
								let $CA = $KEY_426($BY);
								$BZ[4] = ($KEY_43 | 0 | $KEY_55 << 1 | $KEY_37 << 2) ^ $BY();
								let $CB = $KEY_8;
								if ($CB == null) {
									$BZ[5] = $BY() ^ 130;
								} else {
									$CA = [$CA, $CB, $BZ];
									$CA[0].call(void 0, $CA[1], $CA[2], 4);
								}
								$KEY_35.next([[], $BZ]);
							}
						} catch (e) {
							$KEY_37 = true;
							let $CC = Math.random() * 4294967296 | 0;
							let $CD = [$CC >>> 24, $CC >> 16 & 255, $CC >> 8 & 255, $CC & 255];
							$CC = $KEY_342($CC ^ 2554166702);
							let $CE = $KEY_426($CC);
							$CD[4] = ($KEY_43 | 0 | $KEY_55 << 1 | $KEY_37 << 2) ^ $CC();
							let $CF = $KEY_8;
							if ($CF == null) {
								$CD[5] = $CC() ^ 130;
							} else {
								$CE = [$CE, $CF, $CD];
								$CE[0]($CE[1], $CE[2], 4);
							}
							$KEY_35.next([[], $CD]);
						}
					}
let host = {addEventListener(){}};
host.addEventListener("storage", $THREAD_9_91527);
}
main();
