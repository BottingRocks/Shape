function $THREAD_0_115855($A, $B) {
	let $KEY_1 = $A;
	let $KEY_2, $KEY_3, $KEY_6, $KEY_8, $KEY_16, $KEY_17, $KEY_18;
	function $THREAD_6_3902($E) {
		let $KEY_0 = $E;
		let $KEY_1, $KEY_2;
		$KEY_1 = Object.create(null);
		$KEY_2 = 0;
		function $THREAD_9_159482($F, $G, $H) {
			let $KEY_3;
			if ($KEY_1[$F] != null) {
				$KEY_3 = $H;
				$G[++$KEY_3] = $KEY_0() ^ 129;
				$KEY_3 = $KEY_17($KEY_0, $KEY_1[$F], $G, $KEY_3);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_8($F[$J], 0);
					if ($L === 0) {
						$G[++$K] = $KEY_0() ^ 192;
						$L = 128;
					} else if (!($L < 128)) if ($L < 2048) {
						$G[++$K] = ($L >> 6 | 192) ^ $KEY_0();
						$L = $L & 63 | 128;
					} else {
						const $M = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_8($F[$J + 1], 0) : null;
						if ($M == null || $M < 56320 || $M > 57343) {
							$G[++$K] = ($L >> 12 | 224) ^ $KEY_0();
							$G[++$K] = ($L >> 6 & 63 | 128) ^ $KEY_0();
							$L = $L & 63 | 128;
						} else {
							++$J;
							$G[++$K] = (65536 + ($L - 55296 << 10) + ($M - 56320) >> 18 | 240) ^ $KEY_0();
							$G[++$K] = (65536 + ($L - 55296 << 10) + ($M - 56320) >> 12 & 63 | 128) ^ $KEY_0();
							$G[++$K] = (65536 + ($L - 55296 << 10) + ($M - 56320) >> 6 & 63 | 128) ^ $KEY_0();
							$L = 65536 + ($L - 55296 << 10) + ($M - 56320) & 63 | 128;
						}
					}
					$G[++$K] = $L ^ $KEY_0();
					++$J;
				}
				$KEY_3 = Number($K) + 1;
				$G[$KEY_3] = $KEY_0();
				let $N = $KEY_1;
				let $O = Number($KEY_2);
				$KEY_2 = $O + 1;
				$N[$F] = $O;
			}
			return $KEY_3;
		}
		return $THREAD_9_159482;
	}
	$KEY_6 = $THREAD_6_3902;
	function $THREAD_4_5373($E, $F, $G, $H) {
		let $KEY_11;
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
				$J = -$F;
				$K = 64;
			} else {
				$J = $F;
				$K = 0;
			}
			if ($J < 32) {
				$KEY_11 = Number($H) + 1;
				$G[$KEY_11] = ($K | $J) ^ $E();
			} else if ($J < 4096) {
				let $L = Number($H) + 1;
				$G[$L] = ($K | ($J & 31 | 32)) ^ $E();
				$G[$L + 1] = $J >> 5 ^ $E();
				$KEY_11 = $L + 1;
			} else if ($J < 524288) {
				let $M = Number($H) + 1;
				$G[$M] = ($K | ($J & 31 | 32)) ^ $E();
				$G[$M + 1] = ($J >> 5 & 127 | 128) ^ $E();
				$G[$M + 2] = $J >>> 12 ^ $E();
				$KEY_11 = $M + 2;
			} else if ($J < 67108864) {
				let $N = Number($H) + 1;
				$G[$N] = ($K | ($J & 31 | 32)) ^ $E();
				$G[$N + 1] = ($J >> 5 & 127 | 128) ^ $E();
				$G[$N + 2] = ($J >>> 12 & 127 | 128) ^ $E();
				$G[$N + 3] = $J >>> 19 ^ $E();
				$KEY_11 = $N + 3;
			} else {
				$KEY_11 = Number($H) + 1;
				$G[$KEY_11] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_11] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_11] = ($J >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_11] = ($J >>> 19 & 127 | 128) ^ $E();
				$G[++$KEY_11] = $J >>> 26 ^ $E();
			}
		} else {
			$KEY_11 = Number($H) + 1;
			$G[$KEY_11] = $E() ^ 128;
			let $O;
			if ($KEY_18) {
				$KEY_18[0] = $F;
				$O = new $KEY_2($KEY_18.buffer);
			} else if ($F !== $F) {
				$O = $KEY_3;
			} else {
				let $KEY_5 = $F < 0 || $F === 0 && 1 / $F < 0;
				let $P = Math.abs($F);
				let $Q;
				if ($P >= Math.pow(2, -1022)) {
					$Q = Math.min(Math.floor(Math.log($P) / Math.LN2), 1023);
					$P = $P / Math.pow(2, $Q);
					let $R = $P < 1;
					$Q = $R ? $Q - 1 : $Q;
					$R = $R ? $P * 2 : $P;
					let $S = $R >= 2;
					$Q = $S ? $Q + 1 : $Q;
					$S = $S ? $R / 2 : $R;
					let $KEY_12 = Math.pow(2, 52);
					let $T = Math.floor($S * $KEY_12);
					$T = $S * $KEY_12 - $T < .5 ? $T : $S * $KEY_12 - $T > .5 ? $T + 1 : $T % 2 ? $T + 1 : $T;
					let $U = ($T - $KEY_12) / $KEY_12 >= 1;
					$Q = $U ? $Q + 1024 : $Q + 1023;
					if ($Q > 2046) {
						$Q = 2047;
						$P = 0;
					} else {
						$P = $U ? 0 : $T - $KEY_12;
					}
				} else {
					$P = $P / Math.pow(2, -1074);
					let $V = Math.floor($P);
					$Q = 0;
					$P = $P - $V < .5 ? $V : $P - $V > .5 ? $V + 1 : $V % 2 ? $V + 1 : $V;
				}
				let $W = [];
				let k = 51;
				for (; k >= 0; k--) {
					$W.unshift(!!($P & 1));
					$P = Math.floor($P / 2);
				}
				let i = 11;
				for (; i > 0; i--) {
					$W.unshift(!!($Q & 1));
					$Q = Math.floor($Q / 2);
				}
				$W.unshift($KEY_5);
				$O = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					$O.push(Number($W[j]) << 7 | Number($W[j + 1]) << 6 | Number($W[j + 2]) << 5 | Number($W[j + 3]) << 4 | Number($W[j + 4]) << 3 | Number($W[j + 5]) << 2 | Number($W[j + 6]) << 1 | Number($W[j + 7]));
				}
			}
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$G[++$KEY_11] = $O[i_2] ^ $E();
			}
		}
		return $KEY_11;
	}
	$KEY_17 = $THREAD_4_5373;
	let $KEY_0;
	function $THREAD_5_135203($E) {
		let $KEY_1, $KEY_2, $KEY_4, $KEY_5;
		function $THREAD_3_135242() {
			let $F = $KEY_2;
			$KEY_2 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_1 = $KEY_4 + $KEY_5;
				let $G = $KEY_5;
				$KEY_5 = $G ^ $KEY_4;
				$KEY_4 = $G ^ $KEY_4 ^ ($KEY_4 << 26 | $KEY_4 >>> 6) ^ $KEY_5 << 9;
				$KEY_5 = $KEY_5 << 13 | $KEY_5 >>> 19;
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
		$KEY_4 = $KEY_0;
		$KEY_5 = $E;
		$KEY_2 = -1;
		$THREAD_3_135242();
		$THREAD_3_135242();
		$THREAD_3_135242();
		$THREAD_3_135242();
		return $THREAD_3_135242;
	}
	$KEY_16 = $THREAD_5_135203;
	Object.prototype.hasOwnProperty;
	$KEY_8 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_2 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_18 = $KEY_2 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_3 = $KEY_18 ? null : (($PARAMA, $PARAMB) => {
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
	$KEY_0 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_4_137203() {
		let $KEY_5;
		function $THREAD_6_4173($L) {
			if ($L == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($L).length + $KEY_5(Object.getPrototypeOf($L));
			}
		}
		$KEY_5 = $THREAD_6_4173;
		let $E = Math.random() * 4294967296;
		let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];
		let $KEY_0 = $KEY_16(($E | 0) ^ 329299359);
		let $KEY_2 = $KEY_6($KEY_0);
		let $G = $KEY_2(navigator.userAgent, $F, 3);
		$G = $KEY_17($KEY_0, $KEY_5(self), $F, $G);
		let $H = navigator.hardwareConcurrency;
		if ($H == null) {
			$H = $G + 1;
			$F[$H] = $KEY_0() ^ 130;
		} else {
			$H = $KEY_17($KEY_0, $H, $F, $G);
		}
		$H = $KEY_2(navigator.platform, $F, $H);
		$KEY_2(location.origin, $F, $H);
		let $I = postMessage;
		let $J = { a: $KEY_1 };
		let $K = [[], $F];
		$J.b = $K;
		$I($J);
	}
	$THREAD_4_137203($B.b);
}