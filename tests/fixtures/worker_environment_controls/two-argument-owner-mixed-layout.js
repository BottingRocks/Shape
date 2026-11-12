function $THREAD_9_58089($A, $B) {
	let $KEY_9 = $A;
	let $KEY_11, $KEY_14, $KEY_15, $KEY_17, $KEY_18, $KEY_3;
	function $THREAD_4_9280($E) {
		let $KEY_1, $KEY_4, $KEY_5, $KEY_6;
		function $THREAD_5_18988() {
			let $F = $KEY_5;
			$KEY_5 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_6 = $KEY_4 + $KEY_1;
				let $G = $KEY_1;
				$KEY_1 = $G ^ $KEY_4;
				$KEY_4 = $G ^ $KEY_4 ^ ($KEY_4 << 26 | $KEY_4 >>> 6) ^ $KEY_1 << 9;
				$KEY_1 = $KEY_1 << 13 | $KEY_1 >>> 19;
				return $KEY_6 & 255;
			}
			if (($F + 1) % 4 === 1) {
				return $KEY_6 >> 8 & 255;
			}
			if (($F + 1) % 4 === 2) {
				return $KEY_6 >> 16 & 255;
			}
			if (($F + 1) % 4 === 3) {
				return $KEY_6 >>> 24 & 255;
			} else {
				return void 0;
			}
		}
		$KEY_4 = $KEY_3;
		$KEY_1 = $E;
		$KEY_5 = -1;
		$THREAD_5_18988();
		$THREAD_5_18988();
		$THREAD_5_18988();
		$THREAD_5_18988();
		return $THREAD_5_18988;
	}
	let $KEY_0;
	function $THREAD_2_77334($E, $F, $G, $H) {
		let $KEY_8;
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
				$KEY_8 = Number($H) + 1;
				$G[$KEY_8] = ($K | $J) ^ $E();
			} else if ($J < 4096) {
				$KEY_8 = Number($H) + 1;
				$G[$KEY_8] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_8] = $J >> 5 ^ $E();
			} else if ($J < 524288) {
				$KEY_8 = Number($H) + 1;
				$G[$KEY_8] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_8] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_8] = $J >>> 12 ^ $E();
			} else if ($J < 67108864) {
				$KEY_8 = Number($H) + 1;
				$G[$KEY_8] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_8] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_8] = ($J >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_8] = $J >>> 19 ^ $E();
			} else {
				$KEY_8 = Number($H) + 1;
				$G[$KEY_8] = ($K | ($J & 31 | 32)) ^ $E();
				$G[++$KEY_8] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_8] = ($J >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_8] = ($J >>> 19 & 127 | 128) ^ $E();
				$G[++$KEY_8] = $J >>> 26 ^ $E();
			}
		} else {
			$KEY_8 = $H;
			$KEY_8 = Number($H) + 1;
			$G[$KEY_8] = $E() ^ 128;
			let $L;
			if ($KEY_0) {
				$KEY_0[0] = $F;
				$L = new $KEY_17($KEY_0.buffer);
			} else if ($F !== $F) {
				$L = $KEY_18;
			} else {
				let $KEY_5 = $F < 0 || $F === 0 && 1 / $F < 0;
				let $M = Math.abs($F);
				let $N, $O;
				if ($M >= Math.pow(2, -1022)) {
					let $KEY_10 = Math.min(Math.floor(Math.log($M) / Math.LN2), 1023);
					$M = $M / Math.pow(2, $KEY_10);
					let $P = $M < 1;
					$M = $P ? $M * 2 : $M;
					$P = $P ? $KEY_10 - 1 : $KEY_10;
					if ($M >= 2) {
						++$P;
						$M = $M / 2;
					}
					let $KEY_7 = Math.pow(2, 52);
					let $KEY_9 = Math.floor($M * $KEY_7);
					let $KEY_4 = ($M * $KEY_7 - $KEY_9 < .5 ? $KEY_9 : $M * $KEY_7 - $KEY_9 > .5 ? $KEY_9 + 1 : $KEY_9 % 2 ? $KEY_9 + 1 : $KEY_9) - $KEY_7;
					$P += 1023;
					let $Q = $KEY_4 / $KEY_7 >= 1;
					$O = $Q ? $P + 1 : $P;
					if ($O > 2046) {
						$N = 0;
						$O = 2047;
					} else {
						$N = $Q ? 0 : $KEY_4;
					}
				} else {
					$M = $M / Math.pow(2, -1074);
					let $R = Math.floor($M);
					$N = $M - $R < .5 ? $R : $M - $R > .5 ? $R + 1 : $R % 2 ? $R + 1 : $R;
					$O = 0;
				}
				let $S = [];
				let k = 51;
				for (; k >= 0; k--) {
					$S.unshift(!!($N & 1));
					$N = Math.floor($N / 2);
				}
				let i = 11;
				let $T, $U;
				for (;; i--) {
					$T = $S;
					$U = $S;
					if (i <= 0) break;
					$U.unshift.call($T, !!($O & 1));
					$O = Math.floor($O / 2);
				}
				$U.unshift.call($T, $KEY_5);
				let $V = [];
				let j = 56;
				let $W, $X;
				for (;; j = $X - 8) {
					$W = $V;
					$X = j;
					if (!(j >= 0)) break;
					$W = [$W, $W.push];
					$W[1].call($W[0], Number($S[$X]) << 7 | Number($S[$X + 1]) << 6 | Number($S[$X + 2]) << 5 | Number($S[$X + 3]) << 4 | Number($S[$X + 4]) << 3 | Number($S[$X + 5]) << 2 | Number($S[$X + 6]) << 1 | Number($S[$X + 7]));
				}
				$L = $W;
			}
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$G[++$KEY_8] = $L[i_2] ^ $E();
			}
		}
		return $KEY_8;
	}
	let $KEY_8;
	function $THREAD_7_142156($E) {
		let $KEY_1 = $E;
		let $KEY_2;
		$KEY_2 = Object.create(null);
		let $KEY_0 = 0;
		function $THREAD_6_35501($F, $G, $H) {
			let $KEY_4;
			if ($KEY_2[$F] != null) {
				$KEY_4 = $H;
				$KEY_4 = Number($H) + 1;
				$G[$KEY_4] = $KEY_1() ^ 129;
				$KEY_4 = $KEY_14($KEY_1, $KEY_2[$F], $G, $KEY_4);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_8($F[$J], 0);
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
							const $N = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_8($F[$J + 1], 0) : null;
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
				$KEY_4 = Number($K) + 1;
				$G[$KEY_4] = $KEY_1();
				let $O = $KEY_2;
				let $P = Number($KEY_0);
				$KEY_0 = $P + 1;
				$O[$F] = $P;
			}
			return $KEY_4;
		}
		return $THREAD_6_35501;
	}
	$KEY_15 = $THREAD_7_142156;
	$KEY_14 = $THREAD_2_77334;
	$KEY_11 = $THREAD_4_9280;
	Object.prototype.hasOwnProperty;
	$KEY_8 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_17 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_0 = $KEY_17 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_18 = $KEY_0 ? null : (($PARAMA, $PARAMB) => {
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
	$KEY_3 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_6_40205() {
		let $KEY_2;
		function $THREAD_4_59297($K) {
			if ($K == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($K).length + $KEY_2(Object.getPrototypeOf($K));
			}
		}
		$KEY_2 = $THREAD_4_59297;
		let $E = Math.random() * 4294967296;
		let $KEY_3 = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];
		let $KEY_4 = $KEY_11(($E | 0) ^ 3803249866);
		let $KEY_6 = $KEY_15($KEY_4);
		let $F = $KEY_14($KEY_4, $KEY_2(self), $KEY_3, 3);
		let $G = navigator.hardwareConcurrency;
		if ($G == null) {
			$KEY_3[++$F] = $KEY_4() ^ 130;
		} else {
			$F = $KEY_14($KEY_4, $G, $KEY_3, $F);
		}
		$F = $KEY_6(navigator.platform, $KEY_3, $F);
		let $KEY_0 = $KEY_6(location.origin, $KEY_3, $F);
		$KEY_6(navigator.userAgent, $KEY_3, $KEY_0);
		let $H = postMessage;
		let $I = { a: $KEY_9 };
		let $J = [[], $KEY_3];
		$I.b = $J;
		$H($I);
	}
	$THREAD_6_40205($B.b);
}