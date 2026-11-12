function $THREAD_0_31722($A, $B) {
	let $KEY_4 = $A;
	let $KEY_5, $KEY_6, $KEY_12, $KEY_13, $KEY_14, $KEY_18;
	function $THREAD_7_149193($E) {
		let $KEY_0 = $E;
		let $KEY_1, $KEY_2;
		$KEY_2 = Object.create(null);
		$KEY_1 = 0;
		function $THREAD_4_45420($F, $G, $H) {
			let $KEY_9;
			if ($KEY_2[$F] != null) {
				$KEY_9 = $H;
				$KEY_9 = Number($H) + 1;
				$G[$KEY_9] = $KEY_0() ^ 129;
				$KEY_9 = $KEY_14($KEY_0, $KEY_2[$F], $G, $KEY_9);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_13($F[$J], 0);
					if ($L === 0) {
						$G[++$K] = $KEY_0() ^ 192;
						$L = 128;
					} else if (!($L < 128)) if ($L < 2048) {
						$G[++$K] = ($L >> 6 | 192) ^ $KEY_0();
						$L = $L & 63 | 128;
					} else {
						let $M = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_13($F[$J + 1], 0) : null;
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
				$KEY_9 = Number($K) + 1;
				$G[$KEY_9] = $KEY_0();
				let $N = $KEY_2;
				let $O = Number($KEY_1);
				$KEY_1 = $O + 1;
				$N[$F] = $O;
			}
			return $KEY_9;
		}
		return $THREAD_4_45420;
	}
	let $KEY_2 = $THREAD_7_149193;
	let $KEY_1;
	function $THREAD_0_29466($E, $F, $G, $H) {
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
				let $L = Number($H) + 1;
				$G[$L] = ($J | ($K & 31 | 32)) ^ $E();
				$KEY_9 = $L + 1;
				$G[$KEY_9] = $K >> 5 ^ $E();
			} else if ($K < 524288) {
				let $M = Number($H) + 1;
				$G[$M] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$M] = ($K >> 5 & 127 | 128) ^ $E();
				$KEY_9 = $M + 1;
				$G[$KEY_9] = $K >>> 12 ^ $E();
			} else if ($K < 67108864) {
				let $N = Number($H) + 1;
				$G[$N] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$N] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$N] = ($K >>> 12 & 127 | 128) ^ $E();
				$KEY_9 = $N + 1;
				$G[$KEY_9] = $K >>> 19 ^ $E();
			} else {
				let $KEY_10 = Number($H) + 1;
				$G[$KEY_10] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_10] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_10] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_10] = ($K >>> 19 & 127 | 128) ^ $E();
				$KEY_9 = $KEY_10 + 1;
				$G[$KEY_9] = $K >>> 26 ^ $E();
			}
		} else {
			let $O = $G;
			$KEY_9 = Number($H) + 1;
			$O[$KEY_9] = $E() ^ 128;
			let $P;
			if ($KEY_18) {
				$KEY_18[0] = $F;
				$P = new $KEY_1($KEY_18.buffer);
			} else if ($F !== $F) {
				$P = $KEY_6;
			} else {
				const $Q = $F < 0 || $F === 0 && 1 / $F < 0;
				let $R = Math.abs($F);
				let $S, $T;
				if ($R >= Math.pow(2, -1022)) {
					let $U = Math.min(Math.floor(Math.log($R) / Math.LN2), 1023);
					$R = $R / Math.pow(2, $U);
					let $V = $R < 1;
					$U = $V ? $U - 1 : $U;
					$V = $V ? $R * 2 : $R;
					let $W = $V >= 2;
					$V = $W ? $V / 2 : $V;
					let $KEY_15 = Math.pow(2, 52);
					let $KEY_7 = Math.floor($V * $KEY_15);
					$W = ($W ? $U + 1 : $U) + 1023;
					let $X = (($V * $KEY_15 - $KEY_7 < .5 ? $KEY_7 : $V * $KEY_15 - $KEY_7 > .5 ? $KEY_7 + 1 : $KEY_7 % 2 ? $KEY_7 + 1 : $KEY_7) - $KEY_15) / $KEY_15 >= 1;
					$T = $X ? $W + 1 : $W;
					if ($T > 2046) {
						$S = 0;
						$T = 2047;
					} else {
						$S = $X ? 0 : ($V * $KEY_15 - $KEY_7 < .5 ? $KEY_7 : $V * $KEY_15 - $KEY_7 > .5 ? $KEY_7 + 1 : $KEY_7 % 2 ? $KEY_7 + 1 : $KEY_7) - $KEY_15;
					}
				} else {
					$R = $R / Math.pow(2, -1074);
					let $KEY_5 = Math.floor($R);
					$S = $R - $KEY_5 < .5 ? $KEY_5 : $R - $KEY_5 > .5 ? $KEY_5 + 1 : $KEY_5 % 2 ? $KEY_5 + 1 : $KEY_5;
					$T = 0;
				}
				let $Y = [];
				let k = 51;
				for (; k >= 0; k--) {
					$Y.unshift(!!($S & 1));
					$S = Math.floor($S / 2);
				}
				let i = 11;
				for (; i > 0; i--) {
					$Y.unshift(!!($T & 1));
					$T = Math.floor($T / 2);
				}
				$Y.unshift($Q);
				$P = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					$P.push(Number($Y[j]) << 7 | Number($Y[j + 1]) << 6 | Number($Y[j + 2]) << 5 | Number($Y[j + 3]) << 4 | Number($Y[j + 4]) << 3 | Number($Y[j + 5]) << 2 | Number($Y[j + 6]) << 1 | Number($Y[j + 7]));
				}
			}
			let $Z = $G;
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$Z[++$KEY_9] = $P[i_2] ^ $E();
			}
		}
		return $KEY_9;
	}
	$KEY_14 = $THREAD_0_29466;
	function $THREAD_9_149099($E) {
		let $KEY_0, $KEY_1, $KEY_4, $KEY_5;
		function $THREAD_1_134428() {
			let $F = $KEY_5;
			$KEY_5 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_0 = $KEY_1 + $KEY_4;
				let $G = $KEY_4;
				$KEY_4 = $G ^ $KEY_1;
				$KEY_1 = $G ^ $KEY_1 ^ ($KEY_1 << 26 | $KEY_1 >>> 6) ^ $KEY_4 << 9;
				$KEY_4 = $KEY_4 << 13 | $KEY_4 >>> 19;
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
		$KEY_1 = $KEY_12;
		$KEY_4 = $E;
		$KEY_5 = -1;
		$THREAD_1_134428();
		$THREAD_1_134428();
		$THREAD_1_134428();
		$THREAD_1_134428();
		return $THREAD_1_134428;
	}
	$KEY_5 = $THREAD_9_149099;
	Object.prototype.hasOwnProperty;
	$KEY_13 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_1 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_18 = $KEY_1 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_6 = $KEY_18 ? null : [0, 0, 0, 0, 0, 0, 248, 127];
	let $C = $B.a.length >> 1 << 1;
	let i = 0;
	let $D = 0;
	for (; i < $C; i = i + 2) {
		$D = ($D << 5) - $D + ($B.a[i].charCodeAt() << 16 | $B.a[i + 1].charCodeAt()) | 0;
	}
	$KEY_12 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_4_113327() {
		let $KEY_3;
		function $THREAD_8_67960($K) {
			if ($K == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($K).length + $KEY_3(Object.getPrototypeOf($K));
			}
		}
		$KEY_3 = $THREAD_8_67960;
		let $E = Math.random() * 4294967296;
		let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];
		let $KEY_6 = $KEY_5(($E | 0) ^ 2525366799);
		let $G = $KEY_2($KEY_6);
		let $KEY_0 = $G(location.origin, $F, 3);
		$KEY_0 = $G(navigator.platform, $F, $KEY_0);
		let $KEY_7 = navigator.hardwareConcurrency;
		if ($KEY_7 == null) {
			$KEY_7 = $KEY_0 + 1;
			$F[$KEY_7] = $KEY_6() ^ 130;
		} else {
			$KEY_7 = $KEY_14($KEY_6, $KEY_7, $F, $KEY_0);
		}
		$KEY_6 = $KEY_14($KEY_6, $KEY_3(self), $F, $KEY_7);
		$G(navigator.userAgent, $F, $KEY_6);
		let $H = postMessage;
		let $I = { a: $KEY_4 };
		let $J = [[], $F];
		$I.b = $J;
		$H($I);
	}
	$THREAD_4_113327($B.b);
}