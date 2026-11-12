function $THREAD_5_52926($A, $B) {
	let $KEY_5 = $A;
	let $KEY_7, $KEY_9, $KEY_10, $KEY_12, $KEY_14, $KEY_18;
	function $THREAD_9_94669($E) {
		let $KEY_0, $KEY_2, $KEY_3, $KEY_5;
		function $THREAD_7_146119() {
			let $F = $KEY_2;
			$KEY_2 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_3 = $KEY_5 + $KEY_0;
				let $G = $KEY_0;
				$KEY_0 = $G ^ $KEY_5;
				$KEY_5 = $G ^ $KEY_5 ^ ($KEY_5 << 26 | $KEY_5 >>> 6) ^ $KEY_0 << 9;
				$KEY_0 = $KEY_0 << 13 | $KEY_0 >>> 19;
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
		$KEY_5 = $KEY_14;
		$KEY_0 = $E;
		$KEY_2 = -1;
		$THREAD_7_146119();
		$THREAD_7_146119();
		$THREAD_7_146119();
		$THREAD_7_146119();
		return $THREAD_7_146119;
	}
	function $THREAD_4_118603($E) {
		let $KEY_0 = $E;
		let $KEY_1, $KEY_2;
		$KEY_1 = Object.create(null);
		$KEY_2 = 0;
		function $THREAD_1_139706($F, $G, $H) {
			let $KEY_13;
			if ($KEY_1[$F] != null) {
				$KEY_13 = $H;
				$KEY_13 = Number($H) + 1;
				$G[$KEY_13] = $KEY_0() ^ 129;
				$KEY_13 = $KEY_7($KEY_0, $KEY_1[$F], $G, $KEY_13);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_12($F[$J], 0);
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
							let $N = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_12($F[$J + 1], 0) : null;
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
				$KEY_13 = Number($K) + 1;
				$G[$KEY_13] = $KEY_0();
				let $O = $KEY_1;
				let $P = Number($KEY_2);
				$KEY_2 = $P + 1;
				$O[$F] = $P;
			}
			return $KEY_13;
		}
		return $THREAD_1_139706;
	}
	$KEY_9 = $THREAD_4_118603;
	let $KEY_3, $KEY_4;
	function $THREAD_0_25773($E, $F, $G, $H) {
		let $KEY_14;
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
				$KEY_14 = Number($H) + 1;
				$G[$KEY_14] = ($J | $K) ^ $E();
			} else if ($K < 4096) {
				let $L = Number($H) + 1;
				$G[$L] = ($J | ($K & 31 | 32)) ^ $E();
				$KEY_14 = $L + 1;
				$G[$KEY_14] = $K >> 5 ^ $E();
			} else if ($K < 524288) {
				let $M = Number($H) + 1;
				$G[$M] = ($J | ($K & 31 | 32)) ^ $E();
				$KEY_14 = $M + 1;
				$G[$KEY_14] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_14] = $K >>> 12 ^ $E();
			} else if ($K < 67108864) {
				let $N = Number($H) + 1;
				$G[$N] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$N] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$N] = ($K >>> 12 & 127 | 128) ^ $E();
				$KEY_14 = $N + 1;
				$G[$KEY_14] = $K >>> 19 ^ $E();
			} else {
				let $O = Number($H) + 1;
				$G[$O] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$O] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$O] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$O] = ($K >>> 19 & 127 | 128) ^ $E();
				$KEY_14 = $O + 1;
				$G[$KEY_14] = $K >>> 26 ^ $E();
			}
		} else {
			$KEY_14 = Number($H) + 1;
			$G[$KEY_14] = $E() ^ 128;
			let $P, $Q;
			if ($KEY_4) {
				$KEY_4[0] = $F;
				$P = new $KEY_10($KEY_4.buffer);
			} else {
				let $R;
				if ($F !== $F) {
					$R = $KEY_3;
				} else {
					let $KEY_12 = $F < 0 || $F === 0 && 1 / $F < 0;
					let $S = Math.abs($F);
					let $T, $U;
					if ($S >= Math.pow(2, -1022)) {
						let $V = Math.min(Math.floor(Math.log($S) / Math.LN2), 1023);
						$S = $S / Math.pow(2, $V);
						let $W = $S < 1;
						$S = $W ? $S * 2 : $S;
						$W = $W ? $V - 1 : $V;
						if ($S >= 2) {
							++$W;
							$S = $S / 2;
						}
						let $X = Math.pow(2, 52);
						let $Y = Math.floor($S * $X);
						if (!($S * $X - $Y < .5)) if ($S * $X - $Y > .5) {
							++$Y;
						} else {
							$Y = [$Y, $Y % 2];
							$Y = $Y[1] ? $Y[0] + 1 : $Y[0];
						}
						$Y = $Y - $X;
						$W += 1023;
						$X = $Y / $X >= 1;
						$U = $X ? $W + 1 : $W;
						if ($U > 2046) {
							$T = 0;
							$U = 2047;
						} else {
							$T = $X ? 0 : $Y;
						}
					} else {
						$S = $S / Math.pow(2, -1074);
						$T = Math.floor($S);
						if (!($S - $T < .5)) if ($S - $T > .5) {
							++$T;
						} else {
							$T = [$T, $T % 2];
							$T = $T[1] ? $T[0] + 1 : $T[0];
						}
						$U = 0;
					}
					let $Z = [];
					let j = 51;
					for (; j >= 0; j--) {
						$Z.unshift(!!($T & 1));
						$T = Math.floor($T / 2);
					}
					let i = 11;
					for (; i > 0; i--) {
						$Z.unshift(!!($U & 1));
						$U = Math.floor($U / 2);
					}
					$Z.unshift($KEY_12);
					let $AA = [];
					let $AB = 56;
					while ($AB >= 0) {
						$AA.push(Number($Z[$AB]) << 7 | Number($Z[$AB + 1]) << 6 | Number($Z[$AB + 2]) << 5 | Number($Z[$AB + 3]) << 4 | Number($Z[$AB + 4]) << 3 | Number($Z[$AB + 5]) << 2 | Number($Z[$AB + 6]) << 1 | Number($Z[$AB + 7]));
						$AB = $AB - 8;
					}
					$R = $AA;
				}
				$P = $R;
			}
			$Q = $G;
			let k = 0;
			for (; k < 8; k++) {
				$Q[++$KEY_14] = $P[k] ^ $E();
			}
		}
		return $KEY_14;
	}
	$KEY_7 = $THREAD_0_25773;
	$KEY_18 = $THREAD_9_94669;
	Object.prototype.hasOwnProperty;
	$KEY_12 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_10 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_4 = $KEY_10 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_3 = $KEY_4 ? null : [0, 0, 0, 0, 0, 0, 248, 127];
	let $C = $B.a.length >> 1 << 1;
	let i = 0;
	let $D = 0;
	for (; i < $C; i = i + 2) {
		$D = ($D << 5) - $D + ($B.a[i].charCodeAt() << 16 | $B.a[i + 1].charCodeAt()) | 0;
	}
	$KEY_14 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_7_49410() {
		let $KEY_0;
		function $THREAD_7_40350($L) {
			if ($L == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($L).length + $KEY_0(Object.getPrototypeOf($L));
			}
		}
		$KEY_0 = $THREAD_7_40350;
		let $E = Math.random() * 4294967296;
		let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];
		let $KEY_10 = $KEY_18(($E | 0) ^ 4269415474);
		let $KEY_4 = $KEY_9($KEY_10);
		let $G = $KEY_4(location.origin, $F, 3);
		let $H = navigator.hardwareConcurrency;
		if ($H == null) {
			$F[++$G] = $KEY_10() ^ 130;
		} else {
			$G = $KEY_7($KEY_10, $H, $F, $G);
		}
		$G = $KEY_4(navigator.platform, $F, $G);
		$KEY_4 = $KEY_4(navigator.userAgent, $F, $G);
		$KEY_7($KEY_10, $KEY_0(self), $F, $KEY_4);
		let $I = postMessage;
		let $J = { a: $KEY_5 };
		let $K = [[], $F];
		$J.b = $K;
		$I($J);
	}
	$THREAD_7_49410($B.b);
}