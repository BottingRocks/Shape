function $THREAD_1_78447($A, $B) {
	let $KEY_6 = $A;
	let $KEY_8, $KEY_9, $KEY_12, $KEY_14, $KEY_16, $KEY_17;
	function $THREAD_4_59532($F) {
		let $KEY_0, $KEY_2, $KEY_3, $KEY_4;
		function $THREAD_1_143212() {
			let $G = $KEY_4;
			$KEY_4 = ($G + 1) % 4;
			if (($G + 1) % 4 === 0) {
				$KEY_0 = $KEY_3 + $KEY_2;
				let $H = $KEY_2;
				$KEY_2 = $H ^ $KEY_3;
				$KEY_3 = $H ^ $KEY_3 ^ ($KEY_3 << 26 | $KEY_3 >>> 6) ^ $KEY_2 << 9;
				$KEY_2 = $KEY_2 << 13 | $KEY_2 >>> 19;
				return $KEY_0 & 255;
			}
			if (($G + 1) % 4 === 1) {
				return $KEY_0 >> 8 & 255;
			}
			if (($G + 1) % 4 === 2) {
				return $KEY_0 >> 16 & 255;
			}
			if (($G + 1) % 4 === 3) {
				return $KEY_0 >>> 24 & 255;
			} else {
				return void 0;
			}
		}
		$KEY_3 = $KEY_12;
		$KEY_2 = $F;
		$KEY_4 = -1;
		$THREAD_1_143212();
		$THREAD_1_143212();
		$THREAD_1_143212();
		$THREAD_1_143212();
		return $THREAD_1_143212;
	}
	function $THREAD_9_58667($F) {
		let $KEY_2 = $F;
		let $KEY_0 = Object.create(null);
		let $KEY_1 = 0;
		function $THREAD_0_22453($G, $H, $I) {
			let $KEY_3;
			if ($KEY_0[$G] != null) {
				$KEY_3 = $I;
				$KEY_3 = Number($I) + 1;
				$H[$KEY_3] = $KEY_2() ^ 129;
				$KEY_3 = $KEY_8($KEY_2, $KEY_0[$G], $H, $KEY_3);
			} else {
				let $J = $G.length;
				let $K = 0;
				let $L = $I;
				while ($K < $J) {
					let $M = $KEY_9($G[$K], 0);
					if ($M === 0) {
						$H[++$L] = $KEY_2() ^ 192;
						$M = 128;
					} else if (!($M < 128)) if ($M < 2048) {
						$H[++$L] = ($M >> 6 | 192) ^ $KEY_2();
						$M = $M & 63 | 128;
					} else {
						let $N = $M >= 55296 && $M <= 57343 && $M < 56320 && $K + 1 < $J ? $KEY_9($G[$K + 1], 0) : null;
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
				let $O = $KEY_0;
				let $P = Number($KEY_1);
				$KEY_1 = $P + 1;
				$O[$G] = $P;
			}
			return $KEY_3;
		}
		return $THREAD_0_22453;
	}
	$KEY_16 = $THREAD_9_58667;
	let $KEY_4, $KEY_5;
	function $THREAD_9_157720($F, $G, $H, $I) {
		if (($G | 0) === $G) {
			let $J;
			if ($G < 0) {
				$J = $G < 0;
			} else if ($G === 0) {
				$J = 1 / $G < 0;
			} else {
				$J = $G === 0;
			}
			let $K, $L;
			if ($J) {
				$K = -$G;
				$L = 64;
			} else {
				$K = $G;
				$L = 0;
			}
			let $KEY_11;
			if ($K < 32) {
				$KEY_11 = Number($I) + 1;
				$H[$KEY_11] = ($L | $K) ^ $F();
			} else if ($K < 4096) {
				let $M = Number($I) + 1;
				$H[$M] = ($L | ($K & 31 | 32)) ^ $F();
				$KEY_11 = $M + 1;
				$H[$KEY_11] = $K >> 5 ^ $F();
			} else if ($K < 524288) {
				$KEY_11 = Number($I) + 1;
				$H[$KEY_11] = ($L | ($K & 31 | 32)) ^ $F();
				$H[++$KEY_11] = ($K >> 5 & 127 | 128) ^ $F();
				$H[++$KEY_11] = $K >>> 12 ^ $F();
			} else if ($K < 67108864) {
				let $N = Number($I) + 1;
				$H[$N] = ($L | ($K & 31 | 32)) ^ $F();
				$H[++$N] = ($K >> 5 & 127 | 128) ^ $F();
				$H[++$N] = ($K >>> 12 & 127 | 128) ^ $F();
				$KEY_11 = $N + 1;
				$H[$KEY_11] = $K >>> 19 ^ $F();
			} else {
				let $O = Number($I) + 1;
				$H[$O] = ($L | ($K & 31 | 32)) ^ $F();
				$H[++$O] = ($K >> 5 & 127 | 128) ^ $F();
				$H[++$O] = ($K >>> 12 & 127 | 128) ^ $F();
				$H[++$O] = ($K >>> 19 & 127 | 128) ^ $F();
				$KEY_11 = $O + 1;
				$H[$KEY_11] = $K >>> 26 ^ $F();
			}
			return $KEY_11;
		} else {
			let $P = $H;
			let $Q = Number($I) + 1;
			$P[$Q] = $F() ^ 128;
			let $R;
			if ($KEY_17) {
				$KEY_17[0] = $G;
				$R = new $KEY_5($KEY_17.buffer);
			} else if ($G !== $G) {
				$R = $KEY_4;
			} else {
				let $KEY_6 = $G < 0 || $G === 0 && 1 / $G < 0;
				let $S = Math.abs($G);
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
					$S = $S * $X;
					if (!($S - $Y < .5)) if ($S - $Y > .5) {
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
				let k = 51;
				for (; k >= 0; k--) {
					$Z.unshift(!!($T & 1));
					$T = Math.floor($T / 2);
				}
				let i = 11;
				for (; i > 0; i--) {
					$Z.unshift(!!($U & 1));
					$U = Math.floor($U / 2);
				}
				$Z.unshift($KEY_6);
				$R = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					$R.push(Number($Z[j]) << 7 | Number($Z[j + 1]) << 6 | Number($Z[j + 2]) << 5 | Number($Z[j + 3]) << 4 | Number($Z[j + 4]) << 3 | Number($Z[j + 5]) << 2 | Number($Z[j + 6]) << 1 | Number($Z[j + 7]));
				}
			}
			let $AA = $H;
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$AA[++$Q] = $R[i_2] ^ $F();
			}
			return $Q;
		}
	}
	$KEY_8 = $THREAD_9_157720;
	$KEY_14 = $THREAD_4_59532;
	Object.prototype.hasOwnProperty;
	$KEY_9 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_5 = typeof Uint8Array === "function" ? Uint8Array : null;
	let $C;
	if ($KEY_5 && typeof Float64Array === "function") {
		$C = new Float64Array(1);
	} else {
		$C = null;
	}
	$KEY_17 = $C;
	$KEY_4 = $C ? null : [0, 0, 0, 0, 0, 0, 248, 127];
	let $D = $B.a.length >> 1 << 1;
	let i = 0;
	let $E = 0;
	for (; i < $D; i = i + 2) {
		$E = ($E << 5) - $E + ($B.a[i].charCodeAt() << 16 | $B.a[i + 1].charCodeAt()) | 0;
	}
	$KEY_12 = $D !== $B.a.length ? ($E << 5) - $E + ($B.a[$D].charCodeAt() << 16) | 0 : $E;
	function $THREAD_8_90904() {
		let $KEY_7;
		function $THREAD_2_122702($N) {
			if ($N == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($N).length + $KEY_7(Object.getPrototypeOf($N));
			}
		}
		$KEY_7 = $THREAD_2_122702;
		let $F = Math.random() * 4294967296;
		let $G = [($F | 0) >>> 24, ($F | 0) >> 16 & 255, ($F | 0) >> 8 & 255, ($F | 0) & 255];
		let $KEY_5 = $KEY_14(($F | 0) ^ 3296993817);
		let $H = $KEY_16($KEY_5);
		let $I = $H(navigator.userAgent, $G, 3);
		$I = $H(location.origin, $G, $I);
		$H = $H(navigator.platform, $G, $I);
		let $J = navigator.hardwareConcurrency;
		if ($J == null) {
			$J = $H + 1;
			$G[$J] = $KEY_5() ^ 130;
		} else {
			$J = $KEY_8($KEY_5, $J, $G, $H);
		}
		$KEY_8($KEY_5, $KEY_7(self), $G, $J);
		let $K = postMessage;
		let $L = { a: $KEY_6 };
		let $M = [[], $G];
		$L.b = $M;
		$K($L);
	}
	$THREAD_8_90904($B.b);
}