function $THREAD_0_165997($A, $B) {
	let $KEY_18 = $A;
	let $KEY_19, $KEY_20, $KEY_3;
	function $THREAD_9_78544($E) {
		let $KEY_0, $KEY_2, $KEY_4, $KEY_5;
		function $THREAD_1_88339() {
			let $F = $KEY_2;
			$KEY_2 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_4 = $KEY_5 + $KEY_0;
				let $G = $KEY_0;
				$KEY_0 = $G ^ $KEY_5;
				$KEY_5 = $G ^ $KEY_5 ^ ($KEY_5 << 26 | $KEY_5 >>> 6) ^ $KEY_0 << 9;
				$KEY_0 = $KEY_0 << 13 | $KEY_0 >>> 19;
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
		$KEY_5 = $KEY_3;
		$KEY_0 = $E;
		$KEY_2 = -1;
		$THREAD_1_88339();
		$THREAD_1_88339();
		$THREAD_1_88339();
		$THREAD_1_88339();
		return $THREAD_1_88339;
	}
	let $KEY_2, $KEY_4;
	function $THREAD_0_18260($E, $F, $G, $H) {
		let $KEY_0;
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
				$KEY_0 = Number($H) + 1;
				$G[$KEY_0] = ($J | $K) ^ $E();
			} else if ($K < 4096) {
				let $L = Number($H) + 1;
				$G[$L] = ($J | ($K & 31 | 32)) ^ $E();
				$KEY_0 = $L + 1;
				$G[$KEY_0] = $K >> 5 ^ $E();
			} else if ($K < 524288) {
				let $M = Number($H) + 1;
				$G[$M] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$M] = ($K >> 5 & 127 | 128) ^ $E();
				$KEY_0 = $M + 1;
				$G[$KEY_0] = $K >>> 12 ^ $E();
			} else if ($K < 67108864) {
				let $N = Number($H) + 1;
				$G[$N] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$N] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$N] = ($K >>> 12 & 127 | 128) ^ $E();
				$KEY_0 = $N + 1;
				$G[$KEY_0] = $K >>> 19 ^ $E();
			} else {
				let $O = Number($H) + 1;
				$G[$O] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$O] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$O] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$O] = ($K >>> 19 & 127 | 128) ^ $E();
				$KEY_0 = $O + 1;
				$G[$KEY_0] = $K >>> 26 ^ $E();
			}
		} else {
			$KEY_0 = Number($H) + 1;
			$G[$KEY_0] = $E() ^ 128;
			let $P;
			if ($KEY_19) {
				$KEY_19[0] = $F;
				$P = new $KEY_2($KEY_19.buffer);
			} else if ($F !== $F) {
				$P = $KEY_4;
			} else {
				const $Q = $F < 0 || $F === 0 && 1 / $F < 0;
				let $R = Math.abs($F);
				let $S, $T;
				if ($R >= Math.pow(2, -1022)) {
					let $KEY_9 = Math.min(Math.floor(Math.log($R) / Math.LN2), 1023);
					$R = $R / Math.pow(2, $KEY_9);
					let $U = $R < 1;
					$R = $U ? $R * 2 : $R;
					$U = $U ? $KEY_9 - 1 : $KEY_9;
					if ($R >= 2) {
						++$U;
						$R = $R / 2;
					}
					let $KEY_5 = Math.pow(2, 52);
					$R = $R * $KEY_5;
					let $V = Math.floor($R);
					if (!($R - $V < .5)) if ($R - $V > .5) {
						++$V;
					} else {
						$V = [$V, $V % 2];
						$V = $V[1] ? $V[0] + 1 : $V[0];
					}
					$U += 1023;
					$T = ($V - $KEY_5) / $KEY_5 >= 1;
					$KEY_5 = $T ? 0 : $V - $KEY_5;
					$T = $T ? $U + 1 : $U;
					if ($T > 2046) {
						$S = 0;
						$T = 2047;
					} else {
						$S = $KEY_5;
					}
				} else {
					$R = $R / Math.pow(2, -1074);
					$S = Math.floor($R);
					if (!($R - $S < .5)) if ($R - $S > .5) {
						++$S;
					} else {
						$S = [$S, $S % 2];
						$S = $S[1] ? $S[0] + 1 : $S[0];
					}
					$T = 0;
				}
				let $W = [];
				let k = 51;
				for (; k >= 0; k--) {
					$W.unshift(!!($S & 1));
					$S = Math.floor($S / 2);
				}
				let i = 11;
				for (; i > 0; i--) {
					$W.unshift(!!($T & 1));
					$T = Math.floor($T / 2);
				}
				$W.unshift($Q);
				$P = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					$P.push(Number($W[j]) << 7 | Number($W[j + 1]) << 6 | Number($W[j + 2]) << 5 | Number($W[j + 3]) << 4 | Number($W[j + 4]) << 3 | Number($W[j + 5]) << 2 | Number($W[j + 6]) << 1 | Number($W[j + 7]));
				}
			}
			let $X = $G;
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$X[++$KEY_0] = $P[i_2] ^ $E();
			}
		}
		return $KEY_0;
	}
	let $KEY_10, $KEY_13;
	function $THREAD_3_97758($E) {
		let $KEY_2 = $E;
		let $KEY_1 = Object.create(null);
		let $KEY_0 = 0;
		function $THREAD_9_112498($F, $G, $H) {
			let $KEY_12;
			if ($KEY_1[$F] != null) {
				$KEY_12 = $H;
				$KEY_12 = Number($H) + 1;
				$G[$KEY_12] = $KEY_2() ^ 129;
				$KEY_12 = $KEY_10($KEY_2, $KEY_1[$F], $G, $KEY_12);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_13($F[$J], 0);
					if ($L === 0) {
						$G[++$K] = $KEY_2() ^ 192;
						$L = 128;
					} else if (!($L < 128)) if ($L < 2048) {
						$G[++$K] = ($L >> 6 | 192) ^ $KEY_2();
						$L = $L & 63 | 128;
					} else {
						const $M = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_13($F[$J + 1], 0) : null;
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
				$KEY_12 = Number($K) + 1;
				$G[$KEY_12] = $KEY_2();
				let $N = $KEY_1;
				let $O = Number($KEY_0);
				$KEY_0 = $O + 1;
				$N[$F] = $O;
			}
			return $KEY_12;
		}
		return $THREAD_9_112498;
	}
	let $KEY_6 = $THREAD_3_97758;
	$KEY_10 = $THREAD_0_18260;
	$KEY_20 = $THREAD_9_78544;
	Object.prototype.hasOwnProperty;
	$KEY_13 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_2 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_19 = $KEY_2 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_4 = $KEY_19 ? null : (($PARAMA, $PARAMB) => {
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
	function $THREAD_9_123566() {
		let $KEY_3;
		function $THREAD_2_2464($J) {
			if ($J == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($J).length + $KEY_3(Object.getPrototypeOf($J));
			}
		}
		$KEY_3 = $THREAD_2_2464;
		let $KEY_4 = Math.random() * 4294967296 | 0;
		let $E = [$KEY_4 >>> 24, $KEY_4 >> 16 & 255, $KEY_4 >> 8 & 255, $KEY_4 & 255];
		let $KEY_7 = $KEY_20($KEY_4 ^ 2216282875);
		let $KEY_0 = $KEY_6($KEY_7);
		let $F = $KEY_0(location.origin, $E, 3);
		$F = $KEY_0(navigator.platform, $E, $F);
		let $KEY_2 = navigator.hardwareConcurrency;
		if ($KEY_2 == null) {
			$KEY_2 = $F + 1;
			$E[$KEY_2] = $KEY_7() ^ 130;
		} else {
			$KEY_2 = $KEY_10($KEY_7, $KEY_2, $E, $F);
		}
		$KEY_0 = $KEY_0(navigator.userAgent, $E, $KEY_2);
		$KEY_10($KEY_7, $KEY_3(self), $E, $KEY_0);
		let $G = postMessage;
		let $H = { a: $KEY_18 };
		let $I = [[], $E];
		$H.b = $I;
		$G($H);
	}
	$THREAD_9_123566($B.b);
}