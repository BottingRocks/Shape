function $THREAD_8_13514($A, $B) {
	let $KEY_12 = $A;
	let $KEY_13, $KEY_18, $KEY_6, $KEY_10;
	function $THREAD_9_123161($E, $F, $G, $H) {
		let $KEY_4;
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
				$KEY_4 = Number($H) + 1;
				$G[$KEY_4] = ($K | $J) ^ $E();
			} else if ($J < 4096) {
				let $L = Number($H) + 1;
				$G[$L] = ($K | ($J & 31 | 32)) ^ $E();
				$KEY_4 = $L + 1;
				$G[$KEY_4] = $J >> 5 ^ $E();
			} else if ($J < 524288) {
				let $M = Number($H) + 1;
				$G[$M] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$M] = ($J >> 5 & 127 | 128) ^ $E();
				$KEY_4 = $M + 1;
				$G[$KEY_4] = $J >>> 12 ^ $E();
			} else if ($J < 67108864) {
				let $N = Number($H) + 1;
				$G[$N] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$N] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$N] = ($J >>> 12 & 127 | 128) ^ $E();
				$KEY_4 = $N + 1;
				$G[$KEY_4] = $J >>> 19 ^ $E();
			} else {
				let $O = Number($H) + 1;
				$G[$O] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$O] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$O] = ($J >>> 12 & 127 | 128) ^ $E();
				$G[++$O] = ($J >>> 19 & 127 | 128) ^ $E();
				$KEY_4 = $O + 1;
				$G[$KEY_4] = $J >>> 26 ^ $E();
			}
		} else {
			$KEY_4 = Number($H) + 1;
			$G[$KEY_4] = $E() ^ 128;
			let $P;
			if ($KEY_13) {
				$KEY_13[0] = $F;
				$P = new $KEY_6($KEY_13.buffer);
			} else if ($F !== $F) {
				$P = $KEY_10;
			} else {
				let $KEY_0 = $F < 0 || $F === 0 && 1 / $F < 0;
				let $Q = Math.abs($F);
				let $R, $KEY_11;
				if ($Q >= Math.pow(2, -1022)) {
					$R = Math.min(Math.floor(Math.log($Q) / Math.LN2), 1023);
					$Q = $Q / Math.pow(2, $R);
					let $S = $Q < 1;
					$R = $S ? $R - 1 : $R;
					$S = $S ? $Q * 2 : $Q;
					if ($S >= 2) {
						++$R;
						$S = $S / 2;
					}
					$KEY_11 = Math.pow(2, 52);
					$S = $S * $KEY_11;
					let $T = Math.floor($S);
					if (!($S - $T < .5)) if ($S - $T > .5) {
						++$T;
					} else {
						$T = [$T, $T % 2];
						$T = $T[1] ? $T[0] + 1 : $T[0];
					}
					$T = $T - $KEY_11;
					$KEY_11 = $T / $KEY_11 >= 1;
					$R = $KEY_11 ? $R + 1024 : $R + 1023;
					if ($R > 2046) {
						$R = 2047;
						$KEY_11 = 0;
					} else {
						$KEY_11 = $KEY_11 ? 0 : $T;
					}
				} else {
					$Q = $Q / Math.pow(2, -1074);
					$KEY_11 = Math.floor($Q);
					if (!($Q - $KEY_11 < .5)) if ($Q - $KEY_11 > .5) {
						++$KEY_11;
					} else {
						$KEY_11 = [$KEY_11, $KEY_11 % 2];
						$KEY_11 = $KEY_11[1] ? $KEY_11[0] + 1 : $KEY_11[0];
					}
					$R = 0;
				}
				let $U = [];
				let k = 51;
				for (; k >= 0; k--) {
					$U.unshift(!!($KEY_11 & 1));
					$KEY_11 = Math.floor($KEY_11 / 2);
				}
				let i = 11;
				for (; i > 0; i--) {
					$U.unshift(!!($R & 1));
					$R = Math.floor($R / 2);
				}
				$U.unshift($KEY_0);
				$P = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					let $V = [$P, $P.push];
					$V[1].call($V[0], Number($U[j]) << 7 | Number($U[j + 1]) << 6 | Number($U[j + 2]) << 5 | Number($U[j + 3]) << 4 | Number($U[j + 4]) << 3 | Number($U[j + 5]) << 2 | Number($U[j + 6]) << 1 | Number($U[j + 7]));
				}
			}
			let $W = $G;
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$W[++$KEY_4] = $P[i_2] ^ $E();
			}
		}
		return $KEY_4;
	}
	let $KEY_2, $KEY_4;
	function $THREAD_9_149890($E) {
		let $KEY_1 = $E;
		let $KEY_3;
		$KEY_3 = Object.create(null);
		let $KEY_0 = 0;
		function $THREAD_5_11435($F, $G, $H) {
			let $KEY_5;
			if ($KEY_3[$F] != null) {
				$KEY_5 = $H;
				$KEY_5 = Number($H) + 1;
				$G[$KEY_5] = $KEY_1() ^ 129;
				$KEY_5 = $KEY_4($KEY_1, $KEY_3[$F], $G, $KEY_5);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_2($F[$J], 0);
					let $M;
					if ($L === 0) {
						$G[++$K] = $KEY_1() ^ 192;
						$L = $KEY_1();
						$M = 128;
					} else {
						if (!($L < 128)) if ($L < 2048) {
							$G[++$K] = ($L >> 6 | 192) ^ $KEY_1();
							$L = $L & 63 | 128;
						} else {
							const $N = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_2($F[$J + 1], 0) : null;
							if ($N == null || $N < 56320 || $N > 57343) {
								$G[++$K] = ($L >> 12 | 224) ^ $KEY_1();
								$G[++$K] = ($L >> 6 & 63 | 128) ^ $KEY_1();
								$L = $L & 63 | 128;
							} else {
								++$J;
								$G[++$K] = (65536 + ($L - 55296 << 10) + ($N - 56320) >> 18 | 240) ^ $KEY_1();
								$G[++$K] = (65536 + ($L - 55296 << 10) + ($N - 56320) >> 12 & 63 | 128) ^ $KEY_1();
								$G[++$K] = (65536 + ($L - 55296 << 10) + ($N - 56320) >> 6 & 63 | 128) ^ $KEY_1();
								$L = 65536 + ($L - 55296 << 10) + ($N - 56320) & 63 | 128;
							}
						}
						$M = $KEY_1();
					}
					$G[++$K] = $L ^ $M;
					++$J;
				}
				$KEY_5 = Number($K) + 1;
				$G[$KEY_5] = $KEY_1();
				let $O = $KEY_3;
				let $P = Number($KEY_0);
				$KEY_0 = $P + 1;
				$O[$F] = $P;
			}
			return $KEY_5;
		}
		return $THREAD_5_11435;
	}
	$KEY_18 = $THREAD_9_149890;
	$KEY_4 = $THREAD_9_123161;
	let $KEY_9;
	function $THREAD_8_92650($E) {
		let $KEY_2, $KEY_3, $KEY_4, $KEY_6;
		function $THREAD_1_23011() {
			let $F = $KEY_3;
			$KEY_3 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_2 = $KEY_4 + $KEY_6;
				let $G = $KEY_6;
				$KEY_6 = $G ^ $KEY_4;
				$KEY_4 = $G ^ $KEY_4 ^ ($KEY_4 << 26 | $KEY_4 >>> 6) ^ $KEY_6 << 9;
				$KEY_6 = $KEY_6 << 13 | $KEY_6 >>> 19;
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
		$KEY_4 = $KEY_9;
		$KEY_6 = $E;
		$KEY_3 = -1;
		$THREAD_1_23011();
		$THREAD_1_23011();
		$THREAD_1_23011();
		$THREAD_1_23011();
		return $THREAD_1_23011;
	}
	let $KEY_7 = $THREAD_8_92650;
	Object.prototype.hasOwnProperty;
	$KEY_2 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_6 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_13 = $KEY_6 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_10 = $KEY_13 ? null : [0, 0, 0, 0, 0, 0, 248, 127];
	let $C = $B.a.length >> 1 << 1;
	let i = 0;
	let $D = 0;
	for (; i < $C; i = i + 2) {
		$D = ($D << 5) - $D + ($B.a[i].charCodeAt() << 16 | $B.a[i + 1].charCodeAt()) | 0;
	}
	$KEY_9 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_0_109848() {
		let $KEY_5;
		function $THREAD_1_150157($K) {
			if ($K == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($K).length + $KEY_5(Object.getPrototypeOf($K));
			}
		}
		$KEY_5 = $THREAD_1_150157;
		let $E = Math.random() * 4294967296;
		let $KEY_3 = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];
		let $KEY_1 = $KEY_7(($E | 0) ^ 4034127039);
		let $F = $KEY_18($KEY_1);
		let $G = $F(location.origin, $KEY_3, 3);
		$G = $F(navigator.userAgent, $KEY_3, $G);
		let $KEY_0 = navigator.hardwareConcurrency;
		if ($KEY_0 == null) {
			$KEY_0 = $G + 1;
			$KEY_3[$KEY_0] = $KEY_1() ^ 130;
		} else {
			$KEY_0 = $KEY_4($KEY_1, $KEY_0, $KEY_3, $G);
		}
		$F = $F(navigator.platform, $KEY_3, $KEY_0);
		$KEY_4($KEY_1, $KEY_5(self), $KEY_3, $F);
		let $H = postMessage;
		let $I = { a: $KEY_12 };
		let $J = [[], $KEY_3];
		$I.b = $J;
		$H($I);
	}
	$THREAD_0_109848($B.b);
}