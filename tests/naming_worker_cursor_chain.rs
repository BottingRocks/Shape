//! Source-only worker cursor-chain tests; central integration must execute them.
use shape::naming::name_threads;
const CASE_0: &str = r###"function $THREAD_7_53626($A, $B) {
	let $KEY_5 = $A;
	let $KEY_8, $KEY_9, $KEY_12, $KEY_16, $KEY_18;
	function $THREAD_6_106427($E) {
		let $KEY_2 = $E;
		let $KEY_1 = Object.create(null);
		let $KEY_0 = 0;
		function $THREAD_3_166283($F, $G, $H) {
			let $KEY_3;
			if ($KEY_1[$F] != null) {
				$KEY_3 = $H;
				$KEY_3 = Number($H) + 1;
				$G[$KEY_3] = $KEY_2() ^ 129;
				$KEY_3 = $KEY_9($KEY_2, $KEY_1[$F], $G, $KEY_3);
			} else {
				let $I = $F.length;
				let $J = 0;
				let $K = $H;
				while ($J < $I) {
					let $L = $KEY_18($F[$J], 0);
					if ($L === 0) {
						$G[++$K] = $KEY_2() ^ 192;
						$L = 128;
					} else if (!($L < 128)) if ($L < 2048) {
						$G[++$K] = ($L >> 6 | 192) ^ $KEY_2();
						$L = $L & 63 | 128;
					} else {
						const $M = $L >= 55296 && $L <= 57343 && $L < 56320 && $J + 1 < $I ? $KEY_18($F[$J + 1], 0) : null;
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
				$KEY_3 = Number($K) + 1;
				$G[$KEY_3] = $KEY_2();
				let $N = $KEY_1;
				let $O = Number($KEY_0);
				$KEY_0 = $O + 1;
				$N[$F] = $O;
			}
			return $KEY_3;
		}
		return $THREAD_3_166283;
	}
	let $KEY_1 = $THREAD_6_106427;
	let $KEY_0;
	function $THREAD_0_133811($E, $F, $G, $H) {
		let $KEY_19;
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
				$KEY_19 = Number($H) + 1;
				$G[$KEY_19] = ($J | $K) ^ $E();
			} else if ($K < 4096) {
				$KEY_19 = Number($H) + 1;
				$G[$KEY_19] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_19] = $K >> 5 ^ $E();
			} else if ($K < 524288) {
				$KEY_19 = Number($H) + 1;
				$G[$KEY_19] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_19] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_19] = $K >>> 12 ^ $E();
			} else if ($K < 67108864) {
				$KEY_19 = Number($H) + 1;
				$G[$KEY_19] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_19] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_19] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_19] = $K >>> 19 ^ $E();
			} else {
				$KEY_19 = Number($H) + 1;
				$G[$KEY_19] = ($J | ($K & 31 | 32)) ^ $E();
				$G[++$KEY_19] = ($K >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_19] = ($K >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_19] = ($K >>> 19 & 127 | 128) ^ $E();
				$G[++$KEY_19] = $K >>> 26 ^ $E();
			}
		} else {
			$KEY_19 = $H;
			$KEY_19 = Number($H) + 1;
			$G[$KEY_19] = $E() ^ 128;
			let $L;
			if ($KEY_16) {
				$KEY_16[0] = $F;
				$L = new $KEY_0($KEY_16.buffer);
			} else if ($F !== $F) {
				$L = $KEY_12;
			} else {
				let $KEY_9 = $F < 0 || $F === 0 && 1 / $F < 0;
				let $M = Math.abs($F);
				let $N;
				if ($M >= Math.pow(2, -1022)) {
					$N = Math.min(Math.floor(Math.log($M) / Math.LN2), 1023);
					$M = $M / Math.pow(2, $N);
					let $O = $M < 1;
					$N = $O ? $N - 1 : $N;
					$O = $O ? $M * 2 : $M;
					let $P = $O >= 2;
					$N = $P ? $N + 1 : $N;
					$P = $P ? $O / 2 : $O;
					let $Q = Math.pow(2, 52);
					let $KEY_5 = Math.floor($P * $Q);
					let $R = (($P * $Q - $KEY_5 < .5 ? $KEY_5 : $P * $Q - $KEY_5 > .5 ? $KEY_5 + 1 : $KEY_5 % 2 ? $KEY_5 + 1 : $KEY_5) - $Q) / $Q >= 1;
					$N = $R ? $N + 1024 : $N + 1023;
					if ($N > 2046) {
						$N = 2047;
						$M = 0;
					} else {
						$M = $R ? 0 : ($P * $Q - $KEY_5 < .5 ? $KEY_5 : $P * $Q - $KEY_5 > .5 ? $KEY_5 + 1 : $KEY_5 % 2 ? $KEY_5 + 1 : $KEY_5) - $Q;
					}
				} else {
					$M = $M / Math.pow(2, -1074);
					let $S = Math.floor($M);
					$N = 0;
					$M = $M - $S < .5 ? $S : $M - $S > .5 ? $S + 1 : $S % 2 ? $S + 1 : $S;
				}
				let $T = [];
				let k = 51;
				for (; k >= 0; k--) {
					$T.unshift(!!($M & 1));
					$M = Math.floor($M / 2);
				}
				let i = 11;
				for (; i > 0; i--) {
					$T.unshift(!!($N & 1));
					$N = Math.floor($N / 2);
				}
				$T.unshift($KEY_9);
				$L = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					$L.push(Number($T[j]) << 7 | Number($T[j + 1]) << 6 | Number($T[j + 2]) << 5 | Number($T[j + 3]) << 4 | Number($T[j + 4]) << 3 | Number($T[j + 5]) << 2 | Number($T[j + 6]) << 1 | Number($T[j + 7]));
				}
			}
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$G[++$KEY_19] = $L[i_2] ^ $E();
			}
		}
		return $KEY_19;
	}
	$KEY_9 = $THREAD_0_133811;
	let $KEY_2;
	function $THREAD_6_161183($E) {
		let $KEY_0, $KEY_4, $KEY_5, $KEY_6;
		function $THREAD_1_141834() {
			let $F = $KEY_6;
			$KEY_6 = ($F + 1) % 4;
			if (($F + 1) % 4 === 0) {
				$KEY_5 = $KEY_4 + $KEY_0;
				let $G = $KEY_0;
				$KEY_0 = $G ^ $KEY_4;
				$KEY_4 = $G ^ $KEY_4 ^ ($KEY_4 << 26 | $KEY_4 >>> 6) ^ $KEY_0 << 9;
				$KEY_0 = $KEY_0 << 13 | $KEY_0 >>> 19;
				return $KEY_5 & 255;
			}
			if (($F + 1) % 4 === 1) {
				return $KEY_5 >> 8 & 255;
			}
			if (($F + 1) % 4 === 2) {
				return $KEY_5 >> 16 & 255;
			}
			if (($F + 1) % 4 === 3) {
				return $KEY_5 >>> 24 & 255;
			} else {
				return void 0;
			}
		}
		$KEY_4 = $KEY_2;
		$KEY_0 = $E;
		$KEY_6 = -1;
		$THREAD_1_141834();
		$THREAD_1_141834();
		$THREAD_1_141834();
		$THREAD_1_141834();
		return $THREAD_1_141834;
	}
	$KEY_8 = $THREAD_6_161183;
	Object.prototype.hasOwnProperty;
	$KEY_18 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_0 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_16 = $KEY_0 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_12 = $KEY_16 ? null : [0, 0, 0, 0, 0, 0, 248, 127];
	let $C = $B.a.length >> 1 << 1;
	let i = 0;
	let $D = 0;
	for (; i < $C; i = i + 2) {
		$D = ($D << 5) - $D + ($B.a[i].charCodeAt() << 16 | $B.a[i + 1].charCodeAt()) | 0;
	}
	$KEY_2 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;
	function $THREAD_1_120530() {
		let $KEY_0;
		function $THREAD_5_21896($K) {
			if ($K == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($K).length + $KEY_0(Object.getPrototypeOf($K));
			}
		}
		$KEY_0 = $THREAD_5_21896;
		let $E = Math.random() * 4294967296;
		let $F = [($E | 0) >>> 24, ($E | 0) >> 16 & 255, ($E | 0) >> 8 & 255, ($E | 0) & 255];
		$E = $KEY_8(($E | 0) ^ 3600357980);
		let $KEY_6 = $KEY_1($E);
		let $G = navigator.hardwareConcurrency;
		if ($G == null) {
			$F[4] = $E() ^ 130;
			$G = 4;
		} else {
			$G = $KEY_9($E, $G, $F, 3);
		}
		$E = $KEY_9($E, $KEY_0(self), $F, $G);
		$E = $KEY_6(location.origin, $F, $E);
		let $KEY_3 = $KEY_6(navigator.userAgent, $F, $E);
		$KEY_6(navigator.platform, $F, $KEY_3);
		let $H = postMessage;
		let $I = { a: $KEY_5 };
		let $J = [[], $F];
		$I.b = $J;
		$H($I);
	}
	$THREAD_1_120530($B.b);
}"###;
const CASE_1: &str = r###"function $THREAD_3_158824($A, $B) {
	let $KEY_9 = $A;
	let $KEY_10, $KEY_11, $KEY_13, $KEY_14, $KEY_18, $KEY_20;
	function $THREAD_4_40814($F) {
		let $KEY_0, $KEY_3, $KEY_4, $KEY_5;
		function $THREAD_0_48762() {
			let $G = $KEY_4;
			$KEY_4 = ($G + 1) % 4;
			if (($G + 1) % 4 === 0) {
				$KEY_5 = $KEY_3 + $KEY_0;
				let $H = $KEY_0;
				$KEY_0 = $H ^ $KEY_3;
				$KEY_3 = $H ^ $KEY_3 ^ ($KEY_3 << 26 | $KEY_3 >>> 6) ^ $KEY_0 << 9;
				$KEY_0 = $KEY_0 << 13 | $KEY_0 >>> 19;
				return $KEY_5 & 255;
			}
			if (($G + 1) % 4 === 1) {
				return $KEY_5 >> 8 & 255;
			}
			if (($G + 1) % 4 === 2) {
				return $KEY_5 >> 16 & 255;
			}
			if (($G + 1) % 4 === 3) {
				return $KEY_5 >>> 24 & 255;
			} else {
				return void 0;
			}
		}
		$KEY_3 = $KEY_14;
		$KEY_0 = $F;
		$KEY_4 = -1;
		$THREAD_0_48762();
		$THREAD_0_48762();
		$THREAD_0_48762();
		$THREAD_0_48762();
		return $THREAD_0_48762;
	}
	let $KEY_7;
	function $THREAD_8_74386($F, $G, $H, $I) {
		let $KEY_10;
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
				$K = 64;
				$L = -$G;
			} else {
				$K = 0;
				$L = $G;
			}
			if ($L < 32) {
				$KEY_10 = Number($I) + 1;
				$H[$KEY_10] = ($K | $L) ^ $F();
			} else if ($L < 4096) {
				$KEY_10 = Number($I) + 1;
				$H[$KEY_10] = ($K | ($L & 31 | 32)) ^ $F();
				$H[++$KEY_10] = $L >> 5 ^ $F();
			} else if ($L < 524288) {
				$KEY_10 = Number($I) + 1;
				$H[$KEY_10] = ($K | ($L & 31 | 32)) ^ $F();
				$H[++$KEY_10] = ($L >> 5 & 127 | 128) ^ $F();
				$H[++$KEY_10] = $L >>> 12 ^ $F();
			} else if ($L < 67108864) {
				$KEY_10 = Number($I) + 1;
				$H[$KEY_10] = ($K | ($L & 31 | 32)) ^ $F();
				$H[++$KEY_10] = ($L >> 5 & 127 | 128) ^ $F();
				$H[++$KEY_10] = ($L >>> 12 & 127 | 128) ^ $F();
				$H[++$KEY_10] = $L >>> 19 ^ $F();
			} else {
				$KEY_10 = Number($I) + 1;
				$H[$KEY_10] = ($K | ($L & 31 | 32)) ^ $F();
				$H[++$KEY_10] = ($L >> 5 & 127 | 128) ^ $F();
				$H[++$KEY_10] = ($L >>> 12 & 127 | 128) ^ $F();
				$H[++$KEY_10] = ($L >>> 19 & 127 | 128) ^ $F();
				$H[++$KEY_10] = $L >>> 26 ^ $F();
			}
		} else {
			$KEY_10 = $I;
			$KEY_10 = Number($I) + 1;
			$H[$KEY_10] = $F() ^ 128;
			let $M;
			if ($KEY_20) {
				$KEY_20[0] = $G;
				$M = new $KEY_18($KEY_20.buffer);
			} else if ($G !== $G) {
				$M = $KEY_7;
			} else {
				let $KEY_9 = $G < 0 || $G === 0 && 1 / $G < 0;
				let $N = Math.abs($G);
				let $O, $KEY_5;
				if ($N >= Math.pow(2, -1022)) {
					let $P = Math.min(Math.floor(Math.log($N) / Math.LN2), 1023);
					$N = $N / Math.pow(2, $P);
					let $Q = $N < 1;
					$P = $Q ? $P - 1 : $P;
					$Q = $Q ? $N * 2 : $N;
					let $R = $Q >= 2;
					$Q = $R ? $Q / 2 : $Q;
					$KEY_5 = Math.pow(2, 52);
					let $KEY_14 = Math.floor($Q * $KEY_5);
					$R = ($R ? $P + 1 : $P) + 1023;
					let $S = (($Q * $KEY_5 - $KEY_14 < .5 ? $KEY_14 : $Q * $KEY_5 - $KEY_14 > .5 ? $KEY_14 + 1 : $KEY_14 % 2 ? $KEY_14 + 1 : $KEY_14) - $KEY_5) / $KEY_5 >= 1;
					$O = $S ? $R + 1 : $R;
					if ($O > 2046) {
						$O = 2047;
						$KEY_5 = 0;
					} else {
						$KEY_5 = $S ? 0 : ($Q * $KEY_5 - $KEY_14 < .5 ? $KEY_14 : $Q * $KEY_5 - $KEY_14 > .5 ? $KEY_14 + 1 : $KEY_14 % 2 ? $KEY_14 + 1 : $KEY_14) - $KEY_5;
					}
				} else {
					$N = $N / Math.pow(2, -1074);
					let $T = Math.floor($N);
					$O = 0;
					$KEY_5 = $N - $T < .5 ? $T : $N - $T > .5 ? $T + 1 : $T % 2 ? $T + 1 : $T;
				}
				let $U = [];
				let i = 51;
				for (; i >= 0; i--) {
					$U.unshift(!!($KEY_5 & 1));
					$KEY_5 = Math.floor($KEY_5 / 2);
				}
				let j = 11;
				for (; j > 0; j--) {
					$U.unshift(!!($O & 1));
					$O = Math.floor($O / 2);
				}
				$U.unshift($KEY_9);
				$M = [];
				let k = 56;
				for (; k >= 0; k = k - 8) {
					$M.push(Number($U[k]) << 7 | Number($U[k + 1]) << 6 | Number($U[k + 2]) << 5 | Number($U[k + 3]) << 4 | Number($U[k + 4]) << 3 | Number($U[k + 5]) << 2 | Number($U[k + 6]) << 1 | Number($U[k + 7]));
				}
			}
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$H[++$KEY_10] = $M[i_2] ^ $F();
			}
		}
		return $KEY_10;
	}
	let $KEY_5;
	function $THREAD_8_81550($F) {
		let $KEY_2 = $F;
		let $KEY_0 = Object.create(null);
		let $KEY_1 = 0;
		function $THREAD_3_25127($G, $H, $I) {
			let $KEY_8;
			if ($KEY_0[$G] != null) {
				$KEY_8 = $I;
				$KEY_8 = Number($I) + 1;
				$H[$KEY_8] = $KEY_2() ^ 129;
				$KEY_8 = $KEY_11($KEY_2, $KEY_0[$G], $H, $KEY_8);
			} else {
				let $J = $G.length;
				let $K = 0;
				let $L = $I;
				while ($K < $J) {
					let $M = $KEY_5($G[$K], 0);
					if ($M === 0) {
						$H[++$L] = $KEY_2() ^ 192;
						$M = 128;
					} else if (!($M < 128)) if ($M < 2048) {
						$H[++$L] = ($M >> 6 | 192) ^ $KEY_2();
						$M = $M & 63 | 128;
					} else {
						let $N = $M >= 55296 && $M <= 57343 && $M < 56320 && $K + 1 < $J ? $KEY_5($G[$K + 1], 0) : null;
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
				$KEY_8 = Number($L) + 1;
				$H[$KEY_8] = $KEY_2();
				let $O = $KEY_0;
				let $P = Number($KEY_1);
				$KEY_1 = $P + 1;
				$O[$G] = $P;
			}
			return $KEY_8;
		}
		return $THREAD_3_25127;
	}
	$KEY_13 = $THREAD_8_81550;
	$KEY_11 = $THREAD_8_74386;
	$KEY_10 = $THREAD_4_40814;
	Object.prototype.hasOwnProperty;
	$KEY_5 = Array.call.bind(String.prototype.charCodeAt);
	$KEY_18 = typeof Uint8Array === "function" ? Uint8Array : null;
	$KEY_20 = $KEY_18 && typeof Float64Array === "function" ? new Float64Array(1) : null;
	$KEY_7 = $KEY_20 ? null : (($PARAMA, $PARAMB) => {
		$PARAMA[$PARAMB] = 248;
		$PARAMA[7] = 127;
		return $PARAMA;
	})(([0, 0, 0, 0, 0, 0]), 6);
	let $C = $B.a.length >> 1 << 1;
	let $D = 0;
	let $E = 0;
	while ($D < $C) {
		$E = ($E << 5) - $E + ($B.a[$D].charCodeAt() << 16 | $B.a[$D + 1].charCodeAt()) | 0;
		$D += 2;
	}
	$KEY_14 = $C !== $B.a.length ? ($E << 5) - $E + ($B.a[$C].charCodeAt() << 16) | 0 : $E;
	function $THREAD_4_63004() {
		let $KEY_3;
		function $THREAD_6_62308($L) {
			if ($L == null) {
				return 0;
			} else {
				return Object.getOwnPropertyNames($L).length + $KEY_3(Object.getPrototypeOf($L));
			}
		}
		$KEY_3 = $THREAD_6_62308;
		let $F = Math.random() * 4294967296;
		let $G = [($F | 0) >>> 24, ($F | 0) >> 16 & 255, ($F | 0) >> 8 & 255, ($F | 0) & 255];
		let $KEY_2 = $KEY_10(($F | 0) ^ 3229010996);
		let $KEY_5 = $KEY_13($KEY_2);
		let $H = navigator.hardwareConcurrency;
		if ($H == null) {
			$G[4] = $KEY_2() ^ 130;
			$H = 4;
		} else {
			$H = $KEY_11($KEY_2, $H, $G, 3);
		}
		$KEY_2 = $KEY_11($KEY_2, $KEY_3(self), $G, $H);
		$KEY_2 = $KEY_5(navigator.userAgent, $G, $KEY_2);
		let $KEY_7 = $KEY_5(navigator.platform, $G, $KEY_2);
		$KEY_5(location.origin, $G, $KEY_7);
		let $I = postMessage;
		let $J = { a: $KEY_9 };
		let $K = [[], $G];
		$J.b = $K;
		$I($J);
	}
	$THREAD_4_63004($B.b);
}"###;
fn check(body: &str, accepted: bool) {
    // Keep the posted worker declaration unreferenced, as on the real input.
    let source = format!("function main(){{}} main();\n{body}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, &source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        !parsed.panicked && parsed.errors.is_empty(),
        "{:?}",
        parsed.errors
    );
    let result = name_threads(&source, true).expect("static worker fixture");
    for role in [
        "createSeededWorkerEnvironmentBundle",
        "emitSeededWorkerEnvironmentBundle",
    ] {
        let outcomes: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| {
                o.applied
                    && o.role_name.as_deref() == Some(role)
                    && o.evaluator.as_deref() == Some("worker_environment_bundle")
            })
            .collect();
        assert_eq!(outcomes.len(), usize::from(accepted), "{role}: {body}");
    }
}
fn mutant(body: &str, from: &str, to: &str) -> String {
    assert_eq!(body.matches(from).count(), 1, "mutation anchor {from}");
    body.replacen(from, to, 1)
}
#[test]
fn owned_terminal_string_cursor_chain_variants() {
    for source in [CASE_0, CASE_1] {
        check(source, true);
        check(
            &source
                .replace("$KEY_", "$LOCAL_")
                .replace("3600357980", "117"),
            true,
        );
    }
}
#[test]
fn cursor_chain_rejects_wrong_stream_header_and_nested_donors() {
    const NEXT: &str = "$E = $KEY_6(location.origin, $F, $E);";
    for body in [
        mutant(CASE_0, NEXT, "$E = $KEY_6(location.origin, [], $E);"),
        mutant(CASE_0, NEXT, "$E = $KEY_6(location.origin, $F, 3);"),
        mutant(CASE_0, NEXT, "$E = $KEY_6(location.origin, $F, $G);"),
        mutant(CASE_0, NEXT, "$E = 0;"),
        mutant(CASE_0, NEXT, &format!("$E = 0; {NEXT}")),
        mutant(CASE_0, NEXT, &format!("{NEXT} $E = 0;")),
        mutant(CASE_0, NEXT, &format!("if (false) {{ {NEXT} }}")),
        mutant(CASE_0, NEXT, &format!("(() => {{ {NEXT} }});")),
        mutant(
            CASE_0,
            NEXT,
            &format!("function donor() {{ return $E; }} {NEXT}"),
        ),
        mutant(CASE_0, NEXT, &format!("{NEXT} $E();")),
        mutant(CASE_0, NEXT, &format!("{NEXT} (() => $E());")),
        mutant(CASE_0, NEXT, &format!("{NEXT} leak($E);")),
        mutant(
            CASE_0,
            NEXT,
            &format!("{NEXT} $KEY_6(location.origin, $F, $E);"),
        ),
        mutant(
            CASE_0,
            "$KEY_9($E, $KEY_0(self), $F, $G)",
            "$KEY_9($E, $KEY_0(window), $F, $G)",
        ),
        mutant(
            CASE_0,
            "let $KEY_3 = $KEY_6(navigator.userAgent, $F, $E);",
            "let $KEY_3 = $KEY_6(navigator.userAgent, [], $E);",
        ),
        mutant(
            CASE_0,
            "let $KEY_3 = $KEY_6(navigator.userAgent, $F, $E);",
            "let $KEY_3 = $KEY_6(navigator.userAgent, $F, 3);",
        ),
    ] {
        check(&body, false);
    }
}
