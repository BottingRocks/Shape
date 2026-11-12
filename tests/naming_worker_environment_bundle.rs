//! Focused, portable coverage for worker-posted seeded environment bundles.
//!
//! Positives retain complete container bodies and alpha spellings. Negatives change
//! operations, receivers, dataflow, result sinks, and owned donors and must remain
//! unclaimed. The worker source is never run; exact UTF-8 bodies are analyzed statically.

use shape::naming::name_threads;

const EVALUATOR: &str = "worker_environment_bundle";
const EMITTER: &str = "emitSeededWorkerEnvironmentBundle";
const CONTAINER: &str = "createSeededWorkerEnvironmentBundle";

const POS_GUARDED_CONTAINER: &str = r##"function $THREAD_3_15278($A, $B) {
	let $KEY_4 = $A;
	let $KEY_6, $KEY_7, $KEY_8, $KEY_15, $KEY_16, $KEY_18;
	function $THREAD_8_43624($E) {
		let $KEY_0 = $E;
		let $KEY_1, $KEY_2;
		$KEY_2 = Object.create(null);
		$KEY_1 = 0;
		function $THREAD_8_43802($F, $G, $H) {
			let $KEY_3;
			if ($KEY_2[$F] != null) {
				$KEY_3 = $H;
				$KEY_3 = Number($H) + 1;
				$G[$KEY_3] = $KEY_0() ^ 129;
				$KEY_3 = $KEY_18($KEY_0, $KEY_2[$F], $G, $KEY_3);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_6($F[$J], 0);
					let $M;
					if ($L === 0) {
						$G[++$K] = $KEY_0() ^ 192;
						$L = $KEY_0();
						$M = 128;
					} else {
						if (!($L < 128)) if ($L < 2048) {
							$G[++$K] = ($L >> 6 | 192) ^ $KEY_0();
							$L = $L & 63 | 128;
						} else {
							const $N = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_6($F[$J + 1], 0) : null;
							if ($N == null || $N < 56320 || $N > 57343) {
								$G[++$K] = ($L >> 12 | 224) ^ $KEY_0();
								$G[++$K] = ($L >> 6 & 63 | 128) ^ $KEY_0();
								$L = $L & 63 | 128;
							} else {
								++$J;
								$G[++$K] = (65536 + ($L - 55296 << 10) + ($N - 56320) >> 18 | 240) ^ $KEY_0();
								$G[++$K] = (65536 + ($L - 55296 << 10) + ($N - 56320) >> 12 & 63 | 128) ^ $KEY_0();
								$G[++$K] = (65536 + ($L - 55296 << 10) + ($N - 56320) >> 6 & 63 | 128) ^ $KEY_0();
								$L = 65536 + ($L - 55296 << 10) + ($N - 56320) & 63 | 128;
							}
						}
						$M = $KEY_0();
					}
					$G[++$K] = $L ^ $M;
					++$J;
				}
				$KEY_3 = Number($K) + 1;
				$G[$KEY_3] = $KEY_0();
				let $O = $KEY_2;
				let $P = Number($KEY_1);
				$KEY_1 = $P + 1;
				$O[$F] = $P;
			}
			return $KEY_3;
		}
		return $THREAD_8_43802;
	}
	$KEY_8 = $THREAD_8_43624;
	let $KEY_0, $KEY_1;
	function $THREAD_3_114337($E, $F, $G, $H) {
		let $KEY_5;
		if (($F | 0) === $F) {
			let $I;
			if ($F < 0) {
				$I = $F < 0;
			} else if ($F === 0) {
				$I = 1 / $F < 0;
			} else {
				$I = false;
			}
			let $J, $K;
			if ($I) {
				$J = -$F;
				$K = 64;
			} else {
				$J = $F;
				$K = 0;
			}
			if ($J < 32) {
				$KEY_5 = Number($H) + 1;
				$G[$KEY_5] = ($K | $J) ^ $E();
			} else if ($J < 4096) {
				$KEY_5 = Number($H) + 1;
				$G[$KEY_5] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_5] = $J >> 5 ^ $E();
			} else if ($J < 524288) {
				$KEY_5 = Number($H) + 1;
				$G[$KEY_5] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_5] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_5] = $J >>> 12 ^ $E();
			} else if ($J < 67108864) {
				$KEY_5 = Number($H) + 1;
				$G[$KEY_5] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_5] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_5] = ($J >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_5] = $J >>> 19 ^ $E();
			} else {
				$KEY_5 = Number($H) + 1;
				$G[$KEY_5] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_5] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_5] = ($J >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_5] = ($J >>> 19 & 127 | 128) ^ $E();
				$G[++$KEY_5] = $J >>> 26 ^ $E();
			}
		} else {
			$KEY_5 = $H;
			$KEY_5 = Number($H) + 1;
			$G[$KEY_5] = $E() ^ 128;
			let $L;
			if ($KEY_1) {
				$KEY_1[0] = $F;
				$L = new $KEY_7($KEY_1.buffer);
			} else if ($F !== $F) {
				$L = $KEY_0;
			} else {
				let $KEY_13 = $F < 0 || $F === 0 && 1 / $F < 0;
				let $M = Math.abs($F);
				let $N, $KEY_15;
				if ($M >= Math.pow(2, -1022)) {
					let $KEY_6 = Math.min(Math.floor(Math.log($M) / Math.LN2), 1023);
					$M = $M / Math.pow(2, $KEY_6);
					let $O = $M < 1;
					$KEY_6 = $O ? $KEY_6 - 1 : $KEY_6;
					$O = $O ? $M * 2 : $M;
					let $P = $O >= 2;
					$O = $P ? $O / 2 : $O;
					$KEY_15 = Math.pow(2, 52);
					let $KEY_10 = Math.floor($O * $KEY_15);
					$P = ($P ? $KEY_6 + 1 : $KEY_6) + 1023;
					let $Q = (($O * $KEY_15 - $KEY_10 < .5 ? $KEY_10 : $O * $KEY_15 - $KEY_10 > .5 ? $KEY_10 + 1 : $KEY_10 % 2 ? $KEY_10 + 1 : $KEY_10) - $KEY_15) / $KEY_15 >= 1;
					$N = $Q ? $P + 1 : $P;
					if ($N > 2046) {
						$N = 2047;
						$KEY_15 = 0;
					} else {
						$KEY_15 = $Q ? 0 : ($O * $KEY_15 - $KEY_10 < .5 ? $KEY_10 : $O * $KEY_15 - $KEY_10 > .5 ? $KEY_10 + 1 : $KEY_10 % 2 ? $KEY_10 + 1 : $KEY_10) - $KEY_15;
					}
				} else {
					$M = $M / Math.pow(2, -1074);
					let $R = Math.floor($M);
					$N = 0;
					$KEY_15 = $M - $R < .5 ? $R : $M - $R > .5 ? $R + 1 : $R % 2 ? $R + 1 : $R;
				}
				let $S = [];
				let k = 51;
				for (; k >= 0; k--) {
					$S.unshift(!!($KEY_15 & 1));
					$KEY_15 = Math.floor($KEY_15 / 2);
				}
				let i = 11;
				for (; i > 0; i--) {
					$S.unshift(!!($N & 1));
					$N = Math.floor($N / 2);
				}
				$S.unshift($KEY_13);
				$L = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					$L.push(Number($S[j]) << 7 | Number($S[j + 1]) << 6 | Number($S[j + 2]) << 5 | Number($S[j + 3]) << 4 | Number($S[j + 4]) << 3 | Number($S[j + 5]) << 2 | Number($S[j + 6]) << 1 | Number($S[j + 7]));
				}
			}
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$G[++$KEY_5] = $L[i_2] ^ $E();
			}
		}
		return $KEY_5;
	}
	$KEY_18 = $THREAD_3_114337;
	function $THREAD_0_122499($E) {
		let $KEY_0, $KEY_1, $KEY_3, $KEY_4;
		function $THREAD_9_104695() {
			let $F = $KEY_3;
			$KEY_3 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_0 = $KEY_4 + $KEY_1;
				let $G = $KEY_1;
				$KEY_1 = $G ^ $KEY_4;
				$KEY_4 = $G ^ $KEY_4 ^ ($KEY_4 << 26 | $KEY_4 >>> 6) ^ $KEY_1 << 9;
				$KEY_1 = $KEY_1 << 13 | $KEY_1 >>> 19;
				return $KEY_0 & 255;
			}
			if (($F + 1) % 4 === 1) {
				return $KEY_0 >> 8 & 255;
			}
			if (($F + 1) % 4 === 2) {
				return $KEY_0 >> 16 & 255;
			}
			if (($F + 1) % 4 === 3) {
				return $KEY_0 >>> 24 & 255;
			} else {
				return void 0;
			}
		}
		$KEY_4 = $KEY_16;
		$KEY_1 = $E;
		$KEY_3 = -1;
		$THREAD_9_104695();
		$THREAD_9_104695();
		$THREAD_9_104695();
		$THREAD_9_104695();
		return $THREAD_9_104695;
	}
	$KEY_15 = $THREAD_0_122499;
	Object.prototype.hasOwnProperty;
	$KEY_6 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_7 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_1 = $KEY_7 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_0 = $KEY_1 ? null : (($PARAMA, $PARAMB) => {
		$PARAMA[$PARAMB] = 248;
		$PARAMA[7] = 127;
		return $PARAMA;
	})(([0, 0, 0, 0, 0, 0]), 6);
	let $C = $B.a.length >> 1 << 1;
	let i = 0;
	let $D = 0;
	for (; i < $C; i = i + 2) {
		$D = ($D << 5) - $D + ($B.a[i].charCodeAt() << 16 | $B.a[i + 1].charCodeAt()) | 0;
	}
	$KEY_16 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_4_66003() {
		let $KEY_1;
		function $THREAD_2_26830($L) {
			if ($L == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($L).length + $KEY_1(Object.getPrototypeOf($L));
			}
		}
		$KEY_1 = $THREAD_2_26830;
		let $E = Math.random() * 4294967296;
		let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];
		let $KEY_9 = $KEY_15(($E | 0) ^ 1637684726);
		let $KEY_0 = $KEY_8($KEY_9);
		let $G = $KEY_0(location.origin, $F, 3);
		$G = $KEY_18($KEY_9, $KEY_1(self), $F, $G);
		$G = $KEY_0(navigator.userAgent, $F, $G);
		let $H = navigator.hardwareConcurrency;
		if ($H == null) {
			$F[++$G] = $KEY_9() ^ 130;
		} else {
			$G = $KEY_18($KEY_9, $H, $F, $G);
		}
		$KEY_0(navigator.platform, $F, $G);
		let $I = postMessage;
		let $J = { a: $KEY_4 };
		let $K = [[], $F];
		$J.b = $K;
		$I($J);
	}
	$THREAD_4_66003($B.b);
}"##;
const POS_DIRECT_CONTAINER: &str = r##"function $THREAD_8_78169($A, $B) {
	let $KEY_14 = $A;
	let $KEY_15, $KEY_17, $KEY_18, $KEY_1, $KEY_2;
	function $THREAD_4_31351($E, $F, $G, $H) {
		let $KEY_3;
		if (($F | 0) === $F) {
			let $I;
			if ($F < 0) {
				$I = $F < 0;
			} else if ($F === 0) {
				$I = 1 / $F < 0;
			} else {
				$I = false;
			}
			let $J, $K;
			if ($I) {
				$J = -$F;
				$K = 64;
			} else {
				$J = $F;
				$K = 0;
			}
			if ($J < 32) {
				$KEY_3 = Number($H) + 1;
				$G[$KEY_3] = ($K | $J) ^ $E();
			} else if ($J < 4096) {
				$KEY_3 = Number($H) + 1;
				$G[$KEY_3] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_3] = $J >> 5 ^ $E();
			} else if ($J < 524288) {
				$KEY_3 = Number($H) + 1;
				$G[$KEY_3] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_3] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_3] = $J >>> 12 ^ $E();
			} else if ($J < 67108864) {
				$KEY_3 = Number($H) + 1;
				$G[$KEY_3] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_3] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_3] = ($J >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_3] = $J >>> 19 ^ $E();
			} else {
				$KEY_3 = Number($H) + 1;
				$G[$KEY_3] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_3] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_3] = ($J >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_3] = ($J >>> 19 & 127 | 128) ^ $E();
				$G[++$KEY_3] = $J >>> 26 ^ $E();
			}
		} else {
			$KEY_3 = $H;
			$KEY_3 = Number($H) + 1;
			$G[$KEY_3] = $E() ^ 128;
			let $L;
			if ($KEY_1) {
				$KEY_1[0] = $F;
				$L = new $KEY_2($KEY_1.buffer);
			} else if ($F !== $F) {
				$L = $KEY_15;
			} else {
				let $KEY_6 = $F < 0 || $F === 0 && 1 / $F < 0;
				let $M = Math.abs($F);
				let $N;
				if ($M >= Math.pow(2, -1022)) {
					let $O = Math.min(Math.floor(Math.log($M) / Math.LN2), 1023);
					$M = $M / Math.pow(2, $O);
					let $P = $M < 1;
					$O = $P ? $O - 1 : $O;
					$P = $P ? $M * 2 : $M;
					let $Q = $P >= 2;
					$P = $Q ? $P / 2 : $P;
					let $KEY_11 = Math.pow(2, 52);
					let $R = Math.floor($P * $KEY_11);
					$P = $P * $KEY_11 - $R < .5 ? $R : $P * $KEY_11 - $R > .5 ? $R + 1 : $R % 2 ? $R + 1 : $R;
					$Q = ($Q ? $O + 1 : $O) + 1023;
					let $S = ($P - $KEY_11) / $KEY_11 >= 1;
					$N = $S ? $Q + 1 : $Q;
					if ($N > 2046) {
						$N = 2047;
						$M = 0;
					} else {
						$M = $S ? 0 : $P - $KEY_11;
					}
				} else {
					$M = $M / Math.pow(2, -1074);
					let $T = Math.floor($M);
					$N = 0;
					$M = $M - $T < .5 ? $T : $M - $T > .5 ? $T + 1 : $T % 2 ? $T + 1 : $T;
				}
				let $U = [];
				let k = 51;
				for (; k >= 0; k--) {
					$U.unshift(!!($M & 1));
					$M = Math.floor($M / 2);
				}
				let i = 11;
				for (; i > 0; i--) {
					$U.unshift(!!($N & 1));
					$N = Math.floor($N / 2);
				}
				$U.unshift($KEY_6);
				$L = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					let $V = [$L, $L.push];
					$V[1].call($V[0], Number($U[j]) << 7 | Number($U[j + 1]) << 6 | Number($U[j + 2]) << 5 | Number($U[j + 3]) << 4 | Number($U[j + 4]) << 3 | Number($U[j + 5]) << 2 | Number($U[j + 6]) << 1 | Number($U[j + 7]));
				}
			}
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$G[++$KEY_3] = $L[i_2] ^ $E();
			}
		}
		return $KEY_3;
	}
	let $KEY_10;
	function $THREAD_6_62913($E) {
		let $KEY_0 = $E;
		let $KEY_1, $KEY_2;
		$KEY_2 = Object.create(null);
		$KEY_1 = 0;
		function $THREAD_2_45381($F, $G, $H) {
			let $KEY_14;
			if ($KEY_2[$F] != null) {
				$KEY_14 = $H;
				$KEY_14 = Number($H) + 1;
				$G[$KEY_14] = $KEY_0() ^ 129;
				$KEY_14 = $KEY_10($KEY_0, $KEY_2[$F], $G, $KEY_14);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_17($F[$J], 0);
					let $M;
					if ($L === 0) {
						$G[++$K] = $KEY_0() ^ 192;
						$L = $KEY_0();
						$M = 128;
					} else {
						if (!($L < 128)) if ($L < 2048) {
							$G[++$K] = ($L >> 6 | 192) ^ $KEY_0();
							$L = $L & 63 | 128;
						} else {
							const $N = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_17($F[$J + 1], 0) : null;
							if ($N == null || $N < 56320 || $N > 57343) {
								$G[++$K] = ($L >> 12 | 224) ^ $KEY_0();
								$G[++$K] = ($L >> 6 & 63 | 128) ^ $KEY_0();
								$L = $L & 63 | 128;
							} else {
								++$J;
								$G[++$K] = (65536 + ($L - 55296 << 10) + ($N - 56320) >> 18 | 240) ^ $KEY_0();
								$G[++$K] = (65536 + ($L - 55296 << 10) + ($N - 56320) >> 12 & 63 | 128) ^ $KEY_0();
								$G[++$K] = (65536 + ($L - 55296 << 10) + ($N - 56320) >> 6 & 63 | 128) ^ $KEY_0();
								$L = 65536 + ($L - 55296 << 10) + ($N - 56320) & 63 | 128;
							}
						}
						$M = $KEY_0();
					}
					$G[++$K] = $L ^ $M;
					++$J;
				}
				$KEY_14 = Number($K) + 1;
				$G[$KEY_14] = $KEY_0();
				let $O = $KEY_2;
				let $P = Number($KEY_1);
				$KEY_1 = $P + 1;
				$O[$F] = $P;
			}
			return $KEY_14;
		}
		return $THREAD_2_45381;
	}
	$KEY_18 = $THREAD_6_62913;
	$KEY_10 = $THREAD_4_31351;
	let $KEY_6;
	function $THREAD_7_87393($E) {
		let $KEY_0, $KEY_2, $KEY_3, $KEY_5;
		function $THREAD_7_13264() {
			let $F = $KEY_2;
			$KEY_2 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_3 = $KEY_0 + $KEY_5;
				let $G = $KEY_5;
				$KEY_5 = $G ^ $KEY_0;
				$KEY_0 = $G ^ $KEY_0 ^ ($KEY_0 << 26 | $KEY_0 >>> 6) ^ $KEY_5 << 9;
				$KEY_5 = $KEY_5 << 13 | $KEY_5 >>> 19;
				return $KEY_3 & 255;
			}
			if (($F + 1) % 4 === 1) {
				return $KEY_3 >> 8 & 255;
			}
			if (($F + 1) % 4 === 2) {
				return $KEY_3 >> 16 & 255;
			}
			if (($F + 1) % 4 === 3) {
				return $KEY_3 >>> 24 & 255;
			} else {
				return void 0;
			}
		}
		$KEY_0 = $KEY_6;
		$KEY_5 = $E;
		$KEY_2 = -1;
		$THREAD_7_13264();
		$THREAD_7_13264();
		$THREAD_7_13264();
		$THREAD_7_13264();
		return $THREAD_7_13264;
	}
	let $KEY_11 = $THREAD_7_87393;
	Object.prototype.hasOwnProperty;
	$KEY_17 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_2 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_1 = $KEY_2 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_15 = $KEY_1 ? null : (($PARAMA, $PARAMB) => {
		$PARAMA[$PARAMB] = 248;
		$PARAMA[7] = 127;
		return $PARAMA;
	})(([0, 0, 0, 0, 0, 0]), 6);
	let $C = $B.a.length >> 1 << 1;
	let i = 0;
	let $D = 0;
	for (; i < $C; i = i + 2) {
		$D = ($D << 5) - $D + ($B.a[i].charCodeAt() << 16 | $B.a[i + 1].charCodeAt()) | 0;
	}
	$KEY_6 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_2_101314() {
		let $KEY_1;
		function $THREAD_9_116277($M) {
			if ($M == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($M).length + $KEY_1(Object.getPrototypeOf($M));
			}
		}
		$KEY_1 = $THREAD_9_116277;
		let $E = Math.random() * 4294967296;
		let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];
		let $KEY_4 = $KEY_11(($E | 0) ^ 1642589177);
		let $G = $KEY_18($KEY_4);
		let $H = $G(navigator.userAgent, $F, 3);
		$H = $G(location.origin, $F, $H);
		$G = $G(navigator.platform, $F, $H);
		$G = $KEY_10($KEY_4, $KEY_1(self), $F, $G);
		let $I = navigator.hardwareConcurrency;
		if ($I == null) {
			$F[$G + 1] = $KEY_4() ^ 130;
		} else {
			$KEY_10($KEY_4, $I, $F, $G);
		}
		let $J = postMessage;
		let $K = { a: $KEY_14 };
		let $L = [[], $F];
		$K.b = $L;
		$J($K);
	}
	$THREAD_2_101314($B.b);
}"##;
const COUNT_HELPER: &str = r##"function $THREAD_2_26830($L) {
			if ($L == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($L).length + $THREAD_2_26830(Object.getPrototypeOf($L));
			}
		}"##;

fn carrier(body: &str) -> String {
    format!("function main() {{ return 1; }}\nmain();\n{body}")
}

fn alpha_guarded_container() -> String {
    POS_GUARDED_CONTAINER
        .replace("$THREAD_3_15278", "$THREAD_7_90001")
        .replace("$THREAD_4_66003", "$THREAD_8_90002")
        .replace("$THREAD_8_43624", "$THREAD_11_11001")
        .replace("$THREAD_8_43802", "$THREAD_12_11002")
        .replace("$THREAD_3_114337", "$THREAD_13_11003")
        .replace("$THREAD_0_122499", "$THREAD_14_11004")
        .replace("$THREAD_9_104695", "$THREAD_15_11005")
        .replace("$THREAD_2_26830", "$THREAD_16_11006")
        .replace("$KEY_", "$LOCAL_")
}

fn applied_names(source: &str, role: &str) -> Vec<String> {
    let mut names: Vec<String> = name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && outcome.role_name.as_deref() == Some(role)
                && outcome.evaluator.as_deref() == Some(EVALUATOR)
        })
        .filter_map(|outcome| outcome.proposed_name.clone())
        .collect();
    names.sort();
    names
}

fn claims(source: &str, role: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|entry| {
            entry.candidates.iter().any(|candidate| candidate == role)
                && entry.evaluators.iter().any(|evaluator| evaluator == EVALUATOR)
        })
        .count()
}

fn has_applied(source: &str, role: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.applied && outcome.role_name.as_deref() == Some(role))
}

fn assert_refused(source: &str) {
    assert_eq!(claims(source, CONTAINER), 0, "{source}");
    assert_eq!(claims(source, EMITTER), 0, "{source}");
    assert!(applied_names(source, CONTAINER).is_empty(), "{source}");
    assert!(applied_names(source, EMITTER).is_empty(), "{source}");
}

#[test]
fn actual_guarded_container_bundle_names_both_live_roles_and_preserves_helpers() {
    let source = carrier(POS_GUARDED_CONTAINER);
    assert_eq!(claims(&source, CONTAINER), 1);
    assert_eq!(claims(&source, EMITTER), 1);
    assert_eq!(applied_names(&source, CONTAINER), vec![CONTAINER.to_string()]);
    assert_eq!(applied_names(&source, EMITTER), vec![EMITTER.to_string()]);
    let renamed = name_threads(&source, true).unwrap();
    assert!(renamed.source.contains(&format!("function {CONTAINER}(")));
    assert!(renamed.source.contains(&format!("function {EMITTER}(")));
    assert!(!renamed.source.contains("function $THREAD_3_15278("));
    assert!(!renamed.source.contains("function $THREAD_4_66003("));
    for helper in [
        "writeNumber",
        "createInternedStringWriter",
        "writeInternedString",
        "createKeystreamByteSource",
        "nextKeystreamByte",
        "countOwnPropertiesAlongPrototypeChain",
    ] {
        assert!(has_applied(&source, helper), "missing preserved helper {helper}");
    }
}

#[test]
fn actual_direct_container_and_alpha_variants_are_claimed() {
    for source in [carrier(POS_DIRECT_CONTAINER), carrier(&alpha_guarded_container())] {
        assert_eq!(claims(&source, CONTAINER), 1, "{source}");
        assert_eq!(claims(&source, EMITTER), 1, "{source}");
        assert_eq!(
            applied_names(&source, CONTAINER),
            vec![CONTAINER.to_string()],
            "{source}"
        );
        assert_eq!(
            applied_names(&source, EMITTER),
            vec![EMITTER.to_string()],
            "{source}"
        );
    }
}

#[test]
fn result_sink_and_environment_receiver_negatives_are_refused() {
    for source in [
        POS_GUARDED_CONTAINER.replace("postMessage", "window.postMessage"),
        POS_GUARDED_CONTAINER.replace("navigator.hardwareConcurrency", "navigator.platform"),
        POS_GUARDED_CONTAINER.replace("location.origin", "document.URL"),
        POS_GUARDED_CONTAINER.replace("$KEY_1(self)", "$KEY_1(window)"),
        POS_GUARDED_CONTAINER.replace(
            "$F[++$G] = $KEY_9() ^ 130;",
            "$F[++$G] = $KEY_9() + 130;",
        ),
        POS_GUARDED_CONTAINER.replace("let $K = [[], $F];", "let $K = [[], []];"),
    ] {
        assert_refused(&carrier(&source));
    }
}

#[test]
fn bundle_dataflow_and_owned_donor_negatives_are_refused() {
    for source in [
        POS_GUARDED_CONTAINER.replace(
            "$KEY_16 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;",
            "$KEY_16 = 12345;",
        ),
        POS_GUARDED_CONTAINER.replace("$THREAD_4_66003($B.b);", "$THREAD_4_66003;"),
        POS_GUARDED_CONTAINER.replace("let $J = { a: $KEY_4 };", "let $J = { a: $KEY_6 };"),
        POS_GUARDED_CONTAINER.replace("$KEY_18 = $THREAD_3_114337;", "$KEY_18 = 0;"),
        POS_GUARDED_CONTAINER.replace("$KEY_15 = $THREAD_0_122499;", "$KEY_15 = 0;"),
        POS_GUARDED_CONTAINER.replace(
            "function $THREAD_3_15278($A, $B) {",
            "function $THREAD_3_15278($A, $B) { function $THREAD_99_99999() { return 0; }",
        ),
        POS_GUARDED_CONTAINER.replace(
            "function $THREAD_4_66003() {",
            "function $THREAD_4_66003() { function $THREAD_98_99998() { return 0; }",
        ),
        POS_GUARDED_CONTAINER.to_string() + "\n$THREAD_3_15278(null, null);",
    ] {
        assert_refused(&carrier(&source));
    }
}

#[test]
fn real_count_helper_neighbor_keeps_its_existing_name_and_is_not_bundled() {
    let source = carrier(COUNT_HELPER);
    assert!(has_applied(&source, "countOwnPropertiesAlongPrototypeChain"), "{source}");
    assert_refused(&source);
}

// The integration worker runs these parser and evaluator checks. Keep target
// presence explicit: malformed input or a missing candidate is never a refusal.
fn iso_refused(body: &str) {
    let source = carrier(body);
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &source,
        oxc_span::SourceType::unambiguous(),
    ).parse();
    assert!(!parsed.panicked && parsed.errors.is_empty(), "invalid fixture: {:?}", parsed.errors);
    let result = name_threads(&source, true).expect("static fixture analysis");
    for (target, role) in [
        ("$THREAD_3_15278", CONTAINER),
        ("$THREAD_4_66003", EMITTER),
    ] {
        let outcome = result.report.outcomes.iter()
            .find(|outcome| outcome.binding.name == target)
            .unwrap_or_else(|| panic!("missing target {target}"));
        assert_ne!(outcome.evaluator.as_deref(), Some(EVALUATOR), "{target}: {body}");
        assert!(!(outcome.applied && outcome.role_name.as_deref() == Some(role)), "{target}: {body}");
        let entry = result.report.function_roles.iter()
            .find(|entry| entry.current_name.as_deref() == Some(target))
            .unwrap_or_else(|| panic!("missing role entry {target}"));
        assert!(!entry.evaluators.iter().any(|evaluator| evaluator == EVALUATOR), "{target}: {body}");
    }
    assert_refused(&source);
}

fn iso_mutant(from: &str, to: &str) -> String {
    assert_eq!(POS_GUARDED_CONTAINER.matches(from).count(), 1, "ambiguous/missing mutation {from}");
    let mutant = POS_GUARDED_CONTAINER.replacen(from, to, 1);
    assert_ne!(mutant, POS_GUARDED_CONTAINER);
    mutant
}

const ISO_HEADER: &str = "let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];";

#[test]
fn nonce_header_exact_reported_constant_header_and_wrong_lanes_are_refused() {
    for header in [
        "let $F = [0 >> 1, 0 & 1, 0, 0];",
        "let $F = [($E | 0) >> 16 & 255, ($E | 0) >>> 24, ($E | 0) >> 8 & 255, ($E | 0) & 255];",
        "let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 16 & 255, ($E | 0) & 255];",
        "let $F = [($E | 0) >> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];",
        "let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 127, ($E | 0) >> 8 & 255, ($E | 0) & 255];",
        "let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, (($E | 0) & 255) ^ 1];",
        "let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, (~$E) & 255];",
        "let other = Math.random() * 4294967296; let $F = [other >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];",
    ] {
        iso_refused(&iso_mutant(ISO_HEADER, header));
    }
}

#[test]
fn nonce_header_random_and_header_overwrites_and_dead_origins_are_refused() {
    for (from, to) in [
        (ISO_HEADER, format!("$E = 0; {ISO_HEADER}")),
        (ISO_HEADER, format!("{ISO_HEADER} $E = 0;")),
        (ISO_HEADER, format!("{ISO_HEADER} $F = [0, 0, 0, 0];")),
        ("let $E = Math.random() * 4294967296;", "let $E; if (false) { $E = Math.random() * 4294967296; }".to_string()),
        (ISO_HEADER, format!("let $F; if (false) {{ {} }}", ISO_HEADER.replace("let $F =", "$F ="))),
    ] {
        iso_refused(&iso_mutant(from, &to));
    }
}

#[test]
fn nonce_header_supported_coercions_and_unconditional_origins_still_name_both_roles() {
    for header in [
        "let $F = [$E >>> 24, $E >> 16 & 255, $E >> 8 & 255, $E & 255];",
        "let $F = [($E >>> 0) >>> 24 & 255, ($E >>> 0) >>> 16 & 255, ($E >>> 0) >>> 8 & 255, ($E >>> 0) & 255];",
        "let $F; $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];",
    ] {
        let source = carrier(&iso_mutant(ISO_HEADER, header));
        assert_eq!(claims(&source, CONTAINER), 1);
        assert_eq!(claims(&source, EMITTER), 1);
        assert_eq!(applied_names(&source, CONTAINER), vec![CONTAINER.to_string()]);
        assert_eq!(applied_names(&source, EMITTER), vec![EMITTER.to_string()]);
    }
}

#[test]
fn posted_pair_exact_reported_future_attachment_and_dead_attachment_are_refused() {
    for attachment in [
        "$I($J);\n\t\t$J.b = $K;",
        "if (false) { $J.b = $K; }\n\t\t$I($J);",
        "(() => { $J.b = $K; });\n\t\t$I($J);",
        "$J.c = $K;\n\t\t$I($J);",
    ] {
        iso_refused(&iso_mutant("$J.b = $K;\n\t\t$I($J);", attachment));
    }
}

#[test]
fn posted_pair_stale_pair_object_and_payload_overwrites_are_refused() {
    for (from, to) in [
        ("let $K = [[], $F];", "let $K = [[], $F]; $K = [[], []];"),
        ("let $K = [[], $F];", "let $K = [[], []]; $J.b = $K; $K = [[], $F];"),
        ("$J.b = $K;", "$J.b = $K; $J.b = [[], []];"),
        ("$J.b = $K;", "$J.b = $K; $J.a = null;"),
        ("$J.b = $K;", "$J.b = $K; $J = {a: $KEY_4};"),
        ("$J.b = $K;", "$J.b = $K; $K[1] = [];"),
        ("$J.b = $K;", "$J.b = $K; let alias = $J; alias.b = [[], []];"),
        ("$J.b = $K;", "let unrelated = {a: $KEY_4}; unrelated.b = $K;"),
        ("let $J = { a: $KEY_4 };", "let $J = { c: $KEY_4 };"),
        ("let $J = { a: $KEY_4 };", "let $J = { a: $KEY_4, a: null };"),
        ("let $J = { a: $KEY_4 };", "let $J = { a: $KEY_4, ...{a: null} };"),
        ("$I($J);", "$J = {a: $KEY_4, b: $K, b: [[], []]}; $I($J);"),
    ] {
        iso_refused(&iso_mutant(from, to));
    }
}

#[test]
fn posted_pair_nested_and_sibling_pair_donors_are_refused() {
    iso_refused(&iso_mutant(
        "let $K = [[], $F];",
        "let $K; (() => { $K = [[], $F]; });",
    ));
    // The count helper is a sibling of the unused arrow: neither function's
    // assignment can establish a local pair at this postMessage reference.
    let mutant = iso_mutant("let $K = [[], $F];", "let $K;");
    let mutant = mutant.replacen(
        "if ($L == null) {",
        "(() => { $K = [[], $F]; }); if ($L == null) {",
        1,
    );
    iso_refused(&mutant);
}

#[test]
fn posted_pair_inline_pair_and_object_preserve_supported_roles() {
    for body in [
        iso_mutant("let $K = [[], $F];\n\t\t$J.b = $K;", "$J.b = [[], $F];"),
        iso_mutant("let $J = { a: $KEY_4 };\n\t\tlet $K = [[], $F];\n\t\t$J.b = $K;\n\t\t$I($J);", "$I({a: $KEY_4, b: [[], $F]});"),
    ] {
        let source = carrier(&body);
        assert_eq!(claims(&source, CONTAINER), 1);
        assert_eq!(claims(&source, EMITTER), 1);
        assert_eq!(applied_names(&source, CONTAINER), vec![CONTAINER.to_string()]);
        assert_eq!(applied_names(&source, EMITTER), vec![EMITTER.to_string()]);
    }
}

#[test]
fn posted_pair_duplicate_and_spread_pair_overrides_on_actual_posted_object_are_refused() {
    for tail in [
        "$I({a: $KEY_4, b: [[], $F], b: [[], []]});",
        "$I({a: $KEY_4, b: [[], $F], ...{b: [[], []]}});",
        "let $J = {a: $KEY_4, b: [[], $F]}; (() => { $J.b = [[], []]; })(); $I($J);",
        "let $J = {a: $KEY_4}; let $K; $J.b = $K; $K = [[], $F]; $I($J);",
    ] {
        iso_refused(&iso_mutant(
            "let $J = { a: $KEY_4 };\n\t\tlet $K = [[], $F];\n\t\t$J.b = $K;\n\t\t$I($J);",
            tail,
        ));
    }
}

// Complete phase-reuse container with all owned helpers. Recognition must follow dataflow
// rather than source name or offset.
const POS_PHASE_REUSE: &str = r###"function $THREAD_9_2053($A, $B) {
	let $KEY_8 = $A;
	let $KEY_10, $KEY_12, $KEY_13, $KEY_18, $KEY_0;
	function $THREAD_0_38259($E) {
		let $KEY_1 = $E;
		let $KEY_2, $KEY_3;
		$KEY_2 = Object.create(null);
		$KEY_3 = 0;
		function $THREAD_6_103535($F, $G, $H) {
			let $KEY_10;
			if ($KEY_2[$F] != null) {
				$KEY_10 = $H;
				$KEY_10 = Number($H) + 1;
				$G[$KEY_10] = $KEY_1() ^ 129;
				$KEY_10 = $KEY_12($KEY_1, $KEY_2[$F], $G, $KEY_10);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_0($F[$J], 0);
					if ($L === 0) {
						$G[++$K] = $KEY_1() ^ 192;
						$L = 128;
					} else if (!($L < 128)) if ($L < 2048) {
						$G[++$K] = ($L >> 6 | 192) ^ $KEY_1();
						$L = $L & 63 | 128;
					} else {
						let $M = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_0($F[$J + 1], 0) : null;
						if ($M == null || $M < 56320 || $M > 57343) {
							$G[++$K] = ($L >> 12 | 224) ^ $KEY_1();
							$G[++$K] = ($L >> 6 & 63 | 128) ^ $KEY_1();
							$L = $L & 63 | 128;
						} else {
							++$J;
							$G[++$K] = (65536 + ($L - 55296 << 10) + ($M - 56320) >> 18 | 240) ^ $KEY_1();
							$G[++$K] = (65536 + ($L - 55296 << 10) + ($M - 56320) >> 12 & 63 | 128) ^ $KEY_1();
							$G[++$K] = (65536 + ($L - 55296 << 10) + ($M - 56320) >> 6 & 63 | 128) ^ $KEY_1();
							$L = 65536 + ($L - 55296 << 10) + ($M - 56320) & 63 | 128;
						}
					}
					$G[++$K] = $L ^ $KEY_1();
					++$J;
				}
				$KEY_10 = Number($K) + 1;
				$G[$KEY_10] = $KEY_1();
				let $N = $KEY_2;
				let $O = Number($KEY_3);
				$KEY_3 = $O + 1;
				$N[$F] = $O;
			}
			return $KEY_10;
		}
		return $THREAD_6_103535;
	}
	$KEY_13 = $THREAD_0_38259;
	let $KEY_3;
	function $THREAD_2_30045($E, $F, $G, $H) {
		let $KEY_13;
		if (($F | 0) === $F) {
			let $I;
			if ($F < 0) {
				$I = $F < 0;
			} else if ($F === 0) {
				$I = 1 / $F < 0;
			} else {
				$I = $F === 0;
			}
			let $J, $K;
			if ($I) {
				$J = 64;
				$K = -$F;
			} else {
				$J = 0;
				$K = $F;
			}
			if ($K < 32) {
				$KEY_13 = Number($H) + 1;
				$G[$KEY_13] = ($J | $K) ^ $E();
			} else if ($K < 4096) {
				$KEY_13 = Number($H) + 1;
				$G[$KEY_13] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_13] = $K >> 5 ^ $E();
			} else if ($K < 524288) {
				$KEY_13 = Number($H) + 1;
				$G[$KEY_13] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_13] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_13] = $K >>> 12 ^ $E();
			} else if ($K < 67108864) {
				$KEY_13 = Number($H) + 1;
				$G[$KEY_13] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_13] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_13] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_13] = $K >>> 19 ^ $E();
			} else {
				$KEY_13 = Number($H) + 1;
				$G[$KEY_13] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_13] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_13] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_13] = ($K >>> 19 & 127 | 128) ^ $E();
				$G[++$KEY_13] = $K >>> 26 ^ $E();
			}
		} else {
			$KEY_13 = $H;
			$KEY_13 = Number($H) + 1;
			$G[$KEY_13] = $E() ^ 128;
			let $L;
			if ($KEY_3) {
				$KEY_3[0] = $F;
				$L = new $KEY_18($KEY_3.buffer);
			} else if ($F !== $F) {
				$L = $KEY_10;
			} else {
				let $KEY_4 = $F < 0 || $F === 0 && 1 / $F < 0;
				let $M = Math.abs($F);
				let $N, $O;
				if ($M >= Math.pow(2, -1022)) {
					let $KEY_9 = Math.min(Math.floor(Math.log($M) / Math.LN2), 1023);
					$M = $M / Math.pow(2, $KEY_9);
					let $P = $M < 1;
					$M = $P ? $M * 2 : $M;
					$P = $P ? $KEY_9 - 1 : $KEY_9;
					if ($M >= 2) {
						++$P;
						$M = $M / 2;
					}
					let $KEY_8 = Math.pow(2, 52);
					let $KEY_1 = Math.floor($M * $KEY_8);
					const $Q = $M * $KEY_8 - $KEY_1 < .5 ? $KEY_1 : $M * $KEY_8 - $KEY_1 > .5 ? $KEY_1 + 1 : $KEY_1 % 2 ? $KEY_1 + 1 : $KEY_1;
					$P += 1023;
					let $R = ($Q - $KEY_8) / $KEY_8 >= 1;
					$O = $R ? $P + 1 : $P;
					if ($O > 2046) {
						$N = 0;
						$O = 2047;
					} else {
						$N = $R ? 0 : $Q - $KEY_8;
					}
				} else {
					$M = $M / Math.pow(2, -1074);
					let $S = Math.floor($M);
					$N = $M - $S < .5 ? $S : $M - $S > .5 ? $S + 1 : $S % 2 ? $S + 1 : $S;
					$O = 0;
				}
				let $T = [];
				let k = 51;
				for (; k >= 0; k--) {
					$T.unshift(!!($N & 1));
					$N = Math.floor($N / 2);
				}
				let i = 11;
				for (; i > 0; i--) {
					$T.unshift(!!($O & 1));
					$O = Math.floor($O / 2);
				}
				$T.unshift($KEY_4);
				$L = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					$L.push(Number($T[j]) << 7 | Number($T[j + 1]) << 6 | Number($T[j + 2]) << 5 | Number($T[j + 3]) << 4 | Number($T[j + 4]) << 3 | Number($T[j + 5]) << 2 | Number($T[j + 6]) << 1 | Number($T[j + 7]));
				}
			}
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$G[++$KEY_13] = $L[i_2] ^ $E();
			}
		}
		return $KEY_13;
	}
	$KEY_12 = $THREAD_2_30045;
	let $KEY_7;
	function $THREAD_2_1946($E) {
		let $KEY_1, $KEY_2, $KEY_3, $KEY_5;
		function $THREAD_7_38278() {
			let $F = $KEY_3;
			$KEY_3 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_2 = $KEY_5 + $KEY_1;
				let $G = $KEY_1;
				$KEY_1 = $G ^ $KEY_5;
				$KEY_5 = $G ^ $KEY_5 ^ ($KEY_5 << 26 | $KEY_5 >>> 6) ^ $KEY_1 << 9;
				$KEY_1 = $KEY_1 << 13 | $KEY_1 >>> 19;
				return $KEY_2 & 255;
			}
			if (($F + 1) % 4 === 1) {
				return $KEY_2 >> 8 & 255;
			}
			if (($F + 1) % 4 === 2) {
				return $KEY_2 >> 16 & 255;
			}
			if (($F + 1) % 4 === 3) {
				return $KEY_2 >>> 24 & 255;
			} else {
				return void 0;
			}
		}
		$KEY_5 = $KEY_7;
		$KEY_1 = $E;
		$KEY_3 = -1;
		$THREAD_7_38278();
		$THREAD_7_38278();
		$THREAD_7_38278();
		$THREAD_7_38278();
		return $THREAD_7_38278;
	}
	let $KEY_4 = $THREAD_2_1946;
	Object.prototype.hasOwnProperty;
	$KEY_0 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_18 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_3 = $KEY_18 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_10 = $KEY_3 ? null : [0, 0, 0, 0, 0, 0, 248, 127];
	let $C = $B.a.length >> 1 << 1;
	let i = 0;
	let $D = 0;
	for (; i < $C; i = i + 2) {
		$D = ($D << 5) - $D + ($B.a[i].charCodeAt() << 16 | $B.a[i + 1].charCodeAt()) | 0;
	}
	$KEY_7 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_2_13444() {
		let $KEY_0;
		function $THREAD_8_131217($J) {
			if ($J == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($J).length + $KEY_0(Object.getPrototypeOf($J));
			}
		}
		$KEY_0 = $THREAD_8_131217;
		let $KEY_9 = Math.random() * 4294967296 | 0;
		let $KEY_6 = [$KEY_9 >>> 24, $KEY_9 >> 16 & 255, $KEY_9 >> 8 & 255, $KEY_9 & 255];
		$KEY_9 = $KEY_4($KEY_9 ^ 3855773783);
		let $KEY_7 = $KEY_13($KEY_9);
		let $E = $KEY_7(navigator.userAgent, $KEY_6, 3);
		let $F = navigator.hardwareConcurrency;
		if ($F == null) {
			$KEY_6[++$E] = $KEY_9() ^ 130;
		} else {
			$E = $KEY_12($KEY_9, $F, $KEY_6, $E);
		}
		$E = $KEY_7(navigator.platform, $KEY_6, $E);
		$KEY_7 = $KEY_7(location.origin, $KEY_6, $E);
		$KEY_12($KEY_9, $KEY_0(self), $KEY_6, $KEY_7);
		let $G = postMessage;
		let $H = { a: $KEY_8 };
		let $I = [[], $KEY_6];
		$H.b = $I;
		$G($H);
	}
	$THREAD_2_13444($B.b);
}"###;

fn phase_reuse_result(body: &str, accepted: bool) {
    let source = carrier(body);
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator, &source, oxc_span::SourceType::unambiguous(),
    ).parse();
    assert!(!parsed.panicked && parsed.errors.is_empty(), "invalid fixture: {:?}", parsed.errors);
    let result = name_threads(&source, true).expect("static fixture analysis");
    for (target, role) in [
        ("$THREAD_9_2053", CONTAINER),
        ("$THREAD_2_13444", EMITTER),
    ] {
        let outcome = result.report.outcomes.iter()
            .find(|outcome| outcome.binding.name == target)
            .unwrap_or_else(|| panic!("missing target {target}"));
        let entry = result.report.function_roles.iter()
            .find(|entry| entry.current_name.as_deref() == Some(target))
            .unwrap_or_else(|| panic!("missing role entry {target}"));
        assert_eq!(outcome.applied && outcome.role_name.as_deref() == Some(role), accepted,
            "{target}: {body}");
        assert_eq!(outcome.evaluator.as_deref() == Some(EVALUATOR), accepted, "{target}: {body}");
        assert_eq!(entry.evaluators.iter().any(|evaluator| evaluator == EVALUATOR), accepted,
            "{target}: {body}");
    }
    for role in [CONTAINER, EMITTER] {
        assert_eq!(claims(&source, role), usize::from(accepted), "{body}");
    }
}

fn phase_reuse_mutant(from: &str, to: &str) -> String {
    assert_eq!(POS_PHASE_REUSE.matches(from).count(), 1, "ambiguous/missing mutation {from}");
    let body = POS_PHASE_REUSE.replacen(from, to, 1);
    assert_ne!(body, POS_PHASE_REUSE);
    body
}

#[test]
fn phase_reuse_names_exact_container_and_emitter() {
    phase_reuse_result(POS_PHASE_REUSE, true);
    for helper in [
        "writeNumber", "createInternedStringWriter", "writeInternedString",
        "createKeystreamByteSource", "nextKeystreamByte",
        "countOwnPropertiesAlongPrototypeChain",
    ] {
        assert!(has_applied(&carrier(POS_PHASE_REUSE), helper), "missing helper {helper}");
    }
}

#[test]
fn phase_reuse_random_phase_and_final_writer_call_are_independently_supported() {
    // Isolate each reuse: preserve nonce reuse with a separate final cursor,
    // then preserve callable/cursor reuse with a separate nonce/stream binding.
    phase_reuse_result(&phase_reuse_mutant(
        "$KEY_7 = $KEY_7(location.origin, $KEY_6, $E);\n\t\t$KEY_12($KEY_9, $KEY_0(self), $KEY_6, $KEY_7);",
        "let finalCursor = $KEY_7(location.origin, $KEY_6, $E);\n\t\t$KEY_12($KEY_9, $KEY_0(self), $KEY_6, finalCursor);",
    ), true);
    let body = phase_reuse_mutant(
        "let $KEY_9 = Math.random() * 4294967296 | 0;\n\t\tlet $KEY_6 = [$KEY_9 >>> 24, $KEY_9 >> 16 & 255, $KEY_9 >> 8 & 255, $KEY_9 & 255];\n\t\t$KEY_9 = $KEY_4($KEY_9 ^ 3855773783);",
        "let nonce = Math.random() * 4294967296 | 0;\n\t\tlet $KEY_6 = [nonce >>> 24, nonce >> 16 & 255, nonce >> 8 & 255, nonce & 255];\n\t\tlet $KEY_9 = $KEY_4(nonce ^ 3855773783);",
    );
    phase_reuse_result(&body, true);
}

#[test]
fn phase_reuse_phase_clobbers_late_seed_and_donors_are_refused() {
    const SEED: &str = "$KEY_9 = $KEY_4($KEY_9 ^ 3855773783);";
    const HEADER: &str = "let $KEY_6 = [$KEY_9 >>> 24, $KEY_9 >> 16 & 255, $KEY_9 >> 8 & 255, $KEY_9 & 255];";
    for body in [
        phase_reuse_mutant(HEADER, &format!("$KEY_9 = 0; {HEADER}")),
        phase_reuse_mutant(SEED, &format!("$KEY_9 = 0; {SEED}")),
        phase_reuse_mutant(SEED, &format!("$KEY_9 = Math.random() * 4294967296 | 0; {SEED}")),
        phase_reuse_mutant(SEED, &format!("{SEED} $KEY_9 = 0;")),
        phase_reuse_mutant(SEED, &format!("if (false) {{ {SEED} }}")),
        phase_reuse_mutant(SEED, &format!("(() => {{ {SEED} }});")),
        phase_reuse_mutant(
            &format!("{HEADER}\n\t\t{SEED}"),
            &format!("{SEED}\n\t\t{HEADER}"),
        ),
        phase_reuse_mutant(
            &format!("{SEED}\n\t\tlet $KEY_7 = $KEY_13($KEY_9);"),
            &format!("let $KEY_7 = $KEY_13($KEY_9);\n\t\t{SEED}"),
        ),
        phase_reuse_mutant("let $E = $KEY_7(navigator.userAgent, $KEY_6, 3);",
            "$KEY_7 = 0; let $E = $KEY_7(navigator.userAgent, $KEY_6, 3);"),
        phase_reuse_mutant("$E = $KEY_7(navigator.platform, $KEY_6, $E);",
            "$KEY_7 = 0; $E = $KEY_7(navigator.platform, $KEY_6, $E);"),
        phase_reuse_mutant("$KEY_7 = $KEY_7(location.origin, $KEY_6, $E);",
            "$KEY_7 = 0; $KEY_7 = $KEY_7(location.origin, $KEY_6, $E);"),
        phase_reuse_mutant("let $KEY_7 = $KEY_13($KEY_9);",
            "let $KEY_7; (() => { $KEY_7 = $KEY_13($KEY_9); });"),
        phase_reuse_mutant("$KEY_7 = $KEY_7(location.origin, $KEY_6, $E);",
            "$KEY_7 = $KEY_7(location.origin, $KEY_6, $E); $KEY_7(navigator.platform, $KEY_6, $E);"),
        phase_reuse_mutant(SEED, &format!("{SEED} unknown($KEY_9 = 0);")),
    ] {
        phase_reuse_result(&body, false);
    }
}

// Complete terminal-stream-cursor container with its owned helpers.
const POS_TERMINAL_CURSOR: &str = r###"function $THREAD_8_7310($A, $B) {
	let $KEY_3 = $A;
	let $KEY_4, $KEY_7, $KEY_10, $KEY_11, $KEY_13, $KEY_15;
	function $THREAD_6_51157($E) {
		let $KEY_2 = $E;
		let $KEY_0 = Object.create(null);
		let $KEY_1 = 0;
		function $THREAD_9_122274($F, $G, $H) {
			let $KEY_6;
			if ($KEY_0[$F] != null) {
				$KEY_6 = $H;
				$KEY_6 = Number($H) + 1;
				$G[$KEY_6] = $KEY_2() ^ 129;
				$KEY_6 = $KEY_4($KEY_2, $KEY_0[$F], $G, $KEY_6);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_7($F[$J], 0);
					if ($L === 0) {
						$G[++$K] = $KEY_2() ^ 192;
						$L = 128;
					} else if (!($L < 128)) if ($L < 2048) {
						$G[++$K] = ($L >> 6 | 192) ^ $KEY_2();
						$L = $L & 63 | 128;
					} else {
						let $M = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_7($F[$J + 1], 0) : null;
						if ($M == null || $M < 56320 || $M > 57343) {
							$G[++$K] = ($L >> 12 | 224) ^ $KEY_2();
							$G[++$K] = ($L >> 6 & 63 | 128) ^ $KEY_2();
							$L = $L & 63 | 128;
						} else {
							++$J;
							$G[++$K] = (65536 + ($L - 55296 << 10) + ($M - 56320) >> 18 | 240) ^ $KEY_2();
							$G[++$K] = (65536 + ($L - 55296 << 10) + ($M - 56320) >> 12 & 63 | 128) ^ $KEY_2();
							$G[++$K] = (65536 + ($L - 55296 << 10) + ($M - 56320) >> 6 & 63 | 128) ^ $KEY_2();
							$L = 65536 + ($L - 55296 << 10) + ($M - 56320) & 63 | 128;
						}
					}
					$G[++$K] = $L ^ $KEY_2();
					++$J;
				}
				$KEY_6 = Number($K) + 1;
				$G[$KEY_6] = $KEY_2();
				let $N = $KEY_0;
				let $O = Number($KEY_1);
				$KEY_1 = $O + 1;
				$N[$F] = $O;
			}
			return $KEY_6;
		}
		return $THREAD_9_122274;
	}
	let $KEY_2 = $THREAD_6_51157;
	function $THREAD_4_21145($E, $F, $G, $H) {
		let $KEY_3;
		if (($F | 0) === $F) {
			let $I;
			if ($F < 0) {
				$I = $F < 0;
			} else if ($F === 0) {
				$I = 1 / $F < 0;
			} else {
				$I = false;
			}
			let $J, $K;
			if ($I) {
				$J = 64;
				$K = -$F;
			} else {
				$J = 0;
				$K = $F;
			}
			if ($K < 32) {
				$KEY_3 = Number($H) + 1;
				$G[$KEY_3] = ($J | $K) ^ $E();
			} else if ($K < 4096) {
				let $L = Number($H) + 1;
				$G[$L] = ($J | ($K & 31 | 32)) ^ $E();
				$G[$L + 1] = $K >> 5 ^ $E();
				$KEY_3 = $L + 1;
			} else if ($K < 524288) {
				let $M = Number($H) + 1;
				$G[$M] = ($J | ($K & 31 | 32)) ^ $E();
				$G[$M + 1] = ($K >> 5 & 127 | 128) ^ $E();
				$G[$M + 2] = $K >>> 12 ^ $E();
				$KEY_3 = $M + 2;
			} else if ($K < 67108864) {
				let $N = Number($H) + 1;
				$G[$N] = ($J | ($K & 31 | 32)) ^ $E();
				$G[$N + 1] = ($K >> 5 & 127 | 128) ^ $E();
				$G[$N + 2] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[$N + 3] = $K >>> 19 ^ $E();
				$KEY_3 = $N + 3;
			} else {
				let $O = Number($H) + 1;
				$G[$O] = ($J | ($K & 31 | 32)) ^ $E();
				$G[$O + 1] = ($K >> 5 & 127 | 128) ^ $E();
				$G[$O + 2] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[$O + 3] = ($K >>> 19 & 127 | 128) ^ $E();
				$G[$O + 4] = $K >>> 26 ^ $E();
				$KEY_3 = $O + 4;
			}
		} else {
			$KEY_3 = Number($H) + 1;
			$G[$KEY_3] = $E() ^ 128;
			let $P;
			if ($KEY_13) {
				$KEY_13[0] = $F;
				$P = new $KEY_11($KEY_13.buffer);
			} else {
				let $Q;
				if ($F !== $F) {
					$Q = $KEY_15;
				} else {
					let $KEY_8 = $F < 0 || $F === 0 && 1 / $F < 0;
					let $R = Math.abs($F);
					let $S;
					if ($R >= Math.pow(2, -1022)) {
						let $KEY_5 = Math.min(Math.floor(Math.log($R) / Math.LN2), 1023);
						$R = $R / Math.pow(2, $KEY_5);
						let $T = $R < 1;
						$KEY_5 = $T ? $KEY_5 - 1 : $KEY_5;
						$T = $T ? $R * 2 : $R;
						let $U = $T >= 2;
						$T = $U ? $T / 2 : $T;
						let $V = Math.pow(2, 52);
						let $KEY_1 = Math.floor($T * $V);
						$T = $T * $V - $KEY_1 < .5 ? $KEY_1 : $T * $V - $KEY_1 > .5 ? $KEY_1 + 1 : $KEY_1 % 2 ? $KEY_1 + 1 : $KEY_1;
						$U = ($U ? $KEY_5 + 1 : $KEY_5) + 1023;
						let $W = ($T - $V) / $V >= 1;
						$S = $W ? $U + 1 : $U;
						if ($S > 2046) {
							$S = 2047;
							$R = 0;
						} else {
							$R = $W ? 0 : $T - $V;
						}
					} else {
						$R = $R / Math.pow(2, -1074);
						let $X = Math.floor($R);
						$S = 0;
						$R = $R - $X < .5 ? $X : $R - $X > .5 ? $X + 1 : $X % 2 ? $X + 1 : $X;
					}
					let $Y = [];
					let j = 51;
					for (; j >= 0; j--) {
						$Y.unshift(!!($R & 1));
						$R = Math.floor($R / 2);
					}
					let i = 11;
					for (; i > 0; i--) {
						$Y.unshift(!!($S & 1));
						$S = Math.floor($S / 2);
					}
					$Y.unshift($KEY_8);
					let $Z = [];
					let $AA = 56;
					while ($AA >= 0) {
						$Z.push(Number($Y[$AA]) << 7 | Number($Y[$AA + 1]) << 6 | Number($Y[$AA + 2]) << 5 | Number($Y[$AA + 3]) << 4 | Number($Y[$AA + 4]) << 3 | Number($Y[$AA + 5]) << 2 | Number($Y[$AA + 6]) << 1 | Number($Y[$AA + 7]));
						$AA = $AA - 8;
					}
					$Q = $Z;
				}
				$P = $Q;
			}
			let k = 0;
			for (; k < 8; k++) {
				$G[++$KEY_3] = $P[k] ^ $E();
			}
		}
		return $KEY_3;
	}
	$KEY_4 = $THREAD_4_21145;
	function $THREAD_6_54837($E) {
		let $KEY_0, $KEY_1, $KEY_2, $KEY_4;
		function $THREAD_3_126717() {
			let $F = $KEY_2;
			$KEY_2 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_1 = $KEY_0 + $KEY_4;
				let $G = $KEY_4;
				$KEY_4 = $G ^ $KEY_0;
				$KEY_0 = $G ^ $KEY_0 ^ ($KEY_0 << 26 | $KEY_0 >>> 6) ^ $KEY_4 << 9;
				$KEY_4 = $KEY_4 << 13 | $KEY_4 >>> 19;
				return $KEY_1 & 255;
			}
			if (($F + 1) % 4 === 1) {
				return $KEY_1 >> 8 & 255;
			}
			if (($F + 1) % 4 === 2) {
				return $KEY_1 >> 16 & 255;
			}
			if (($F + 1) % 4 === 3) {
				return $KEY_1 >>> 24 & 255;
			} else {
				return void 0;
			}
		}
		$KEY_0 = $KEY_10;
		$KEY_4 = $E;
		$KEY_2 = -1;
		$THREAD_3_126717();
		$THREAD_3_126717();
		$THREAD_3_126717();
		$THREAD_3_126717();
		return $THREAD_3_126717;
	}
	let $KEY_1 = $THREAD_6_54837;
	Object.prototype.hasOwnProperty;
	$KEY_7 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_11 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_13 = $KEY_11 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_15 = $KEY_13 ? null : (($PARAMA, $PARAMB) => {
		$PARAMA[$PARAMB] = 248;
		$PARAMA[7] = 127;
		return $PARAMA;
	})(([0, 0, 0, 0, 0, 0]), 6);
	let $C = $B.a.length >> 1 << 1;
	let i = 0;
	let $D = 0;
	for (; i < $C; i = i + 2) {
		$D = ($D << 5) - $D + ($B.a[i].charCodeAt() << 16 | $B.a[i + 1].charCodeAt()) | 0;
	}
	$KEY_10 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_8_72160() {
		let $KEY_10;
		function $THREAD_0_159703($K) {
			if ($K == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($K).length + $KEY_10(Object.getPrototypeOf($K));
			}
		}
		$KEY_10 = $THREAD_0_159703;
		let $E = Math.random() * 4294967296;
		let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];
		let $KEY_11 = $KEY_1(($E | 0) ^ 2147929915);
		let $KEY_0 = $KEY_2($KEY_11);
		let $G = navigator.hardwareConcurrency;
		if ($G == null) {
			$F[4] = $KEY_11() ^ 130;
			$G = 4;
		} else {
			$G = $KEY_4($KEY_11, $G, $F, 3);
		}
		$G = $KEY_0(navigator.userAgent, $F, $G);
		$G = $KEY_0(navigator.platform, $F, $G);
		$KEY_11 = $KEY_4($KEY_11, $KEY_10(self), $F, $G);
		$KEY_0(location.origin, $F, $KEY_11);
		let $H = postMessage;
		let $I = { a: $KEY_3 };
		let $J = [[], $F];
		$I.b = $J;
		$H($I);
	}
	$THREAD_8_72160($B.b);
}"###;

fn terminal_cursor_result(body: &str, accepted: bool) {
    let source = carrier(body);
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator, &source, oxc_span::SourceType::unambiguous(),
    ).parse();
    assert!(!parsed.panicked && parsed.errors.is_empty(), "invalid fixture: {:?}", parsed.errors);
    let result = name_threads(&source, true).expect("static fixture analysis");
    for (target, role) in [
        ("$THREAD_8_7310", CONTAINER),
        ("$THREAD_8_72160", EMITTER),
    ] {
        let outcome = result.report.outcomes.iter()
            .find(|outcome| outcome.binding.name == target)
            .unwrap_or_else(|| panic!("missing target {target}"));
        let entry = result.report.function_roles.iter()
            .find(|entry| entry.current_name.as_deref() == Some(target))
            .unwrap_or_else(|| panic!("missing role entry {target}"));
        assert_eq!(outcome.applied && outcome.role_name.as_deref() == Some(role), accepted,
            "{target}: {body}");
        assert_eq!(outcome.evaluator.as_deref() == Some(EVALUATOR), accepted, "{target}: {body}");
        assert_eq!(entry.evaluators.iter().any(|evaluator| evaluator == EVALUATOR), accepted,
            "{target}: {body}");
    }
    for role in [CONTAINER, EMITTER] {
        assert_eq!(claims(&source, role), usize::from(accepted), "{body}");
    }
}

fn terminal_cursor_mutant(from: &str, to: &str) -> String {
    assert_eq!(POS_TERMINAL_CURSOR.matches(from).count(), 1, "ambiguous/missing mutation {from}");
    let body = POS_TERMINAL_CURSOR.replacen(from, to, 1);
    assert_ne!(body, POS_TERMINAL_CURSOR);
    body
}

#[test]
fn terminal_stream_cursor_names_both_roles() {
    terminal_cursor_result(POS_TERMINAL_CURSOR, true);
    for helper in [
        "writeNumber", "createInternedStringWriter", "writeInternedString",
        "createKeystreamByteSource", "nextKeystreamByte",
        "countOwnPropertiesAlongPrototypeChain",
    ] {
        assert!(has_applied(&carrier(POS_TERMINAL_CURSOR), helper), "missing helper {helper}");
    }
}

#[test]
fn terminal_cursor_equivalent_word_coercions_and_renamed_locals_are_supported() {
    // Every header/seed operation observes the same low 32 bits. Exercise the
    // terminal stream phase independently of generated local names and salt.
    let body = POS_TERMINAL_CURSOR.replace("($E | 0)", "($E >>> 0)")
        .replace("2147929915", "123456789")
        .replace("$KEY_", "$LOCAL_");
    terminal_cursor_result(&body, true);
    let body = POS_TERMINAL_CURSOR
        .replace("let $E = Math.random() * 4294967296;", "let $KEY_11 = Math.random() * 4294967296;")
        .replace("($E | 0)", "($KEY_11 | 0)")
        .replace("let $KEY_11 = $KEY_1", "$KEY_11 = $KEY_1");
    terminal_cursor_result(&body, true);
    terminal_cursor_result(&terminal_cursor_mutant(
        "$KEY_11 = $KEY_4($KEY_11, $KEY_10(self), $F, $G);\n\t\t$KEY_0(location.origin, $F, $KEY_11);",
        "let finalCursor = $KEY_4($KEY_11, $KEY_10(self), $F, $G);\n\t\t$KEY_0(location.origin, $F, finalCursor);",
    ), true);
}

#[test]
fn terminal_cursor_mismatched_word_or_clobbered_nonce_is_refused() {
    for body in [
        terminal_cursor_mutant("($E | 0) ^ 2147929915", "($E | 1) ^ 2147929915"),
        terminal_cursor_mutant("($E | 0) >>> 24", "($E >>> 1) >>> 24"),
        terminal_cursor_mutant("let $KEY_11 = $KEY_1", "$E = 0; let $KEY_11 = $KEY_1"),
        terminal_cursor_mutant("let $KEY_11 = $KEY_1", "$E = Math.random() * 4294967296; let $KEY_11 = $KEY_1"),
    ] {
        terminal_cursor_result(&body, false);
    }
}

#[test]
fn terminal_cursor_terminal_stream_cursor_clobbers_and_callable_reuse_are_refused() {
    const FINAL: &str = "$KEY_11 = $KEY_4($KEY_11, $KEY_10(self), $F, $G);";
    const USE: &str = "$KEY_0(location.origin, $F, $KEY_11);";
    for body in [
        terminal_cursor_mutant(FINAL, "$KEY_11 = 0;"),
        terminal_cursor_mutant(FINAL, "$KEY_11 = $KEY_4($KEY_11, 0, $F, $G);"),
        terminal_cursor_mutant(FINAL, "$KEY_11 = $KEY_4($KEY_11, $KEY_10(window), $F, $G);"),
        terminal_cursor_mutant(FINAL, "$KEY_11 = $KEY_4($KEY_11, $KEY_10(self), [], $G);"),
        terminal_cursor_mutant(FINAL, &format!("$KEY_11 = 0; {FINAL}")),
        terminal_cursor_mutant(FINAL, &format!("{FINAL} $KEY_11 = 0;")),
        terminal_cursor_mutant(FINAL, &format!("if (false) {{ {FINAL} }}")),
        terminal_cursor_mutant(FINAL, &format!("(() => {{ {FINAL} }});")),
        terminal_cursor_mutant(USE, &format!("{USE} $KEY_11();")),
        terminal_cursor_mutant(USE, &format!("{USE} (() => $KEY_11());")),
        terminal_cursor_mutant(FINAL, &format!("(() => $KEY_11()); {FINAL}")),
        terminal_cursor_mutant(USE, &format!("{USE} $KEY_4($KEY_11, 1, $F, $G);")),
        terminal_cursor_mutant(USE, "$KEY_0(location.origin, $F, 3);"),
        terminal_cursor_mutant(USE, &format!("if (false) {{ {USE} }}")),
        terminal_cursor_mutant(USE, &format!("(() => {{ {USE} }});")),
        terminal_cursor_mutant(USE, &format!("{USE} {USE}")),
        terminal_cursor_mutant("$I.b = $J;", "$I.b = $J; $I.b = [[], []];"),
    ] {
        terminal_cursor_result(&body, false);
    }
}

const WHOLE_EPOCH_EPOCH: &str = include_str!("fixtures/worker-environment-count-helper-neighbor.js");
fn epoch_result(body:&str,accepted:bool) {
    let source=carrier(body);
    let allocator=oxc_allocator::Allocator::default();
    let parsed=oxc_parser::Parser::new(&allocator,&source,oxc_span::SourceType::unambiguous()).parse();
    assert!(!parsed.panicked && parsed.errors.is_empty(),"invalid fixture: {:?}",parsed.errors);
    let result=name_threads(&source,true).expect("production matcher");
    for (target,role) in [("$THREAD_7_93340",CONTAINER),("$THREAD_2_34779",EMITTER)] {
        let target=if body.contains("$THREAD_887_93340"){target.replace("$THREAD_","$THREAD_88")}else{target.to_string()};
        let outcome=result.report.outcomes.iter().find(|o|o.binding.name==target).unwrap_or_else(||panic!("missing candidate {target}"));
        let entry=result.report.function_roles.iter().find(|e|e.current_name.as_deref()==Some(target.as_str())).unwrap_or_else(||panic!("missing role entry {target}"));
        assert_eq!(outcome.applied && outcome.role_name.as_deref()==Some(role) && outcome.evaluator.as_deref()==Some(EVALUATOR),accepted,"{target}: {body}");
        assert_eq!(entry.evaluators.iter().any(|e|e==EVALUATOR),accepted,"{target}: {body}");
    }
}
fn epoch_mutant(source:&str,from:&str,to:&str)->String {
    assert_eq!(source.matches(from).count(),1,"mutation must be unique: {from}");
    source.replacen(from,to,1)
}
fn split_epoch()->String {
    epoch_mutant(WHOLE_EPOCH_EPOCH,"$KEY_6 = [[], $KEY_6];","let pair = [[], $KEY_6];").replace("$J.b = $KEY_6;","$J.b = pair;")
}
#[test]
fn actual_whole_epoch_whole_epoch_and_alpha_and_separate_pair_are_supported() {
    epoch_result(WHOLE_EPOCH_EPOCH,true);
    epoch_result(&WHOLE_EPOCH_EPOCH.replace("$KEY_","$LOCAL_").replace("$THREAD_","$THREAD_88"),true);
    epoch_result(&split_epoch(),true);
}
#[test]
fn epoch_transition_and_sink_matrix_refuses_on_supported_whole_owner() {
    epoch_result(WHOLE_EPOCH_EPOCH,true);
    for (from,to) in [
        ("$KEY_6 = [[], $KEY_6];","$KEY_6 = [[], []];"),
        ("$KEY_6 = [[], $KEY_6];","$KEY_6 = [$KEY_6, []];"),
        ("$KEY_6 = [[], $KEY_6];","$KEY_6 = [[], otherHeader];"),
        ("$KEY_6 = [[], $KEY_6];","if(false){$KEY_6 = [[], $KEY_6];}"),
        ("$KEY_6 = [[], $KEY_6];","(()=>{$KEY_6 = [[], $KEY_6];});"),
        ("$KEY_6 = [[], $KEY_6];","$KEY_6 = [[], $KEY_6]; $KEY_6 = [[], $KEY_6];"),
        ("$KEY_6 = [[], $KEY_6];","$KEY_6 = [[], $KEY_6]; $KEY_6[0]=0;"),
        ("$KEY_6 = [[], $KEY_6];","$KEY_6 = [[], $KEY_6]; $KEY_6[1]=[];"),
        ("$KEY_6 = [[], $KEY_6];","$KEY_6 = [[], $KEY_6]; escapePair($KEY_6);"),
        ("$J.b = $KEY_6;","$J.b = $KEY_6; $J.b=[[],[]];"),
        ("$J.b = $KEY_6;","$J.b = $KEY_6; $J.a=null;"),
        ("$J.b = $KEY_6;","let unrelated={a:$KEY_14};unrelated.b=$KEY_6;"),
        ("$J.b = $KEY_6;\n\t\t$I($J);","$I($J);$J.b=$KEY_6;"),
        ("let $J = { a: $KEY_14 };","let $J = {a:$KEY_14,a:null};"),
        ("let $J = { a: $KEY_14 };","let $J = {a:$KEY_14,...{a:null}};"),
        ("let $I = postMessage;","let $I = window.postMessage;"),
        ("$I($J);","$I($J);$I($J);"),
        ("$KEY_6 = [[], $KEY_6];","$KEY_6 = [[], $KEY_6];$F(location.origin,$KEY_6,$G);"),
        ("$F(location.origin, $KEY_6, $G);","$KEY_6=[[],$KEY_6];$F(location.origin,$KEY_6,$G);"),
    ] { epoch_result(&epoch_mutant(WHOLE_EPOCH_EPOCH,from,to),false); }
}
#[test]
fn epoch_cursor_and_capture_mutants_refuse_in_both_representations() {
    for source in [WHOLE_EPOCH_EPOCH.to_string(),split_epoch()] {
        epoch_result(&source,true);
        for (from,to) in [
            ("$KEY_6[++$G] =","$KEY_6[$G] ="),
            ("$KEY_3() ^ 130","otherStream() ^ 130"),
            ("$KEY_3() ^ 130","$KEY_3() ^ 129"),
            ("$G = $KEY_2($KEY_3, $H, $KEY_6, $G);","$H = $KEY_2($KEY_3, $H, $KEY_6, $G);"),
            ("$F(location.origin, $KEY_6, $G);","$G=0;$F(location.origin, $KEY_6, $G);"),
            ("$I($J);","unrelatedEffect();$I($J);"),
            ("$I($J);","$THREAD_2_34779();$I($J);"),
            ("let $KEY_7 = $THREAD_6_129153;","let $KEY_7 = otherFactory;"),
            ("$KEY_2 = $THREAD_5_93671;","$KEY_2 = otherNumber;"),
            ("$KEY_1 = Array.call.bind(String.prototype.charCodeAt);","if(false){$KEY_1 = Array.call.bind(String.prototype.charCodeAt);}"),
            ("$KEY_1 = Array.call.bind(String.prototype.charCodeAt);","$KEY_1 = Array.call.bind(String.prototype.charAt);"),
            ("$KEY_12 = typeof Uint8Array === \"function\" ? Uint8Array : null;","$KEY_12 = null;"),
            ("$D = ($D << 5) - $D + ($B.a[i].charCodeAt() << 16 | $B.a[i + 1].charCodeAt()) | 0;","$D = 0;"),
            ("let $KEY_3 = $KEY_4(($E | 0) ^ 1210093294);","$KEY_4=0;let $KEY_3 = $KEY_4(($E | 0) ^ 1210093294);"),
            ("$KEY_1 = $KEY_5;","$KEY_1 = $KEY_5;unrelatedEffect();"),
            ("$S.unshift($KEY_1);","$S.unshift($KEY_1);unrelatedEffect();"),
        ] {epoch_result(&epoch_mutant(&source,from,to),false);}
        let key="$KEY_5 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;";
        let late=epoch_mutant(&source,key,"");
        epoch_result(&epoch_mutant(&late,"$THREAD_2_34779($B.b);",&format!("$THREAD_2_34779($B.b);{key}")),false);
        let early=epoch_mutant(&source,"$THREAD_2_34779($B.b);","");
        epoch_result(&epoch_mutant(&early,"let $KEY_7 = $THREAD_6_129153;","$THREAD_2_34779($B.b);let $KEY_7 = $THREAD_6_129153;"),false);
    }
}

// A refusal gate on a supported original/alpha pair must reject the mutation rather than
// merely emit an accepted diagnostic.
#[test]
fn numeric_fallback_scalar_clobber_is_refused_for_original_and_alpha() {
    for alpha in [false,true] {
        let rename=|s:&str|if alpha{s.replace("$KEY_","$LOCAL_").replace("$THREAD_","$THREAD_88")}else{s.to_string()};
        epoch_result(&rename(WHOLE_EPOCH_EPOCH),true);
        let mutant=epoch_mutant(WHOLE_EPOCH_EPOCH,"let $M = Math.abs($F);","let $M = Math.abs($F); $M = 0;");
        epoch_result(&rename(&mutant),false);
    }
}
#[test]
fn complete_numeric_scalar_literals_operators_and_typed_ports_are_closed() {
    epoch_result(WHOLE_EPOCH_EPOCH,true);
    epoch_result(&WHOLE_EPOCH_EPOCH.replace("$KEY_","$LOCAL_").replace("$THREAD_","$THREAD_88"),true);
    for (from,to) in [
        ("let $M = Math.abs($F);","let $M = Math.abs($F)+1;"),
        ("let $M = Math.abs($F);","let $M = Math.abs($H);"),
        ("$M - $P < .5","$M - $P < .25"),
        ("$M - $P > .5","$M - $P > .25"),
        ("Math.min(Math.floor(Math.log($M) / Math.LN2), 1023)","Math.min(Math.floor(Math.log($M) / Math.LN2), 1022)"),
        ("$O = $Q ? $O + 1024 : $O + 1023;","$O = $Q ? $O + 1024 : $O + 1022;"),
        ("if ($O > 2046)","if ($O > 2045)"),
        ("$O = 2047;","$O = 2046;"),
        ("$P % 2","$P % 3"),
        ("$N & 1","$N & 2"),
        ("Number($S[j]) << 7","Number($S[j]) << 6"),
        ("Math.pow(2, -1074)","Math.pow(2, -1073)"),
        ("Math.pow(2, -1022)","Math.pow(2, -1021)"),
        ("Math.pow(2, 52)","Math.pow(2, 51)"),
        ("$G[$KEY_10] = $E() ^ 128;","$G[$KEY_10] = $E() ^ 127;"),
        ("let $P = Math.floor($M);","let $P = otherNumeric($M);"),
        ("$KEY_8[0] = $F;","$KEY_8[1] = $F;"),
        ("new $KEY_12($KEY_8.buffer)","new $KEY_8($KEY_8.buffer)"),
        ("$L = $KEY_13;","$L = $KEY_8;"),
        ("$L = new $KEY_12($KEY_8.buffer);","$L = new $KEY_12($KEY_13.buffer);"),
        ("$G[++$KEY_10] = $L[i_2] ^ $E();","$G[$KEY_10] = $L[i_2] ^ $E();"),
        ("for (; i_2 < 8; i_2++)","for (; i_2 < 7; i_2++)"),
        ("$J = 64;","$J = 65;"),
        ("$K < 4096","$K < 4095"),
        ("$G[++$KEY_10] = $K >>> 12 ^ $E();","$G[++$KEY_10] = $K >>> 13 ^ $E();"),
    ] {
        let body=epoch_mutant(WHOLE_EPOCH_EPOCH,from,to);
        epoch_result(&body,false);
        epoch_result(&body.replace("$KEY_","$LOCAL_").replace("$THREAD_","$THREAD_88"),false);
    }
}

#[test]
fn saved_selected_worker_containers_preserve_both_roles() {
    for (body,container,emitter) in [
        (include_str!("fixtures/worker_environment_controls/two-argument-owner-guarded-layout.js"),"$THREAD_0_115855","$THREAD_4_137203"),
        (include_str!("fixtures/worker_environment_controls/two-argument-owner-branch-layout.js"),"$THREAD_0_31722","$THREAD_4_113327"),
        (include_str!("fixtures/worker_environment_controls/two-argument-owner-numeric-layout.js"),"$THREAD_5_52926","$THREAD_7_49410"),
        (include_str!("fixtures/worker_environment_controls/helper-count-owner.js"),"$THREAD_8_13514","$THREAD_0_109848"),
        (include_str!("fixtures/worker_environment_controls/two-argument-owner-mixed-layout.js"),"$THREAD_9_58089","$THREAD_6_40205"),
        (include_str!("fixtures/worker_environment_controls/two-argument-owner-baseline.js"),"$THREAD_0_165997","$THREAD_9_123566"),
        (include_str!("fixtures/worker_environment_controls/two-argument-owner-reset-layout.js"),"$THREAD_1_66217","$THREAD_2_141560"),
        (include_str!("fixtures/worker_environment_controls/two-argument-owner-heldout.js"),"$THREAD_8_135183","$THREAD_7_125068"),
        (include_str!("fixtures/worker_environment_controls/two-argument-owner-alternate.js"),"$THREAD_1_78447","$THREAD_8_90904"),
    ] {
        let source=carrier(body);
        let allocator=oxc_allocator::Allocator::default();
        let parsed=oxc_parser::Parser::new(&allocator,&source,oxc_span::SourceType::unambiguous()).parse();
        assert!(!parsed.panicked && parsed.errors.is_empty(),"{:?}",parsed.errors);
        let result=name_threads(&source,true).expect("production control matcher");
        for (target,role) in [(container,CONTAINER),(emitter,EMITTER)] {
            let outcome=result.report.outcomes.iter().find(|o|o.binding.name==target).unwrap_or_else(||panic!("missing control {target}"));
            assert!(outcome.applied && outcome.role_name.as_deref()==Some(role) && outcome.evaluator.as_deref()==Some(EVALUATOR),"control {target}: {body}");
        }
    }
}

#[test]
fn composed_shared_numeric_role_still_rejects_body_mutation() {
    let body = include_str!("fixtures/worker_environment_controls/two-argument-owner-reset-layout.js");
    let mutated = body.replace("$J = 64;", "$J = 65;");
    assert_ne!(body, mutated);
    assert_refused(&carrier(&mutated));
}

#[path = "worker_environment_two_argument_owner.rs"]
mod guarded_reuse;

#[test]
fn original_and_broader_closed_proofs_keep_canonical_metadata() {
    // Original numeric proof and the newly accepted shared numeric fallback.
    for body in [POS_GUARDED_CONTAINER, include_str!("fixtures/worker_environment_controls/two-argument-owner-reset-layout.js")] {
        let result = name_threads(&carrier(body), true).unwrap();
        for (role, explanation) in [
            (EMITTER, "\"worker-side seeded environment emitter: Math.random 32-bit header, keystream seeded from header ^ constant, interned-string/number writes of location.origin, navigator.userAgent, navigator.platform, navigator.hardwareConcurrency (literal null tag) and countOwnPropertiesAlongPrototypeChain(self), then the captured payload plus [[], header] posted through the unbound postMessage sink\""),
            (CONTAINER, "\"worker-posted environment bundle container: exactly four owned nested functions (accepted writeNumber, createInternedStringWriter, createKeystreamByteSource and the independently proven emitter), a 32-bit key derived from the captured string array on formal slot 1, and one emitter call with formal slot 1 data\""),
        ] {
            let outcome = result.report.outcomes.iter().find(|o| o.role_name.as_deref() == Some(role) && o.evaluator.as_deref() == Some(EVALUATOR)).expect("complete proof accepted");
            assert!(outcome.applied);
            assert_eq!(outcome.explanation, vec![explanation.trim_matches('"').to_string()]);
        }
    }
}
