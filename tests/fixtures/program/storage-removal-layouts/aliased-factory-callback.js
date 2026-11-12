// Verbatim callback and its own resolved factories; source/input pins in artifacts.
function main(){
let $KEY_0 = null, $KEY_1 = null, $KEY_3 = null, $KEY_5 = null, $KEY_184 = null, $KEY_2 = null, $KEY_10 = null, $KEY_143 = null, $KEY_214 = null, $KEY_35 = null, $KEY_29 = null, $KEY_45 = null, $KEY_41 = null, $KEY_4 = null, $KEY_24 = null;
function $THREAD_8_53270($BQ) {
					let $KEY_0, $KEY_1, $KEY_3, $KEY_5;
					function $THREAD_0_90789() {
						let $BR = $KEY_0;
						$KEY_0 = ($BR + 1) % 4;
						if (($BR + 1) % 4 === 0) {
							$KEY_5 = $KEY_3 + $KEY_1;
							let $BS = $KEY_1;
							$KEY_1 = $BS ^ $KEY_3;
							$KEY_3 = $BS ^ $KEY_3 ^ ($KEY_3 << 26 | $KEY_3 >>> 6) ^ $KEY_1 << 9;
							$KEY_1 = $KEY_1 << 13 | $KEY_1 >>> 19;
							return $KEY_5 & 255;
						}
						if (($BR + 1) % 4 === 1) {
							return $KEY_5 >> 8 & 255;
						}
						if (($BR + 1) % 4 === 2) {
							return $KEY_5 >> 16 & 255;
						}
						if (($BR + 1) % 4 === 3) {
							return $KEY_5 >>> 24 & 255;
						} else {
							return void 0;
						}
					}
					$KEY_3 = $KEY_184;
					$KEY_1 = $BQ;
					$KEY_0 = -1;
					$THREAD_0_90789();
					$THREAD_0_90789();
					$THREAD_0_90789();
					$THREAD_0_90789();
					return $THREAD_0_90789;
				}
let $KEY_19 = $THREAD_8_53270;
function $THREAD_8_44347($BQ) {
					let $KEY_1 = $BQ;
					let $KEY_2;
					$KEY_2 = Object.create(null);
					let $KEY_0 = 0;
					function $THREAD_3_24252($BR, $BS, $BT) {
						let $KEY_10;
						if ($KEY_2[$BR] != null) {
							$KEY_10 = $BT;
							$KEY_10 = Number($BT) + 1;
							$BS[$KEY_10] = $KEY_1() ^ 129;
							$KEY_10 = $KEY_143($KEY_1, $KEY_2[$BR], $BS, $KEY_10);
						} else {
							let $BU = $BR.length;
							let $BV = 0;
							let $BW = $BT;
							while ($BV < $BU) {
								let $BX = $KEY_214($BR[$BV], 0);
								let $BY;
								if ($BX === 0) {
									$BS[++$BW] = $KEY_1() ^ 192;
									$BX = $KEY_1();
									$BY = 128;
								} else {
									if (!($BX < 128)) if ($BX < 2048) {
										$BS[++$BW] = ($BX >> 6 | 192) ^ $KEY_1();
										$BX = $BX & 63 | 128;
									} else {
										const $BZ = $BX >= 55296 && $BX <= 57343 && $BX < 56320 && $BV + 1 < $BU ? $KEY_214($BR[$BV + 1], 0) : null;
										if ($BZ == null || $BZ < 56320 || $BZ > 57343) {
											$BS[++$BW] = ($BX >> 12 | 224) ^ $KEY_1();
											$BS[++$BW] = ($BX >> 6 & 63 | 128) ^ $KEY_1();
											$BX = $BX & 63 | 128;
										} else {
											++$BV;
											$BS[++$BW] = (65536 + ($BX - 55296 << 10) + ($BZ - 56320) >> 18 | 240) ^ $KEY_1();
											$BS[++$BW] = (65536 + ($BX - 55296 << 10) + ($BZ - 56320) >> 12 & 63 | 128) ^ $KEY_1();
											$BS[++$BW] = (65536 + ($BX - 55296 << 10) + ($BZ - 56320) >> 6 & 63 | 128) ^ $KEY_1();
											$BX = 65536 + ($BX - 55296 << 10) + ($BZ - 56320) & 63 | 128;
										}
									}
									$BY = $KEY_1();
								}
								$BS[++$BW] = $BX ^ $BY;
								++$BV;
							}
							$KEY_10 = Number($BW) + 1;
							$BS[$KEY_10] = $KEY_1();
							let $CA = $KEY_2;
							let $CB = Number($KEY_0);
							$KEY_0 = $CB + 1;
							$CA[$BR] = $CB;
						}
						return $KEY_10;
					}
					return $THREAD_3_24252;
				}
let $KEY_454 = $THREAD_8_44347;
function $THREAD_6_11610($BW) {
						try {
							if ($BW.key === null || $BW.key === $KEY_35 && $BW.newValue === null) {
								$KEY_29 = true;
								$KEY_45 = $BW.oldValue ?? null;
								let $BX = Math.random() * 4294967296 | 0;
								let $BY = [$BX >>> 24, $BX >> 16 & 255, $BX >> 8 & 255, $BX & 255];
								$BX = $KEY_19($BX ^ 2873863633);
								let $BZ = $KEY_454($BX);
								let $CA = $KEY_41;
								let $CB = $KEY_45;
								if ($CB == null) {
									$BY[4] = $BX() ^ 130;
									$BZ = 4;
								} else {
									$BZ = $BZ($CB, $BY, 3);
								}
								$BY[$BZ + 1] = ($CA | 0 | $KEY_29 << 1 | $KEY_4 << 2) ^ $BX();
								$KEY_24.next([[], $BY]);
							}
						} catch (e) {
							$KEY_41 = true;
							let $CC = Math.random() * 4294967296;
							let $CD = [($CC | 0) >>> 24, ($CC | 0) >> 16 & 255, ($CC | 0) >> 8 & 255, ($CC | 0) & 255];
							$CC = $KEY_19(($CC | 0) ^ 2873863633);
							let $CE = $KEY_454($CC);
							let $CF = $KEY_41;
							let $CG = $KEY_45;
							let $CH;
							if ($CG == null) {
								$CD[4] = $CC() ^ 130;
								$CH = 4;
							} else {
								$CE = [$CE, $CG, $CD];
								$CH = $CE[0]($CE[1], $CE[2], 3);
							}
							$CD[$CH + 1] = ($CF | 0 | $KEY_29 << 1 | $KEY_4 << 2) ^ $CC();
							$KEY_24.next([[], $CD]);
						}
					}
let host = {addEventListener(){}};
host.addEventListener("storage", $THREAD_6_11610);
}
main();
