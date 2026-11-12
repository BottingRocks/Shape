// Verbatim callback and its own resolved factories; source/input pins in artifacts.
function main(){
let $KEY_1 = null, $KEY_2 = null, $KEY_3 = null, $KEY_4 = null, $KEY_395 = null, $KEY_0 = null, $KEY_8 = null, $KEY_111 = null, $KEY_393 = null, $KEY_7 = null, $KEY_20 = null, $KEY_9 = null, $KEY_16 = null, $KEY_13 = null, $KEY_5 = null;
function $THREAD_1_44288($BN) {
					let $KEY_1, $KEY_2, $KEY_3, $KEY_4;
					function $THREAD_1_60273() {
						let $BO = $KEY_2;
						$KEY_2 = ($BO + 1) % 4;
						if (($BO + 1) % 4 === 0) {
							$KEY_1 = $KEY_3 + $KEY_4;
							let $BP = $KEY_4;
							$KEY_4 = $BP ^ $KEY_3;
							$KEY_3 = $BP ^ $KEY_3 ^ ($KEY_3 << 26 | $KEY_3 >>> 6) ^ $KEY_4 << 9;
							$KEY_4 = $KEY_4 << 13 | $KEY_4 >>> 19;
							return $KEY_1 & 255;
						}
						if (($BO + 1) % 4 === 1) {
							return $KEY_1 >> 8 & 255;
						}
						if (($BO + 1) % 4 === 2) {
							return $KEY_1 >> 16 & 255;
						}
						if (($BO + 1) % 4 === 3) {
							return $KEY_1 >>> 24 & 255;
						} else {
							return void 0;
						}
					}
					$KEY_3 = $KEY_395;
					$KEY_4 = $BN;
					$KEY_2 = -1;
					$THREAD_1_60273();
					$THREAD_1_60273();
					$THREAD_1_60273();
					$THREAD_1_60273();
					return $THREAD_1_60273;
				}
let $KEY_241 = $THREAD_1_44288;
function $THREAD_3_71187($BN) {
					let $KEY_2 = $BN;
					let $KEY_1 = Object.create(null);
					let $KEY_0 = 0;
					function $THREAD_4_104240($BO, $BP, $BQ) {
						let $KEY_8;
						if ($KEY_1[$BO] != null) {
							$KEY_8 = $BQ;
							$KEY_8 = Number($BQ) + 1;
							$BP[$KEY_8] = $KEY_2() ^ 129;
							$KEY_8 = $KEY_111($KEY_2, $KEY_1[$BO], $BP, $KEY_8);
						} else {
							let $BR = $BO.length;
							let $BS = 0;
							let $BT = $BQ;
							while ($BS < $BR) {
								let $BU = $KEY_393($BO[$BS], 0);
								let $BV;
								if ($BU === 0) {
									$BP[++$BT] = $KEY_2() ^ 192;
									$BU = $KEY_2();
									$BV = 128;
								} else {
									if (!($BU < 128)) if ($BU < 2048) {
										$BP[++$BT] = ($BU >> 6 | 192) ^ $KEY_2();
										$BU = $BU & 63 | 128;
									} else {
										let $BW = $BU >= 55296 && $BU <= 57343 && $BU < 56320 && $BS + 1 < $BR ? $KEY_393($BO[$BS + 1], 0) : null;
										if ($BW == null || $BW < 56320 || $BW > 57343) {
											$BP[++$BT] = ($BU >> 12 | 224) ^ $KEY_2();
											$BP[++$BT] = ($BU >> 6 & 63 | 128) ^ $KEY_2();
											$BU = $BU & 63 | 128;
										} else {
											++$BS;
											$BP[++$BT] = (65536 + ($BU - 55296 << 10) + ($BW - 56320) >> 18 | 240) ^ $KEY_2();
											$BP[++$BT] = (65536 + ($BU - 55296 << 10) + ($BW - 56320) >> 12 & 63 | 128) ^ $KEY_2();
											$BP[++$BT] = (65536 + ($BU - 55296 << 10) + ($BW - 56320) >> 6 & 63 | 128) ^ $KEY_2();
											$BU = 65536 + ($BU - 55296 << 10) + ($BW - 56320) & 63 | 128;
										}
									}
									$BV = $KEY_2();
								}
								$BP[++$BT] = $BU ^ $BV;
								++$BS;
							}
							$KEY_8 = Number($BT) + 1;
							$BP[$KEY_8] = $KEY_2();
							let $BX = $KEY_1;
							let $BY = Number($KEY_0);
							$KEY_0 = $BY + 1;
							$BX[$BO] = $BY;
						}
						return $KEY_8;
					}
					return $THREAD_4_104240;
				}
let $KEY_122 = $THREAD_3_71187;
function $THREAD_3_98944($BT) {
						try {
							if ($BT.key === null || $BT.key === $KEY_7 && $BT.newValue === null) {
								$KEY_20 = true;
								$KEY_9 = $BT.oldValue ?? null;
								let $BU = Math.random() * 4294967296 | 0;
								let $BV = [$BU >>> 24, $BU >> 16 & 255, $BU >> 8 & 255, $BU & 255];
								$BU = $KEY_241($BU ^ 721228179);
								let $BW = $KEY_122($BU);
								let $BX = $KEY_9;
								let $BY;
								if ($BX == null) {
									$BV[4] = $BU() ^ 130;
									$BY = 4;
								} else {
									$BW = [$BW, $BX, $BV];
									$BY = $BW[0].call(void 0, $BW[1], $BW[2], 3);
								}
								$BV[$BY + 1] = ($KEY_20 | 0 | $KEY_16 << 1 | $KEY_13 << 2) ^ $BU();
								$KEY_5.next([[], $BV]);
							}
						} catch (e) {
							$KEY_16 = true;
							let $BZ = Math.random() * 4294967296;
							let $CA = [($BZ | 0) >>> 24, ($BZ | 0) >> 16 & 255, ($BZ | 0) >> 8 & 255, ($BZ | 0) & 255];
							$BZ = $KEY_241(($BZ | 0) ^ 721228179);
							let $CB = $KEY_122($BZ);
							let $CC = $KEY_9;
							if ($CC == null) {
								$CA[4] = $BZ() ^ 130;
								$CB = 4;
							} else {
								$CB = $CB($CC, $CA, 3);
							}
							$CA[$CB + 1] = ($KEY_20 | 0 | $KEY_16 << 1 | $KEY_13 << 2) ^ $BZ();
							$KEY_5.next([[], $CA]);
						}
					}
let host = {addEventListener(){}};
host.addEventListener("storage", $THREAD_3_98944);
}
main();
