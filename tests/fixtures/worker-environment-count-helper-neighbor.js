function $THREAD_7_93340($A, $B) {
	let $KEY_14 = $A;
	let $KEY_8, $KEY_12, $KEY_13;
	function $THREAD_5_93671($E, $F, $G, $H) {
		let $KEY_10;
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
				$KEY_10 = Number($H) + 1;
				$G[$KEY_10] = ($J | $K) ^ $E();
			} else if ($K < 4096) {
				$KEY_10 = Number($H) + 1;
				$G[$KEY_10] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_10] = $K >> 5 ^ $E();
			} else if ($K < 524288) {
				$KEY_10 = Number($H) + 1;
				$G[$KEY_10] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_10] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_10] = $K >>> 12 ^ $E();
			} else if ($K < 67108864) {
				$KEY_10 = Number($H) + 1;
				$G[$KEY_10] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_10] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_10] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_10] = $K >>> 19 ^ $E();
			} else {
				$KEY_10 = Number($H) + 1;
				$G[$KEY_10] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_10] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_10] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_10] = ($K >>> 19 & 127 | 128) ^ $E();
				$G[++$KEY_10] = $K >>> 26 ^ $E();
			}
		} else {
			$KEY_10 = $H;
			$KEY_10 = Number($H) + 1;
			$G[$KEY_10] = $E() ^ 128;
			let $L;
			if ($KEY_8) {
				$KEY_8[0] = $F;
				$L = new $KEY_12($KEY_8.buffer);
			} else if ($F !== $F) {
				$L = $KEY_13;
			} else {
				let $KEY_1 = $F < 0 || $F === 0 && 1 / $F < 0;
				let $M = Math.abs($F);
				let $N, $O;
				if ($M >= Math.pow(2, -1022)) {
					let $KEY_9 = Math.min(Math.floor(Math.log($M) / Math.LN2), 1023);
					$M = $M / Math.pow(2, $KEY_9);
					$O = $M < 1;
					$M = $O ? $M * 2 : $M;
					$O = $O ? $KEY_9 - 1 : $KEY_9;
					if ($M >= 2) {
						++$O;
						$M = $M / 2;
					}
					let $KEY_4 = Math.pow(2, 52);
					$M = $M * $KEY_4;
					let $P = Math.floor($M);
					$M = $M - $P < .5 ? $P : $M - $P > .5 ? $P + 1 : $P % 2 ? $P + 1 : $P;
					let $Q = ($M - $KEY_4) / $KEY_4 >= 1;
					$O = $Q ? $O + 1024 : $O + 1023;
					if ($O > 2046) {
						$N = 0;
						$O = 2047;
					} else {
						$N = $Q ? 0 : $M - $KEY_4;
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
				for (; i > 0; i--) {
					$S.unshift(!!($O & 1));
					$O = Math.floor($O / 2);
				}
				$S.unshift($KEY_1);
				$L = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					let $T = [$L, $L.push];
					$T[1].call($T[0], Number($S[j]) << 7 | Number($S[j + 1]) << 6 | Number($S[j + 2]) << 5 | Number($S[j + 3]) << 4 | Number($S[j + 4]) << 3 | Number($S[j + 5]) << 2 | Number($S[j + 6]) << 1 | Number($S[j + 7]));
				}
			}
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$G[++$KEY_10] = $L[i_2] ^ $E();
			}
		}
		return $KEY_10;
	}
	let $KEY_1, $KEY_2;
	function $THREAD_6_129153($E) {
		let $KEY_0 = $E;
		let $KEY_4, $KEY_6;
		$KEY_6 = Object.create(null);
		$KEY_4 = 0;
		function $THREAD_4_68077($F, $G, $H) {
			let $KEY_11;
			if ($KEY_6[$F] != null) {
				$KEY_11 = $H;
				$G[++$KEY_11] = $KEY_0() ^ 129;
				$KEY_11 = $KEY_2($KEY_0, $KEY_6[$F], $G, $KEY_11);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_1($F[$J], 0);
					if ($L === 0) {
						$G[++$K] = $KEY_0() ^ 192;
						$L = 128;
					} else if (!($L < 128)) if ($L < 2048) {
						$G[++$K] = ($L >> 6 | 192) ^ $KEY_0();
						$L = $L & 63 | 128;
					} else {
						let $M = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_1($F[$J + 1], 0) : null;
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
				$KEY_11 = Number($K) + 1;
				$G[$KEY_11] = $KEY_0();
				let $N = $KEY_6;
				let $O = Number($KEY_4);
				$KEY_4 = $O + 1;
				$N[$F] = $O;
			}
			return $KEY_11;
		}
		return $THREAD_4_68077;
	}
	let $KEY_7 = $THREAD_6_129153;
	$KEY_2 = $THREAD_5_93671;
	let $KEY_5;
	function $THREAD_4_78249($E) {
		let $KEY_0, $KEY_1, $KEY_2, $KEY_3;
		function $THREAD_9_110333() {
			let $F = $KEY_0;
			$KEY_0 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_2 = $KEY_1 + $KEY_3;
				let $G = $KEY_3;
				$KEY_3 = $G ^ $KEY_1;
				$KEY_1 = $G ^ $KEY_1 ^ ($KEY_1 << 26 | $KEY_1 >>> 6) ^ $KEY_3 << 9;
				$KEY_3 = $KEY_3 << 13 | $KEY_3 >>> 19;
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
		$KEY_1 = $KEY_5;
		$KEY_3 = $E;
		$KEY_0 = -1;
		$THREAD_9_110333();
		$THREAD_9_110333();
		$THREAD_9_110333();
		$THREAD_9_110333();
		return $THREAD_9_110333;
	}
	let $KEY_4 = $THREAD_4_78249;
	Object.prototype.hasOwnProperty;
	$KEY_1 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_12 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_8 = $KEY_12 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_13 = $KEY_8 ? null : [0, 0, 0, 0, 0, 0, 248, 127];
	let $C = $B.a.length >> 1 << 1;
	let i = 0;
	let $D = 0;
	for (; i < $C; i = i + 2) {
		$D = ($D << 5) - $D + ($B.a[i].charCodeAt() << 16 | $B.a[i + 1].charCodeAt()) | 0;
	}
	$KEY_5 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_2_34779() {
		let $KEY_1;
		function $THREAD_0_93116($K) {
			if ($K == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($K).length + $KEY_1(Object.getPrototypeOf($K));
			}
		}
		$KEY_1 = $THREAD_0_93116;
		let $E = Math.random() * 4294967296;
		let $KEY_6 = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];
		let $KEY_3 = $KEY_4(($E | 0) ^ 1210093294);
		let $F = $KEY_7($KEY_3);
		let $G = $F(navigator.platform, $KEY_6, 3);
		$G = $F(navigator.userAgent, $KEY_6, $G);
		$G = $KEY_2($KEY_3, $KEY_1(self), $KEY_6, $G);
		let $H = navigator.hardwareConcurrency;
		if ($H == null) {
			$KEY_6[++$G] = $KEY_3() ^ 130;
		} else {
			$G = $KEY_2($KEY_3, $H, $KEY_6, $G);
		}
		$F(location.origin, $KEY_6, $G);
		let $I = postMessage;
		let $J = { a: $KEY_14 };
		$KEY_6 = [[], $KEY_6];
		$J.b = $KEY_6;
		$I($J);
	}
	$THREAD_2_34779($B.b);
}