function $THREAD_1_66217($A, $B) {
	let $KEY_2 = $A;
	let $KEY_10, $KEY_12, $KEY_13, $KEY_14, $KEY_15, $KEY_17;
	function $THREAD_8_78309($E) {
		let $KEY_0 = $E;
		let $KEY_1, $KEY_2;
		$KEY_2 = Object.create(null);
		$KEY_1 = 0;
		function $THREAD_5_30378($F, $G, $H) {
			let $KEY_3;
			if ($KEY_2[$F] != null) {
				$KEY_3 = $H;
				$KEY_3 = Number($H) + 1;
				$G[$KEY_3] = $KEY_0() ^ 129;
				$KEY_3 = $KEY_15($KEY_0, $KEY_2[$F], $G, $KEY_3);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_14($F[$J], 0);
					if ($L === 0) {
						$G[++$K] = $KEY_0() ^ 192;
						$L = 128;
					} else if (!($L < 128)) if ($L < 2048) {
						$G[++$K] = ($L >> 6 | 192) ^ $KEY_0();
						$L = $L & 63 | 128;
					} else {
						let $M = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_14($F[$J + 1], 0) : null;
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
				let $N = $KEY_2;
				let $O = Number($KEY_1);
				$KEY_1 = $O + 1;
				$N[$F] = $O;
			}
			return $KEY_3;
		}
		return $THREAD_5_30378;
	}
	$KEY_12 = $THREAD_8_78309;
	let $KEY_1;
	function $THREAD_9_121859($E, $F, $G, $H) {
		let $KEY_9;
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
				$KEY_9 = Number($H) + 1;
				$G[$KEY_9] = ($J | $K) ^ $E();
			} else if ($K < 4096) {
				$KEY_9 = Number($H) + 1;
				$G[$KEY_9] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_9] = $K >> 5 ^ $E();
			} else if ($K < 524288) {
				$KEY_9 = Number($H) + 1;
				$G[$KEY_9] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_9] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_9] = $K >>> 12 ^ $E();
			} else if ($K < 67108864) {
				$KEY_9 = Number($H) + 1;
				$G[$KEY_9] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_9] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_9] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_9] = $K >>> 19 ^ $E();
			} else {
				$KEY_9 = Number($H) + 1;
				$G[$KEY_9] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_9] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_9] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_9] = ($K >>> 19 & 127 | 128) ^ $E();
				$G[++$KEY_9] = $K >>> 26 ^ $E();
			}
		} else {
			$KEY_9 = $H;
			$KEY_9 = Number($H) + 1;
			$G[$KEY_9] = $E() ^ 128;
			let $L;
			if ($KEY_1) {
				$KEY_1[0] = $F;
				$L = new $KEY_17($KEY_1.buffer);
			} else if ($F !== $F) {
				$L = $KEY_10;
			} else {
				let $KEY_0 = $F < 0 || $F === 0 && 1 / $F < 0;
				let $M = Math.abs($F);
				let $N, $O;
				if ($M >= Math.pow(2, -1022)) {
					$N = Math.min(Math.floor(Math.log($M) / Math.LN2), 1023);
					$M = $M / Math.pow(2, $N);
					let $P = $M < 1;
					$N = $P ? $N - 1 : $N;
					$P = $P ? $M * 2 : $M;
					let $Q = $P >= 2;
					$N = $Q ? $N + 1 : $N;
					$Q = $Q ? $P / 2 : $P;
					let $KEY_11 = Math.pow(2, 52);
					let $R = Math.floor($Q * $KEY_11);
					if (!($Q * $KEY_11 - $R < .5)) if ($Q * $KEY_11 - $R > .5) {
						++$R;
					} else {
						$R = [$R, $R % 2];
						$R = $R[1] ? $R[0] + 1 : $R[0];
					}
					$O = ($R - $KEY_11) / $KEY_11 >= 1;
					$N = $O ? $N + 1024 : $N + 1023;
					$O = $O ? 0 : $R - $KEY_11;
					if ($N > 2046) {
						$N = 2047;
						$O = 0;
					}
				} else {
					$M = $M / Math.pow(2, -1074);
					$O = Math.floor($M);
					if (!($M - $O < .5)) if ($M - $O > .5) {
						++$O;
					} else {
						$O = [$O, $O % 2];
						$O = $O[1] ? $O[0] + 1 : $O[0];
					}
					$N = 0;
				}
				let $S = [];
				let k = 51;
				for (; k >= 0; k--) {
					$S.unshift(!!($O & 1));
					$O = Math.floor($O / 2);
				}
				let i = 11;
				let $T, $U;
				for (;; i--) {
					$T = $S;
					$U = $S;
					if (i <= 0) break;
					$U.unshift.call($T, !!($N & 1));
					$N = Math.floor($N / 2);
				}
				$U.unshift.call($T, $KEY_0);
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
				$G[++$KEY_9] = $L[i_2] ^ $E();
			}
		}
		return $KEY_9;
	}
	$KEY_15 = $THREAD_9_121859;
	function $THREAD_2_71171($E) {
		let $KEY_0, $KEY_1, $KEY_4, $KEY_5;
		function $THREAD_5_65933() {
			let $F = $KEY_0;
			$KEY_0 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_4 = $KEY_1 + $KEY_5;
				let $G = $KEY_5;
				$KEY_5 = $G ^ $KEY_1;
				$KEY_1 = $G ^ $KEY_1 ^ ($KEY_1 << 26 | $KEY_1 >>> 6) ^ $KEY_5 << 9;
				$KEY_5 = $KEY_5 << 13 | $KEY_5 >>> 19;
				return $KEY_4 & 255;
			}
			if (($F + 1) % 4 === 1) {
				return $KEY_4 >> 8 & 255;
			}
			if (($F + 1) % 4 === 2) {
				return $KEY_4 >> 16 & 255;
			}
			if (($F + 1) % 4 === 3) {
				return $KEY_4 >>> 24 & 255;
			} else {
				return void 0;
			}
		}
		$KEY_1 = $KEY_13;
		$KEY_5 = $E;
		$KEY_0 = -1;
		$THREAD_5_65933();
		$THREAD_5_65933();
		$THREAD_5_65933();
		$THREAD_5_65933();
		return $THREAD_5_65933;
	}
	let $KEY_0 = $THREAD_2_71171;
	Object.prototype.hasOwnProperty;
	$KEY_14 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_17 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_1 = $KEY_17 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_10 = $KEY_1 ? null : (($PARAMA, $PARAMB) => {
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
	$KEY_13 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_2_141560() {
		let $KEY_7;
		function $THREAD_1_107492($J) {
			if ($J == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($J).length + $KEY_7(Object.getPrototypeOf($J));
			}
		}
		$KEY_7 = $THREAD_1_107492;
		let $E = Math.random() * 4294967296;
		let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];
		$E = $KEY_0(($E | 0) ^ 2114204984);
		let $KEY_8 = $KEY_12($E);
		let $KEY_1 = navigator.hardwareConcurrency;
		if ($KEY_1 == null) {
			$F[4] = $E() ^ 130;
			$KEY_1 = 4;
		} else {
			$KEY_1 = $KEY_15($E, $KEY_1, $F, 3);
		}
		$KEY_1 = $KEY_8(location.origin, $F, $KEY_1);
		let $KEY_3 = $KEY_8(navigator.userAgent, $F, $KEY_1);
		$KEY_8 = $KEY_8(navigator.platform, $F, $KEY_3);
		$KEY_15($E, $KEY_7(self), $F, $KEY_8);
		let $G = postMessage;
		let $H = { a: $KEY_2 };
		let $I = [[], $F];
		$H.b = $I;
		$G($H);
	}
	$THREAD_2_141560($B.b);
}