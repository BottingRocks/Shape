		function createSignalPayloadEncoder($BS, $BT, $BU, $BV) {
			let $KEY_0 = $BS;
			let $KEY_7 = $BV;
			let $KEY_9;
			let $BW = {
				keysetsToOmit: $BT,
				depthBound: 100,
				extensions: void 0
			};
			let $BX = { "0": $KEY_93 };
			let $BY = $KEY_395;
			if ("3735928559" in $BX) {
				$BY = {
					value: $BY,
					enumerable: true,
					configurable: true,
					writable: true
				};
				Object.defineProperty($BX, "3735928559", $BY);
			} else {
				$BX[3735928559] = $BY;
			}
			$BW.extensions = $BX;
			$KEY_9 = $BW;
			function serializeSignalsToBase64($BZ) {
				let $CA = [];
				let $KEY_4 = $BZ.errors.length === 0 ? [] : $KEY_397($BZ.errors);
				let $CB = [];
				let $CC;
				if ($KEY_4.length >>> 0 < 128) {
					$CB[0] = $KEY_4.length >>> 0;
					$CC = void 0;
				} else {
					let i = -1;
					let j = $KEY_4.length >>> 0 >>> 7;
					let k = $KEY_4.length >>> 0 & 127;
					let $CD, $CE, $CF;
					do {
						$CD = Number(i) + 1;
						$CB[$CD] = k | 128;
						$CE = j & 127;
						$CF = j >>> 7;
					} while ($CF > 0 && (i = $CD, j = $CF, k = $CE, true));
					$CB[$CD + 1] = $CE;
					$CC = $CB;
				}
				let i_2 = 0;
				for (; i_2 < $CB.length; i_2++) {
					$CA.push($CB[i_2]);
				}
				let j_2 = 0;
				for (; j_2 < $KEY_4.length; j_2++) {
					$CA.push($KEY_4[j_2]);
				}
				let j_7 = 0;
				for (; j_7 < $BZ.positionalSignals.length; j_7++) {
					let $CG;
					if ($BZ.positionalSignals[j_7] === $KEY_300) {
						$CA.push(0);
						$CA.push(0);
						$CG = $KEY_4;
					} else {
						const $CH = $BZ.positionalSignals[j_7][0].length === 0 ? [] : $KEY_397($BZ.positionalSignals[j_7][0]);
						let $CI = [];
						let $CJ, $CK, $CL, $CM;
						if ($CH.length >>> 0 < 128) {
							$CK = 0;
							$CL = $CH.length >>> 0;
							$CM = $KEY_4;
						} else {
							let $CN = $CH.length >>> 0 & 127;
							let $CO = -1;
							let $CP = $CH.length >>> 0 >>> 7;
							for (;;) {
								$CM = Number($CO) + 1;
								$CI[$CM] = $CN | 128;
								if ($CP >>> 7 <= 0) break;
								$CN = $CP & 127;
								$CO = $CM;
								$CP = $CP >>> 7;
							}
							$CK = $CM + 1;
							$CL = $CP & 127;
						}
						$CJ = $CI;
						$CJ[$CK] = $CL;
						let k_2 = 0;
						for (; k_2 < $CI.length; k_2++) {
							$CA.push($CI[k_2]);
						}
						let i_3 = 0;
						for (; i_3 < $CH.length; i_3++) {
							$CA.push($CH[i_3]);
						}
						$CB = [];
						let $CQ, $CR, $CS;
						if ($BZ.positionalSignals[j_7][1].length >>> 0 < 128) {
							$CR = 0;
							$CS = $BZ.positionalSignals[j_7][1].length >>> 0;
							$CG = $CM;
						} else {
							let $CT = $BZ.positionalSignals[j_7][1].length >>> 0 >>> 7;
							let $CU = $BZ.positionalSignals[j_7][1].length >>> 0 & 127;
							let $CV = -1;
							for (;;) {
								$CB[++$CV] = $CU | 128;
								$CS = $CT & 127;
								if ($CT >>> 7 <= 0) break;
								$CT = $CT >>> 7;
								$CU = $CS;
							}
							$CR = $CV + 1;
							$CG = $CB;
						}
						$CQ = $CB;
						$CQ[$CR] = $CS;
						let j_3 = 0;
						for (; j_3 < $CB.length; j_3++) {
							$CA.push($CB[j_3]);
						}
						let k_3 = 0;
						for (; k_3 < $BZ.positionalSignals[j_7][1].length; k_3++) {
							$CA.push($BZ.positionalSignals[j_7][1][k_3]);
						}
						$CC = $BZ.positionalSignals[j_7][1];
					}
					$KEY_4 = $CG;
				}
				const $CW = $BZ.signals.length === 0 ? [] : $KEY_397($BZ.signals, $KEY_9);
				let $CX = [];
				let $CY, $CZ, $DA, $DB, $DC;
				if ($CW.length >>> 0 < 128) {
					$CZ = 0;
					$DA = $CW.length >>> 0;
					$DB = $CW.length >>> 0;
					$DC = $CC;
				} else {
					let $DD = $CW.length >>> 0 & 127;
					let $DE = $CW.length >>> 0 >>> 7;
					let $DF = -1;
					for (;;) {
						$DC = Number($DF) + 1;
						$CX[$DC] = $DD | 128;
						if ($DE >>> 7 <= 0) break;
						$DD = $DE & 127;
						$DE = $DE >>> 7;
						$DF = $DC;
					}
					$CZ = $DC + 1;
					$DA = $DE & 127;
					$DB = $DE >>> 7;
				}
				$CY = $CX;
				$CY[$CZ] = $DA;
				let i_4 = 0;
				for (; i_4 < $CX.length; i_4++) {
					$CA.push($CX[i_4]);
				}
				let j_4 = 0;
				for (; j_4 < $CW.length; j_4++) {
					$CA.push($CW[j_4]);
				}
				let $DG = 0;
				while ($DG < $BZ.keyedSignals.length) {
					let $DH = [];
					let $DI, $DJ, $DK, $DL, $DM;
					if ($BZ.keyedSignals[$DG][0] >>> 0 < 128) {
						$DJ = 0;
						$DK = $BZ.keyedSignals[$DG][0] >>> 0;
						$DL = $DB;
						$DM = $DC;
					} else {
						let $DN = $BZ.keyedSignals[$DG][0] >>> 0 >>> 7;
						let $DO = $BZ.keyedSignals[$DG][0] >>> 0 & 127;
						let $DP = -1;
						for (;;) {
							$DM = Number($DP) + 1;
							$DH[$DM] = $DO | 128;
							$DK = $DN & 127;
							if ($DN >>> 7 <= 0) break;
							$DN = $DN >>> 7;
							$DO = $DK;
							$DP = $DM;
						}
						$DJ = $DM + 1;
						$DL = $DH;
					}
					$DI = $DH;
					$DI[$DJ] = $DK;
					let k_4 = 0;
					for (; k_4 < $DH.length; k_4++) {
						$CA.push($DH[k_4]);
					}
					if ($BZ.keyedSignals[$DG][1] === $KEY_300) {
						$CA.push(0);
						$CA.push(0);
						$DB = $DL;
						$DC = $DM;
					} else {
						const $DQ = $BZ.keyedSignals[$DG][1][0].length === 0 ? [] : $KEY_397($BZ.keyedSignals[$DG][1][0]);
						let $DR = [];
						let $DS, $DT;
						if ($DQ.length >>> 0 < 128) {
							$DS = 0;
							$DT = $DQ.length >>> 0;
						} else {
							let i_5 = $DQ.length >>> 0 & 127;
							let j_5 = $DQ.length >>> 0 >>> 7;
							let k_5 = -1;
							let $DU, $DV, $DW;
							do {
								$DU = Number(k_5) + 1;
								$DR[$DU] = i_5 | 128;
								$DV = j_5 & 127;
								$DW = j_5 >>> 7;
							} while ($DW > 0 && (i_5 = $DV, j_5 = $DW, k_5 = $DU, true));
							$DS = $DU + 1;
							$DT = $DV;
							$DM = $DR;
						}
						$DR[$DS] = $DT;
						let i_6 = 0;
						for (; i_6 < $DR.length; i_6++) {
							$CA.push($DR[i_6]);
						}
						let j_6 = 0;
						for (; j_6 < $DQ.length; j_6++) {
							$CA.push($DQ[j_6]);
						}
						let $DX = [];
						let $DY, $DZ, $EA;
						if ($BZ.keyedSignals[$DG][1][1].length >>> 0 < 128) {
							$DZ = 0;
							$EA = $BZ.keyedSignals[$DG][1][1].length >>> 0;
							$DB = $BZ.keyedSignals[$DG][1][1].length >>> 0;
							$DC = $DM;
						} else {
							let $EB = $BZ.keyedSignals[$DG][1][1].length >>> 0 >>> 7;
							let $EC = -1;
							let $ED = $BZ.keyedSignals[$DG][1][1].length >>> 0 & 127;
							let $EE;
							for (;;) {
								$DX[++$EC] = $ED | 128;
								$EE = $EB;
								if ($EB >>> 7 <= 0) break;
								$EB = $EB >>> 7;
								$ED = $EE & 127;
							}
							$DZ = $EC + 1;
							$EA = $EE & 127;
							$DB = $DX;
							$DC = $EE & 127;
						}
						$DY = $DX;
						$DY[$DZ] = $EA;
						let k_6 = 0;
						for (; k_6 < $DX.length; k_6++) {
							$CA.push($DX[k_6]);
						}
						let i_7 = 0;
						for (; i_7 < $BZ.keyedSignals[$DG][1][1].length; i_7++) {
							$CA.push($BZ.keyedSignals[$DG][1][1][i_7]);
						}
					}
					++$DG;
				}
				let $EF;
				if ($KEY_77 != null && $KEY_7 != null) {
					let $EG = Math.random() * 4294967296 >>> 0;
					let $EH = 0;
					try {
						$EH = performance.now() >>> 0;
					} catch (e) {}
					let $EI = Number(new Date()) >>> 0;
					let $EJ = 0;
					try {
						$EJ = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_2) {}
					$EG = $EG ^ 1103515245;
					$EH = $EH === $EH ? $EG ^ $EH : $EG;
					$EI = $EI === $EI ? $EH ^ $EI : $EH;
					++$KEY_293;
					$EJ = ($EI ^ $EJ ^ $KEY_293) >>> 0;
					let $EK = Math.random() * 4294967296 >>> 0;
					let $EL = 0;
					try {
						$EL = performance.now() >>> 0;
					} catch (e_3) {}
					let $EM = Number(new Date()) >>> 0;
					let $EN = 0;
					try {
						$EN = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_4) {}
					$EK = $EK ^ 1103515245;
					$EL = $EL === $EL ? $EK ^ $EL : $EK;
					$EM = $EM === $EM ? $EL ^ $EM : $EL;
					++$KEY_293;
					$EN = ($EM ^ $EN ^ $KEY_293) >>> 0;
					$EF = $KEY_77($CA, $KEY_7, $EJ, $EN);
					$EF.unshift($EJ >>> 24, $EJ >>> 16 & 255, $EJ >>> 8 & 255, $EJ & 255, $EN >>> 24, $EN >>> 16 & 255, $EN >>> 8 & 255, $EN & 255);
				} else {
					$EF = $CA;
				}
				let $EO = $KEY_0 ?? "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=";
				let $EP = $EF.length - $EF.length % 3;
				let $EQ = 0;
				let $ER = "";
				while ($EQ < $EP) {
					let $ES = $EF[$EQ + 2];
					$ER = $ER + ($EO.charAt($EF[$EQ] >> 2) + $EO.charAt(($EF[$EQ] & 3) << 4 | $EF[$EQ + 1] >> 4) + $EO.charAt(($EF[$EQ + 1] & 15) << 2 | $ES >> 6) + $EO.charAt($ES & 63));
					$EQ += 3;
				}
				if ($EF.length % 3 === 2) {
					return $ER + ($EO.charAt($EF[$EQ] >> 2) + $EO.charAt(($EF[$EQ] & 3) << 4 | $EF[$EQ + 1] >> 4) + $EO.charAt(($EF[$EQ + 1] & 15) << 2));
				}
				if ($EF.length % 3 === 1) {
					$EF = $EF[$EQ];
					return $ER + ($EO.charAt($EF >> 2) + $EO.charAt(($EF & 3) << 4));
				} else {
					return $ER;
				}
			}
			return serializeSignalsToBase64;
		}