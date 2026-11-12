function $THREAD_6_134455($A, $B) {
	let $KEY_12 = $A;
	let $KEY_13, $KEY_16, $KEY_18;
	function $THREAD_6_121017($E) {
		let $KEY_0, $KEY_3, $KEY_4, $KEY_5;
		function $THREAD_9_23064() {
			let $F = $KEY_4;
			$KEY_4 = ($F + 1) % 4;
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
		$KEY_0 = $KEY_16;
		$KEY_5 = $E;
		$KEY_4 = -1;
		$THREAD_9_23064();
		$THREAD_9_23064();
		$THREAD_9_23064();
		$THREAD_9_23064();
		return $THREAD_9_23064;
	}
	let $KEY_7, $KEY_9, $KEY_11;
	function $THREAD_6_21303($E, $F, $G, $H) {
		let $KEY_12;
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
				$KEY_12 = Number($H) + 1;
				$G[$KEY_12] = ($J | $K) ^ $E();
			} else if ($K < 4096) {
				$KEY_12 = Number($H) + 1;
				$G[$KEY_12] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_12] = $K >> 5 ^ $E();
			} else if ($K < 524288) {
				$KEY_12 = Number($H) + 1;
				$G[$KEY_12] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_12] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_12] = $K >>> 12 ^ $E();
			} else if ($K < 67108864) {
				$KEY_12 = Number($H) + 1;
				$G[$KEY_12] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_12] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_12] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_12] = $K >>> 19 ^ $E();
			} else {
				$KEY_12 = Number($H) + 1;
				$G[$KEY_12] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_12] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_12] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_12] = ($K >>> 19 & 127 | 128) ^ $E();
				$G[++$KEY_12] = $K >>> 26 ^ $E();
			}
		} else {
			$KEY_12 = $H;
			$KEY_12 = Number($H) + 1;
			$G[$KEY_12] = $E() ^ 128;
			let $L;
			if ($KEY_9) {
				$KEY_9[0] = $F;
				$L = new $KEY_7($KEY_9.buffer);
			} else if ($F !== $F) {
				$L = $KEY_11;
			} else {
				let $KEY_4 = $F < 0 || $F === 0 && 1 / $F < 0;
				let $M = Math.abs($F);
				let $N, $O;
				if ($M >= Math.pow(2, -1022)) {
					let $P = Math.min(Math.floor(Math.log($M) / Math.LN2), 1023);
					$M = $M / Math.pow(2, $P);
					let $Q = $M < 1;
					$P = $Q ? $P - 1 : $P;
					$Q = $Q ? $M * 2 : $M;
					let $R = $Q >= 2;
					$Q = $R ? $Q / 2 : $Q;
					let $S = Math.pow(2, 52);
					let $T = Math.floor($Q * $S);
					if (!($Q * $S - $T < .5)) if ($Q * $S - $T > .5) {
						++$T;
					} else {
						$T = [$T, $T % 2];
						$T = $T[1] ? $T[0] + 1 : $T[0];
					}
					$R = ($R ? $P + 1 : $P) + 1023;
					$O = ($T - $S) / $S >= 1;
					$N = $O ? $R + 1 : $R;
					$O = $O ? 0 : $T - $S;
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
				let $U = [];
				let k = 51;
				for (; k >= 0; k--) {
					$U.unshift(!!($O & 1));
					$O = Math.floor($O / 2);
				}
				let i = 11;
				for (; i > 0; i--) {
					$U.unshift(!!($N & 1));
					$N = Math.floor($N / 2);
				}
				$U.unshift($KEY_4);
				$L = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					$L.push(Number($U[j]) << 7 | Number($U[j + 1]) << 6 | Number($U[j + 2]) << 5 | Number($U[j + 3]) << 4 | Number($U[j + 4]) << 3 | Number($U[j + 5]) << 2 | Number($U[j + 6]) << 1 | Number($U[j + 7]));
				}
			}
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$G[++$KEY_12] = $L[i_2] ^ $E();
			}
		}
		return $KEY_12;
	}
	let $KEY_0;
	function $THREAD_1_1366($E) {
		let $KEY_3 = $E;
		let $KEY_2 = Object.create(null);
		let $KEY_1 = 0;
		function $THREAD_0_103070($F, $G, $H) {
			let $KEY_8;
			if ($KEY_2[$F] != null) {
				$KEY_8 = $H;
				$KEY_8 = Number($H) + 1;
				$G[$KEY_8] = $KEY_3() ^ 129;
				$KEY_8 = $KEY_0($KEY_3, $KEY_2[$F], $G, $KEY_8);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_13($F[$J], 0);
					let $M;
					if ($L === 0) {
						$G[++$K] = $KEY_3() ^ 192;
						$L = $KEY_3();
						$M = 128;
					} else {
						if (!($L < 128)) if ($L < 2048) {
							$G[++$K] = ($L >> 6 | 192) ^ $KEY_3();
							$L = $L & 63 | 128;
						} else {
							let $N = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_13($F[$J + 1], 0) : null;
							if ($N == null || $N < 56320 || $N > 57343) {
								$G[++$K] = ($L >> 12 | 224) ^ $KEY_3();
								$G[++$K] = ($L >> 6 & 63 | 128) ^ $KEY_3();
								$L = $L & 63 | 128;
							} else {
								++$J;
								$G[++$K] = (65536 + ($L - 55296 << 10) + ($N - 56320) >> 18 | 240) ^ $KEY_3();
								$G[++$K] = (65536 + ($L - 55296 << 10) + ($N - 56320) >> 12 & 63 | 128) ^ $KEY_3();
								$G[++$K] = (65536 + ($L - 55296 << 10) + ($N - 56320) >> 6 & 63 | 128) ^ $KEY_3();
								$L = 65536 + ($L - 55296 << 10) + ($N - 56320) & 63 | 128;
							}
						}
						$M = $KEY_3();
					}
					$G[++$K] = $L ^ $M;
					++$J;
				}
				$KEY_8 = Number($K) + 1;
				$G[$KEY_8] = $KEY_3();
				let $O = $KEY_2;
				let $P = Number($KEY_1);
				$KEY_1 = $P + 1;
				$O[$F] = $P;
			}
			return $KEY_8;
		}
		return $THREAD_0_103070;
	}
	let $KEY_4 = $THREAD_1_1366;
	$KEY_0 = $THREAD_6_21303;
	$KEY_18 = $THREAD_6_121017;
	Object.prototype.hasOwnProperty;
	$KEY_13 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_7 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_9 = $KEY_7 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_11 = $KEY_9 ? null : [0, 0, 0, 0, 0, 0, 248, 127];
	let $C = $B.a.length >> 1 << 1;
	let i = 0;
	let $D = 0;
	for (; i < $C; i = i + 2) {
		$D = ($D << 5) - $D + ($B.a[i].charCodeAt() << 16 | $B.a[i + 1].charCodeAt()) | 0;
	}
	$KEY_16 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_4_143124() {
		let $KEY_6;
		function $THREAD_0_135938($H) {
			if ($H == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($H).length + $KEY_6(Object.getPrototypeOf($H));
			}
		}
		$KEY_6 = $THREAD_0_135938;
		let $KEY_9 = Math.random() * 4294967296 | 0;
		let $KEY_8 = [$KEY_9 >>> 24, $KEY_9 >> 16 & 255, $KEY_9 >> 8 & 255, $KEY_9 & 255];
		let $KEY_7 = $KEY_18($KEY_9 ^ 3438527792);
		let $KEY_5 = $KEY_4($KEY_7);
		let $E = navigator.hardwareConcurrency;
		if ($E == null) {
			$KEY_8[4] = $KEY_7() ^ 130;
			$E = 4;
		} else {
			$E = $KEY_0($KEY_7, $E, $KEY_8, 3);
		}
		$KEY_7 = $KEY_0($KEY_7, $KEY_6(self), $KEY_8, $E);
		$KEY_7 = $KEY_5(navigator.platform, $KEY_8, $KEY_7);
		let $KEY_2 = $KEY_5(navigator.userAgent, $KEY_8, $KEY_7);
		$KEY_5(location.origin, $KEY_8, $KEY_2);
		let $F = postMessage;
		let $G = { a: $KEY_12 };
		$KEY_8 = [[], $KEY_8];
		$G.b = $KEY_8;
		$F($G);
	}
	$THREAD_4_143124($B.b);
}