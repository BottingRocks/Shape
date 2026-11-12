function $THREAD_8_135183($A, $B) {
	let $KEY_12 = $A;
	let $KEY_17, $KEY_18, $KEY_7;
	function $THREAD_9_104270($F) {
		let $KEY_2 = $F;
		let $KEY_1 = Object.create(null);
		let $KEY_0 = 0;
		function $THREAD_4_134400($G, $H, $I) {
			let $KEY_3;
			if ($KEY_1[$G] != null) {
				$KEY_3 = $I;
				$KEY_3 = Number($I) + 1;
				$H[$KEY_3] = $KEY_2() ^ 129;
				$KEY_3 = $KEY_17($KEY_2, $KEY_1[$G], $H, $KEY_3);
			} else {
				let $J = $G.length;
				let $K = 0;
				let $L = $I;
				while ($K < $J) {
					let $M = $KEY_7($G[$K], 0);
					if ($M === 0) {
						$H[++$L] = $KEY_2() ^ 192;
						$M = 128;
					} else if (!($M < 128)) if ($M < 2048) {
						$H[++$L] = ($M >> 6 | 192) ^ $KEY_2();
						$M = $M & 63 | 128;
					} else {
						const $N = $M >= 55296 && $M <= 57343 && $M < 56320 && $K + 1 < $J ? $KEY_7($G[$K + 1], 0) : null;
						if ($N == null || $N < 56320 || $N > 57343) {
							$H[++$L] = ($M >> 12 | 224) ^ $KEY_2();
							$H[++$L] = ($M >> 6 & 63 | 128) ^ $KEY_2();
							$M = $M & 63 | 128;
						} else {
							++$K;
							$H[++$L] = (65536 + ($M - 55296 << 10) + ($N - 56320) >> 18 | 240) ^ $KEY_2();
							$H[++$L] = (65536 + ($M - 55296 << 10) + ($N - 56320) >> 12 & 63 | 128) ^ $KEY_2();
							$H[++$L] = (65536 + ($M - 55296 << 10) + ($N - 56320) >> 6 & 63 | 128) ^ $KEY_2();
							$M = 65536 + ($M - 55296 << 10) + ($N - 56320) & 63 | 128;
						}
					}
					$H[++$L] = $M ^ $KEY_2();
					++$K;
				}
				$KEY_3 = Number($L) + 1;
				$H[$KEY_3] = $KEY_2();
				let $O = $KEY_1;
				let $P = Number($KEY_0);
				$KEY_0 = $P + 1;
				$O[$G] = $P;
			}
			return $KEY_3;
		}
		return $THREAD_4_134400;
	}
	$KEY_18 = $THREAD_9_104270;
	let $KEY_2, $KEY_4, $KEY_8;
	function $THREAD_1_73724($F, $G, $H, $I) {
		let $KEY_7;
		if (($G | 0) === $G) {
			let $J;
			if ($G < 0) {
				$J = $G < 0;
			} else if ($G === 0) {
				$J = 1 / $G < 0;
			} else {
				$J = false;
			}
			let $K, $L;
			if ($J) {
				$K = -$G;
				$L = 64;
			} else {
				$K = $G;
				$L = 0;
			}
			if ($K < 32) {
				$KEY_7 = Number($I) + 1;
				$H[$KEY_7] = ($L | $K) ^ $F();
			} else if ($K < 4096) {
				$KEY_7 = Number($I) + 1;
				$H[$KEY_7] = ($L | ($K & 31 | 32)) ^ $F();
				$H[++$KEY_7] = $K >> 5 ^ $F();
			} else if ($K < 524288) {
				$KEY_7 = Number($I) + 1;
				$H[$KEY_7] = ($L | ($K & 31 | 32)) ^ $F();
				$H[++$KEY_7] = ($K >> 5 & 127 | 128) ^ $F();
				$H[++$KEY_7] = $K >>> 12 ^ $F();
			} else if ($K < 67108864) {
				$KEY_7 = Number($I) + 1;
				$H[$KEY_7] = ($L | ($K & 31 | 32)) ^ $F();
				$H[++$KEY_7] = ($K >> 5 & 127 | 128) ^ $F();
				$H[++$KEY_7] = ($K >>> 12 & 127 | 128) ^ $F();
				$H[++$KEY_7] = $K >>> 19 ^ $F();
			} else {
				$KEY_7 = Number($I) + 1;
				$H[$KEY_7] = ($L | ($K & 31 | 32)) ^ $F();
				$H[++$KEY_7] = ($K >> 5 & 127 | 128) ^ $F();
				$H[++$KEY_7] = ($K >>> 12 & 127 | 128) ^ $F();
				$H[++$KEY_7] = ($K >>> 19 & 127 | 128) ^ $F();
				$H[++$KEY_7] = $K >>> 26 ^ $F();
			}
		} else {
			$KEY_7 = $I;
			$KEY_7 = Number($I) + 1;
			$H[$KEY_7] = $F() ^ 128;
			let $M;
			if ($KEY_4) {
				$KEY_4[0] = $G;
				$M = new $KEY_2($KEY_4.buffer);
			} else if ($G !== $G) {
				$M = $KEY_8;
			} else {
				const $N = $G < 0 || $G === 0 && 1 / $G < 0;
				let $O = Math.abs($G);
				let $P, $Q;
				if ($O >= Math.pow(2, -1022)) {
					let $KEY_9 = Math.min(Math.floor(Math.log($O) / Math.LN2), 1023);
					$O = $O / Math.pow(2, $KEY_9);
					$P = $O < 1;
					$O = $P ? $O * 2 : $O;
					$P = $P ? $KEY_9 - 1 : $KEY_9;
					if ($O >= 2) {
						++$P;
						$O = $O / 2;
					}
					let $KEY_13 = Math.pow(2, 52);
					let $R = Math.floor($O * $KEY_13);
					const $S = $O * $KEY_13 - $R < .5 ? $R : $O * $KEY_13 - $R > .5 ? $R + 1 : $R % 2 ? $R + 1 : $R;
					let $T = ($S - $KEY_13) / $KEY_13 >= 1;
					$P = $T ? $P + 1024 : $P + 1023;
					if ($P > 2046) {
						$P = 2047;
						$Q = 0;
					} else {
						$Q = $T ? 0 : $S - $KEY_13;
					}
				} else {
					$O = $O / Math.pow(2, -1074);
					let $U = Math.floor($O);
					$P = 0;
					$Q = $O - $U < .5 ? $U : $O - $U > .5 ? $U + 1 : $U % 2 ? $U + 1 : $U;
				}
				let $V = [];
				let k = 51;
				for (; k >= 0; k--) {
					$V.unshift(!!($Q & 1));
					$Q = Math.floor($Q / 2);
				}
				let i = 11;
				for (; i > 0; i--) {
					$V.unshift(!!($P & 1));
					$P = Math.floor($P / 2);
				}
				$V.unshift($N);
				$M = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					$M.push(Number($V[j]) << 7 | Number($V[j + 1]) << 6 | Number($V[j + 2]) << 5 | Number($V[j + 3]) << 4 | Number($V[j + 4]) << 3 | Number($V[j + 5]) << 2 | Number($V[j + 6]) << 1 | Number($V[j + 7]));
				}
			}
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$H[++$KEY_7] = $M[i_2] ^ $F();
			}
		}
		return $KEY_7;
	}
	$KEY_17 = $THREAD_1_73724;
	let $KEY_11;
	function $THREAD_7_137336($F) {
		let $KEY_0, $KEY_3, $KEY_4, $KEY_5;
		function $THREAD_1_16710() {
			let $G = $KEY_0;
			$KEY_0 = ($G + 1) % 4;
			if (($G + 1) % 4 === 0) {
				$KEY_3 = $KEY_4 + $KEY_5;
				let $H = $KEY_5;
				$KEY_5 = $H ^ $KEY_4;
				$KEY_4 = $H ^ $KEY_4 ^ ($KEY_4 << 26 | $KEY_4 >>> 6) ^ $KEY_5 << 9;
				$KEY_5 = $KEY_5 << 13 | $KEY_5 >>> 19;
				return $KEY_3 & 255;
			}
			if (($G + 1) % 4 === 1) {
				return $KEY_3 >> 8 & 255;
			}
			if (($G + 1) % 4 === 2) {
				return $KEY_3 >> 16 & 255;
			}
			if (($G + 1) % 4 === 3) {
				return $KEY_3 >>> 24 & 255;
			} else {
				return void 0;
			}
		}
		$KEY_4 = $KEY_11;
		$KEY_5 = $F;
		$KEY_0 = -1;
		$THREAD_1_16710();
		$THREAD_1_16710();
		$THREAD_1_16710();
		$THREAD_1_16710();
		return $THREAD_1_16710;
	}
	let $KEY_10 = $THREAD_7_137336;
	Object.prototype.hasOwnProperty;
	$KEY_7 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_2 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_4 = $KEY_2 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_8 = $KEY_4 ? null : [0, 0, 0, 0, 0, 0, 248, 127];
	let $C = $B.a.length >> 1 << 1;
	let $D = 0;
	let $E = 0;
	while ($D < $C) {
		$E = ($E << 5) - $E + ($B.a[$D].charCodeAt() << 16 | $B.a[$D + 1].charCodeAt()) | 0;
		$D += 2;
	}
	$KEY_11 = $C !== $B.a.length ? ($E << 5) - $E + ($B.a[$C].charCodeAt() << 16) | 0 : $E;
	function $THREAD_7_125068() {
		let $KEY_4;
		function $THREAD_9_105318($M) {
			if ($M == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($M).length + $KEY_4(Object.getPrototypeOf($M));
			}
		}
		$KEY_4 = $THREAD_9_105318;
		let $F = Math.random() * 4294967296;
		let $KEY_3 = [($F | 0) >>> 24, ($F | 0) >> 16 & 255, ($F | 0) >> 8 & 255, ($F | 0) & 255];
		let $KEY_0 = $KEY_10(($F | 0) ^ 2058213362);
		let $G = $KEY_18($KEY_0);
		let $H = $G(navigator.platform, $KEY_3, 3);
		$H = $G(navigator.userAgent, $KEY_3, $H);
		$G = $G(location.origin, $KEY_3, $H);
		$G = $KEY_17($KEY_0, $KEY_4(self), $KEY_3, $G);
		let $I = navigator.hardwareConcurrency;
		if ($I == null) {
			$KEY_3[$G + 1] = $KEY_0() ^ 130;
		} else {
			$KEY_17($KEY_0, $I, $KEY_3, $G);
		}
		let $J = postMessage;
		let $K = { a: $KEY_12 };
		let $L = [[], $KEY_3];
		$K.b = $L;
		$J($K);
	}
	$THREAD_7_125068($B.b);
}