//                              Route canonicalizer with opaque captured helpers.
function bootstrap() {
var $KEY_32 = { anchorHrefSet: function(){}, anchorHrefGet: function(){}, schedule: function(){}, test: function(){ return true; } };
var $KEY_390 = { anchorHrefSet: function(){}, anchorHrefGet: function(){}, schedule: function(){}, test: function(){ return true; } };
var $KEY_312 = 0;
var $KEY_316 = 0;
var $KEY_401 = 0;
var $KEY_382 = 0;
var $KEY_259 = 0;
var $KEY_379 = 0;
var $KEY_210 = 0;
var $KEY_116 = 0;
function $THREAD_1_33315($BP, $BQ, $BR, $BS) {
			let $KEY_5, $KEY_6, $KEY_7, $KEY_9, $KEY_12, $KEY_22, $KEY_23;
			let $KEY_1 = $BP.url || window.location + "";
			let $BT = RegExp("^https?:\\/\\/").test($KEY_1);
			if ($BT) {
				if ($KEY_32[$KEY_1] != null) {
					$KEY_6 = $KEY_32[$KEY_1];
					let $BU, $BV, $BW;
					if ($KEY_32[$KEY_1].hostname && /[xX][nN]--/.test($KEY_6.hostname)) {
						let $BX = $KEY_6;
						let $KEY_16 = $KEY_6.hostname;
						if ($KEY_16 == null) {
							throw new Error("Bad Input: Input cannot be null");
						}
						if ($KEY_16.toLowerCase == null) {
							throw new Error("Unknown Input");
						}
						if ($KEY_16.length > 63) {
							throw new Error("Bad Input: Maximum domain length cannot be greater than 63");
						} else {
							$KEY_16 = [$KEY_16, $KEY_16.split];
							let $BY = $KEY_16[1].call($KEY_16[0], ".");
							let $BZ = [];
							let i = 0;
							let $CA, $CB, $CC, $CD;
							for (; i < $BY.length; i++) {
								let $CE, $CF, $CG;
								if (/^[xX][nN]--/.test($BY[i])) {
									if (!$BY[i].length) {
										throw new Error("Bad Input: Zero length punycodes not allowed");
									}
									if ($BY[i][$BY[i].length - 1] === "-") {
										throw new Error("Bad Input: Host name labels cannot end with a hyphen-minus");
									} else {
										let $CH = $BY[i].slice("xn--".length);
										let $CI = Array.prototype.map.call($CH.toLowerCase(), $KEY_401);
										function $THREAD_6_139329($EM) {
											return $EM >= $KEY_259 && $EM <= $KEY_379 || $EM >= $KEY_210 && $EM <= $KEY_116 || $EM === 45;
										}
										if (!$CI.every($THREAD_6_139329)) {
											throw new Error("Bad Input: Input contains invalid code points");
										}
										$CF = $BZ.push;
										let $CJ = $KEY_382;
										let $CK = $CI.length;
										let $CL = [$CI, $CI.lastIndexOf];
										let $CM = $CL[1].call($CL[0], 45);
										if ($CM > 253) {
											throw new Error("Bad Input: Delimiter index can't exceed maximum output string length limit(253).");
										}
										const $CN = $CM >= 0 ? $CI.slice(0, $CM) : [];
										let $CO = 128;
										let $CP = 72;
										$CA = 0;
										++$CM;
										while ($CM < $CK) {
											let $CQ = 1;
											let $CR = 36;
											let $CS = $CA;
											let $CT = $CM;
											for (;;) {
												let $CU = $CI[$CT] >= $KEY_210 && $CI[$CT] <= $KEY_116;
												$CD = void 0;
												if ($CU) {
													$CD = $CI[$CT] - $KEY_210;
												} else {
													let $CV = $CI[$CT] >= $KEY_259 && $CI[$CT] <= $KEY_379;
													$CD = $CV ? $CI[$CT] - $KEY_259 + 26 : 36;
												}
												$CM = Number($CT) + 1;
												if ($CD >= 36) throw new Error("Bad Input: Decoded digit value cannot be greater than base.");
												if ($CD > (9007199254740991 - $CS) / $CQ) {
													throw new Error("Overflow: Decoded digit value cannot be greater than allowed positional value");
												}
												let $CW = $CS + $CD * $CQ;
												$CC = $CR <= $CP ? 1 : $CR >= $CP + 26 ? 26 : $CR - $CP;
												if ($CD < $CC) {
													$CB = $CN.length;
													const $CX = $CA === 0 ? Math.floor(($CW - $CA) / 700) : $CW - $CA >> 1;
													let $CY = 0;
													let $CZ = $CX + Math.floor($CX / ($CB + 1));
													while ($CZ > 455) {
														$CZ = Math.floor($CZ / 35);
														$CY += 36;
													}
													$CP = Math.floor($CY + $CZ * 36 / ($CZ + 38));
													if ($CW / ($CB + 1) > 9007199254740991 - $CO) {
														throw new Error("Overflow: Unexpected increment");
													}
													$CO = $CO + Math.floor($CW / ($CB + 1));
													if ($CB >= 253) {
														throw new Error("Overflow: Output string length cannot be greater than MAX_OUT(253)");
													} else {
														$CN.splice($CW % ($CB + 1), 0, Math.floor($CO));
														$CA = Number($CW % ($CB + 1)) + 1;
														break;
													}
												} else if ($CQ > 257348550135456.88) {
													throw new Error("Overflow: Unexpected weight");
												} else {
													$CQ = $CQ * (36 - $CC);
													$CR += 36;
													$CS = $CW;
													$CT = $CM;
													continue;
												}
											}
										}
										$CG = $CJ($CN);
										$CE = $BZ;
									}
								} else {
									$CE = $BZ;
									$CF = $BZ.push;
									$CG = $BY[i];
								}
								$CF.call($CE, $CG);
							}
							$BX.hostname = $BZ.join(".");
							$BU = $BQ;
							$BV = $BR;
							$BW = $BS;
						}
					} else {
						$BU = $BQ;
						$BV = $BR;
						$BW = $BS;
					}
					$KEY_23 = $BP.method.toLowerCase();
					if (!$BV) {
						$KEY_7 = $BU[0];
						return $BU[1].some($EM => {
							let $EN = $EM[0].some($ES => $ES.toLowerCase() === "any" || $ES.toLowerCase() === $KEY_23);
							if (!$EN) {
								return $EN;
							}
							let $KEY_3 = $KEY_7;
							let $KEY_1 = $KEY_6;
							let $EO = $KEY_3[$EM[1][0]].test($KEY_1.protocol);
							if ($EO) {
								let $EP = [$KEY_3, $EM[1]];
								$EO = $EP[0][$EP[1][1]].test($KEY_1.hostname);
							}
							if ($EO) {
								let $EQ = [$KEY_3, $EM[1]];
								$EO = $EQ[0][$EQ[1][2]].test($KEY_1.port);
							}
							if ($EO) {
								let $ER = [$KEY_3, $EM[1]];
								$EO = $ER[0][$ER[1][3]].test($KEY_1.pathname);
							}
							if ($EO) {
								$KEY_3 = [$KEY_3, $EM[1]];
								return $KEY_3[0][$KEY_3[1][4]].test($KEY_1.search);
							} else {
								return $EO;
							}
						});
					}
					try {
						if (!$BW) {
							return false;
						}
						$BW = $BW();
						if ($BW) {
							let $DA = $BW.map($EM => {
								let $EN = $KEY_316;
								let $EO = { port: $EM.port == null ? RegExp("(?:)") : $EM.port };
								return $EN({}, $EM, $EO);
							});
							$KEY_12 = $KEY_6.protocol;
							$KEY_5 = $KEY_6.hostname;
							$KEY_9 = $KEY_6.pathname;
							$KEY_22 = $KEY_6.port;
							$KEY_9 = decodeURIComponent($KEY_9);
							return $DA.some($EM => {
								let $EN;
								if (Array.isArray($EM.method)) {
									$EN = $EM.method;
								} else {
									$EN = [$EM.method];
								}
								let $EO = $EN.map($ER => $ER.toLowerCase());
								let $EP = [$EO, $EO.indexOf];
								let $EQ;
								if ($EP[1].call($EP[0], $KEY_23) >= 0) {
									$EQ = true;
								} else {
									$EO = [$EO, $EO.indexOf];
									$EQ = $EO[1].call($EO[0], "any") >= 0;
								}
								$EQ = $EQ && $EM.protocol.test($KEY_12) && $EM.host.test($KEY_5) && $EM.path.test($KEY_9);
								if ($EQ) {
									return $EM.port.test($KEY_22);
								} else {
									return $EQ;
								}
							});
						} else {
							return false;
						}
					} catch (e) {
						return false;
					}
				}
			}
			let $DB = $KEY_390;
			let $KEY_28 = document.createElement("a");
			$DB.anchorHrefSet($KEY_28, $KEY_1);
			if ($KEY_28.host === "") {
				$DB.anchorHrefSet($KEY_28, $DB.anchorHrefGet($KEY_28));
			}
			let $DC = $KEY_28.protocol.slice(0, $KEY_28.protocol.length - 1);
			let $DD;
			if ($KEY_28.pathname[0] === "/") {
				$DD = $KEY_28.pathname.replace(RegExp("(%(2[146-9a-f]|3[0-9abd]|4[0-9a-f]|5[0-9abdf]|6[1-9a-f]|7[0-9ae]))", "gi"), $EM => decodeURIComponent($EM));
			} else {
				$DD = "/" + $KEY_28.pathname.replace(RegExp("(%(2[146-9a-f]|3[0-9abd]|4[0-9a-f]|5[0-9abdf]|6[1-9a-f]|7[0-9ae]))", "gi"), $EM => decodeURIComponent($EM));
			}
			let $KEY_24 = $KEY_28.search.replace(RegExp("(%(2[146-9a-f]|3[0-9abd]|4[0-9a-f]|5[0-9abdf]|6[1-9a-f]|7[0-9ae]))", "gi"), $EM => decodeURIComponent($EM)).slice(1);
			let $KEY_27 = $KEY_28.hash.replace(RegExp("(%(2[146-9a-f]|3[0-9abd]|4[0-9a-f]|5[0-9abdf]|6[1-9a-f]|7[0-9ae]))", "gi"), $EM => decodeURIComponent($EM)).slice(1);
			let $KEY_20 = $KEY_28.port === $KEY_312[$DC] ? "" : $KEY_28.port;
			let $DE = {
				protocol: $DC,
				hostname: $KEY_28.hostname,
				port: $KEY_20,
				pathname: $DD,
				search: $KEY_24
			};
			$DE.hash = $KEY_27;
			$DE.href = $DC + "://" + $KEY_28.hostname + ($KEY_20.length > 0 ? ":" + $KEY_20 : "") + $DD + ($KEY_24.length > 0 ? "?" + $KEY_24 : "") + ($KEY_27.length > 0 ? "#" + $KEY_27 : "");
			if ($BT) {
				$KEY_32[$KEY_1] = $DE;
			}
			$KEY_6 = $DE;
			let $DF, $DG, $DH;
			if ($DE.hostname && /[xX][nN]--/.test($KEY_6.hostname)) {
				let $DI = $KEY_6;
				let $DJ = $KEY_6.hostname;
				if ($DJ == null) {
					throw new Error("Bad Input: Input cannot be null");
				}
				if ($DJ.toLowerCase == null) {
					throw new Error("Unknown Input");
				}
				if ($DJ.length > 63) {
					throw new Error("Bad Input: Maximum domain length cannot be greater than 63");
				} else {
					$DJ = [$DJ, $DJ.split];
					let $DK = $DJ[1].call($DJ[0], ".");
					let $DL = [];
					let j = 0;
					let $DM, $DN, $DO;
					for (; j < $DK.length; j++) {
						let $DP, $DQ, $DR;
						if (/^[xX][nN]--/.test($DK[j])) {
							if (!$DK[j].length) {
								throw new Error("Bad Input: Zero length punycodes not allowed");
							}
							if ($DK[j][$DK[j].length - 1] === "-") {
								throw new Error("Bad Input: Host name labels cannot end with a hyphen-minus");
							} else {
								let $DS = $DK[j].slice("xn--".length);
								let $DT = Array.prototype.map.call($DS.toLowerCase(), $KEY_401);
								function $THREAD_6_139329_2($EM) {
									return $EM >= $KEY_259 && $EM <= $KEY_379 || $EM >= $KEY_210 && $EM <= $KEY_116 || $EM === 45;
								}
								if (!$DT.every($THREAD_6_139329_2)) {
									throw new Error("Bad Input: Input contains invalid code points");
								}
								$DQ = $DL.push;
								let $DU = $KEY_382;
								let $DV = $DT.length;
								let $DW = [$DT, $DT.lastIndexOf];
								let $DX = $DW[1].call($DW[0], 45);
								if ($DX > 253) {
									throw new Error("Bad Input: Delimiter index can't exceed maximum output string length limit(253).");
								}
								const $DY = $DX >= 0 ? $DT.slice(0, $DX) : [];
								let $DZ = 128;
								let $EA = 72;
								$DM = 0;
								++$DX;
								while ($DX < $DV) {
									let $EB = 1;
									let $EC = 36;
									let $ED = $DM;
									let $EE = $DX;
									for (;;) {
										let $EF = $DT[$EE] >= $KEY_210 && $DT[$EE] <= $KEY_116;
										$DO = void 0;
										if ($EF) {
											$DO = $DT[$EE] - $KEY_210;
										} else {
											let $EG = $DT[$EE] >= $KEY_259 && $DT[$EE] <= $KEY_379;
											$DO = $EG ? $DT[$EE] - $KEY_259 + 26 : 36;
										}
										$DX = Number($EE) + 1;
										if ($DO >= 36) throw new Error("Bad Input: Decoded digit value cannot be greater than base.");
										if ($DO > (9007199254740991 - $ED) / $EB) {
											throw new Error("Overflow: Decoded digit value cannot be greater than allowed positional value");
										}
										let $EH = $ED + $DO * $EB;
										$DN = $EC <= $EA ? 1 : $EC >= $EA + 26 ? 26 : $EC - $EA;
										if ($DO < $DN) {
											$DE = $DY.length;
											const $EI = $DM === 0 ? Math.floor(($EH - $DM) / 700) : $EH - $DM >> 1;
											let $EJ = 0;
											let $EK = $EI + Math.floor($EI / ($DE + 1));
											while ($EK > 455) {
												$EK = Math.floor($EK / 35);
												$EJ += 36;
											}
											$EA = Math.floor($EJ + $EK * 36 / ($EK + 38));
											if ($EH / ($DE + 1) > 9007199254740991 - $DZ) {
												throw new Error("Overflow: Unexpected increment");
											}
											$DZ = $DZ + Math.floor($EH / ($DE + 1));
											if ($DE >= 253) {
												throw new Error("Overflow: Output string length cannot be greater than MAX_OUT(253)");
											} else {
												$DY.splice($EH % ($DE + 1), 0, Math.floor($DZ));
												$KEY_20 = 34377;
												$DM = Number($EH % ($DE + 1)) + 1;
												break;
											}
										} else if ($EB > 257348550135456.88) {
											throw new Error("Overflow: Unexpected weight");
										} else {
											$EB = $EB * (36 - $DN);
											$EC += 36;
											$ED = $EH;
											$EE = $DX;
											continue;
										}
									}
								}
								$DR = $DU($DY);
								$DP = $DL;
							}
						} else {
							$DP = $DL;
							$DQ = $DL.push;
							$DR = $DK[j];
						}
						$DQ.call($DP, $DR);
					}
					$DI.hostname = $DL.join(".");
					$DF = $BQ;
					$DG = $BR;
					$DH = $BS;
				}
			} else {
				$DF = $BQ;
				$DG = $BR;
				$DH = $BS;
			}
			$KEY_23 = $BP.method.toLowerCase();
			if (!$DG) {
				$KEY_7 = $DF[0];
				return $DF[1].some($EM => {
					let $EN = $EM[0].some($ES => $ES.toLowerCase() === "any" || $ES.toLowerCase() === $KEY_23);
					if (!$EN) {
						return $EN;
					}
					let $KEY_3 = $KEY_7;
					let $KEY_1 = $KEY_6;
					let $EO = $KEY_3[$EM[1][0]].test($KEY_1.protocol);
					if ($EO) {
						let $EP = [$KEY_3, $EM[1]];
						$EO = $EP[0][$EP[1][1]].test($KEY_1.hostname);
					}
					if ($EO) {
						let $EQ = [$KEY_3, $EM[1]];
						$EO = $EQ[0][$EQ[1][2]].test($KEY_1.port);
					}
					if ($EO) {
						let $ER = [$KEY_3, $EM[1]];
						$EO = $ER[0][$ER[1][3]].test($KEY_1.pathname);
					}
					if ($EO) {
						$KEY_3 = [$KEY_3, $EM[1]];
						return $KEY_3[0][$KEY_3[1][4]].test($KEY_1.search);
					} else {
						return $EO;
					}
				});
			}
			try {
				if (!$DH) {
					return false;
				}
				$DH = $DH();
				if ($DH) {
					let $EL = $DH.map($EM => {
						let $EN = $KEY_316;
						let $EO = { port: $EM.port == null ? RegExp("(?:)") : $EM.port };
						return $EN({}, $EM, $EO);
					});
					$KEY_12 = $KEY_6.protocol;
					$KEY_5 = $KEY_6.hostname;
					$KEY_9 = $KEY_6.pathname;
					$KEY_22 = $KEY_6.port;
					$KEY_9 = decodeURIComponent($KEY_9);
					return $EL.some($EM => {
						let $EN;
						if (Array.isArray($EM.method)) {
							$EN = $EM.method;
						} else {
							$EN = [$EM.method];
						}
						let $EO = $EN.map($ER => $ER.toLowerCase());
						let $EP = [$EO, $EO.indexOf];
						let $EQ;
						if ($EP[1].call($EP[0], $KEY_23) >= 0) {
							$EQ = true;
						} else {
							$EO = [$EO, $EO.indexOf];
							$EQ = $EO[1].call($EO[0], "any") >= 0;
						}
						$EQ = $EQ && $EM.protocol.test($KEY_12) && $EM.host.test($KEY_5) && $EM.path.test($KEY_9);
						if ($EQ) {
							return $EM.port.test($KEY_22);
						} else {
							return $EQ;
						}
					});
				} else {
					return false;
				}
			} catch (e_2) {
				return false;
			}
		}

return $THREAD_1_33315;
}
bootstrap();
