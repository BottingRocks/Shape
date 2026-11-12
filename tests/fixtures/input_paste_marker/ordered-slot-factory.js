function $THREAD_4_34102($BP, $BQ, $BR, $BS, $BT, $BU) {
					let $KEY_2 = $BQ;
					let $KEY_3, $KEY_4, $KEY_7, $KEY_10, $KEY_11, $KEY_13, $KEY_14, $KEY_16, $KEY_17;
					let $KEY_0 = $BP[0];
					let $KEY_1 = $BP[1];
					$KEY_10 = $BP[2];
					$KEY_7 = $BP[3];
					$KEY_16 = $BP[4];
					$KEY_3 = $KEY_2.global.document;
					$KEY_11 = $KEY_2.global.document;
					$KEY_14 = null;
					$KEY_4 = false;
					$KEY_13 = $KEY_2.global;
					$KEY_17 = 0;
					$BU(() => {
						if ($KEY_14 != null) {
							$KEY_3.removeEventListener("input", $KEY_14, true);
						}
					});
					$BT(() => {
						let $KEY_5 = Math.random() * 4294967296 | 0;
						let $BV = [$KEY_5 >>> 24, $KEY_5 >> 16 & 255, $KEY_5 >> 8 & 255, $KEY_5 & 255];
						let $KEY_1 = $KEY_183($KEY_5 ^ 4185131742);
						$BV[$KEY_108($KEY_1, $KEY_17, $BV, 3) + 1] = ($KEY_4 ? 39 : 59) ^ $KEY_1();
						$KEY_16.next([[], $BV]);
					});
					$BS(() => {
						let $BV = [];
						try {
							let $BW = [];
							let $BX = $KEY_160;
							let $BY = $KEY_11.createElement("video");
							let i = 0;
							for (; i < $BX.length; i++) {
								try {
									$BW.push($BY.canPlayType($BX[i]));
								} catch (e) {
									$BW.push(null);
								}
							}
							let $BZ = 0;
							while ($BZ < $KEY_160.length) {
								$BV[$BZ] = $BW[$BZ] === "" ? 0 : $BW[$BZ] === "maybe" ? 1 : $BW[$BZ] === "probably" ? 2 : $BW[$BZ] === null ? 3 : 4;
								++$BZ;
							}
							let $CA = Math.random() * 4294967296 | 0;
							let $CB = [$CA >>> 24, $CA >> 16 & 255, $CA >> 8 & 255, $CA & 255];
							$CA = $KEY_183($CA ^ 4014925216);
							let $CC = $BV.length;
							let j = 0;
							let $CD = $KEY_108($CA, $CC, $CB, 3);
							for (; j < $CC; j++) {
								$CD = $KEY_108($CA, $BV[j], $CB, $CD);
							}
							$KEY_1.next([[], $CB]);
							$KEY_1.complete();
							return void 0;
						} catch (e_2) {
							$KEY_1.error(e_2);
							let $CE = Math.random() * 4294967296 | 0;
							let $CF = [$CE >>> 24, $CE >> 16 & 255, $CE >> 8 & 255, $CE & 255];
							$CE = $KEY_183($CE ^ 4014925216);
							let $CG = $BV.length;
							let k = 0;
							let $CH = $KEY_108($CE, $CG, $CF, 3);
							for (; k < $CG; k++) {
								$CH = $KEY_108($CE, $BV[k], $CF, $CH);
							}
							$KEY_1.next([[], $CF]);
							$KEY_1.complete();
							return void 0;
						}
					}, false);
					$BR(() => {
						let $BV = Math.random() * 4294967296;
						let $BW = [];
						let $BX = [($BV | 0) >>> 24, ($BV | 0) >> 16 & 255, ($BV | 0) >> 8 & 255, ($BV | 0) & 255];
						let $KEY_31 = $KEY_183(($BV | 0) ^ 400779145);
						let $BY = $KEY_314($KEY_31);
						let $BZ = Math.random() * 4294967296;
						let $CA = ["height", "width", "availHeight", "availWidth", "pixelDepth", "colorDepth"];
						let $CB = {};
						let $CC;
						if ($KEY_13.screen == null || typeof $KEY_13.screen !== "object") {
							$CC = void 0;
						} else {
							$CC = void 0;
							let i = 0;
							for (; i < $CA.length; i++) {
								if ($CA[i] in $KEY_13.screen) {
									try {
										$CC = $CB;
										$CB[$CA[i]] = $KEY_13.screen[$CA[i]];
									} catch (e) {}
								}
							}
						}
						let $CD = $KEY_9({
							height: null,
							width: null,
							availHeight: null,
							availWidth: null,
							pixelDepth: null,
							colorDepth: null
						}, $CB);
						let $KEY_25 = Object.keys($KEY_2.global.location).length;
						let $CE;
						if ($CD.colorDepth == null) {
							$BX[4] = $KEY_31() ^ 130;
							$CE = 4;
						} else {
							$CE = $KEY_108($KEY_31, $CD.colorDepth, $BX, 3);
						}
						let $CF = $KEY_9;
						let $CG = ["appCodeName", "appName", "appVersion", "buildID", "cpuClass", "hardwareConcurrency", "maxTouchPoints", "platform", "product", "productSub", "oscpu", "userAgent", "vendor", "vendorSub", "doNotTrack"];
						let $CH = {};
						let $CI;
						if ($KEY_13.navigator == null || typeof $KEY_13.navigator !== "object") {
							$CI = void 0;
						} else {
							let k = 0;
							$CI = void 0;
							for (; k < $CG.length; k++) {
								$CI = $CG[k];
								if ($CI in $KEY_13.navigator) {
									try {
										$CH[$CI] = $KEY_13.navigator[$CI];
									} catch (e_2) {}
								}
							}
						}
						let $CJ;
						if ($KEY_13.navigator) {
							$CJ = { webdriver: $KEY_13.navigator.webdriver === true };
						} else {
							$CJ = Object.create(null);
						}
						$CF = $CF({
							appCodeName: null,
							appName: null,
							appVersion: null,
							buildID: null,
							cpuClass: null,
							hardwareConcurrency: null,
							maxTouchPoints: null,
							platform: null,
							product: null,
							productSub: null,
							oscpu: null,
							userAgent: null,
							vendor: null,
							vendorSub: null
						}, $CH, $CJ);
						let $CK = [($BZ | 0) >>> 24, ($BZ | 0) >> 16 & 255, ($BZ | 0) >> 8 & 255, ($BZ | 0) & 255];
						let $CL = [];
						$BZ = $KEY_183(($BZ | 0) ^ 2935147062);
						let $CM;
						if ($CF.product == null) {
							$BX[++$CE] = $KEY_31() ^ 131;
							$CM = $BX;
						} else {
							$CM = $CH;
							$CE = $BY($CF.product, $BX, $CE);
						}
						let $CN = ["innerHeight", "innerWidth", "outerHeight", "outerWidth", "screenX", "screenY", "devicePixelRatio", "isSecureContext"];
						$CM = {};
						if ($KEY_13 != null && typeof $KEY_13 === "object") {
							let j = 0;
							for (; j < $CN.length; j++) {
								if ($CN[j] in $KEY_13) {
									try {
										$CM[$CN[j]] = $KEY_13[$CN[j]];
									} catch (e_3) {}
								}
							}
						}
						$CM = $KEY_9({
							innerHeight: null,
							innerWidth: null,
							outerHeight: null,
							outerWidth: null,
							screenX: null,
							screenY: null,
							isSecureContext: null
						}, $CM);
						if ($CM.screenY == null) {
							$BX[++$CE] = $KEY_31() ^ 132;
						} else {
							$CE = $KEY_108($KEY_31, $CM.screenY, $BX, $CE);
						}
						if ($CF.appCodeName == null) {
							$BX[++$CE] = $KEY_31() ^ 133;
						} else {
							$CE = $BY($CF.appCodeName, $BX, $CE);
						}
						if ($CF.vendor == null) {
							$BX[++$CE] = $KEY_31() ^ 134;
						} else {
							$CE = $BY($CF.vendor, $BX, $CE);
						}
						let $CO = $CF.userAgent;
						let $CP;
						if ($KEY_2.global.location !== null && $KEY_2.global.location !== void 0) {
							let $CQ = Object($KEY_2.global.location);
							let $CR = 0;
							$CP = "";
							for (const $CS in $CQ) {
								let $CT = $KEY_111[$CS];
								++$CR;
								if ($CR > ($KEY_25 < 30 ? $KEY_25 : 30)) {
									if ($CO == null) {
										$BX[++$CE] = $KEY_31() ^ 135;
									} else {
										$CE = $BY($CO, $BX, $CE);
									}
									if ($CF.platform == null) {
										$BX[++$CE] = $KEY_31() ^ 136;
									} else {
										$CE = $BY($CF.platform, $BX, $CE);
									}
									const $CU = $KEY_13.toolbar && typeof $KEY_13.toolbar.visible === "boolean" ? $KEY_13.toolbar.visible : null;
									let $CV;
									if ($CU == null) {
										$BX[++$CE] = $KEY_31() ^ 137;
										$CV = 0;
									} else {
										$CV = $CU | 0;
									}
									function $THREAD_5_86532($DG) {
										try {
											if ($DG.target.tagName === "INPUT") {
												if ($DG.inputType === "insertFromPaste") {
													++$KEY_17;
												} else if ($DG.target !== $KEY_3.activeElement) {
													$KEY_4 = true;
												}
											}
										} catch (e) {
											$KEY_16.error(e);
										}
									}
									$KEY_14 = $THREAD_5_86532;
									if ($CD.width == null) {
										$BX[++$CE] = $KEY_31() ^ 138;
									} else {
										$CE = $KEY_108($KEY_31, $CD.width, $BX, $CE);
									}
									if ($CF.appName == null) {
										$BX[++$CE] = $KEY_31() ^ 139;
									} else {
										$CE = $BY($CF.appName, $BX, $CE);
									}
									const $CW = $KEY_13.opera && typeof $KEY_13.opera.version === "function" ? $KEY_13.opera.version() : null;
									if ($CW == null) {
										$BX[++$CE] = $KEY_31() ^ 140;
									} else {
										$CE = $BY($CW, $BX, $CE);
									}
									$BZ = $KEY_314($BZ);
									let $CX = Math.random() * 4294967296;
									if ($CM.screenX == null) {
										$BX[++$CE] = $KEY_31() ^ 141;
									} else {
										$CE = $KEY_108($KEY_31, $CM.screenX, $BX, $CE);
									}
									if ($KEY_218($CF, "doNotTrack")) {
										if ($CF.doNotTrack == null) {
											$BX[++$CE] = $KEY_31() ^ 143;
										} else {
											$CE = $BY($CF.doNotTrack, $BX, $CE);
										}
									} else {
										$BX[++$CE] = $KEY_31() ^ 142;
									}
									if ($CD.availHeight == null) {
										$BX[++$CE] = $KEY_31() ^ 144;
									} else {
										$CE = $KEY_108($KEY_31, $CD.availHeight, $BX, $CE);
									}
									if ($CM.innerWidth == null) {
										$BX[++$CE] = $KEY_31() ^ 145;
									} else {
										$CE = $KEY_108($KEY_31, $CM.innerWidth, $BX, $CE);
									}
									const $CY = $KEY_13.locationbar && typeof $KEY_13.locationbar.visible === "boolean" ? $KEY_13.locationbar.visible : null;
									if ($CY == null) {
										$BX[++$CE] = $KEY_31() ^ 146;
									} else {
										$CV = $CV | $CY << 1;
									}
									if ($CD.availWidth == null) {
										$BX[++$CE] = $KEY_31() ^ 147;
									} else {
										$CE = $KEY_108($KEY_31, $CD.availWidth, $BX, $CE);
									}
									if ($CF.vendorSub == null) {
										$BX[++$CE] = $KEY_31() ^ 148;
									} else {
										$CE = $BY($CF.vendorSub, $BX, $CE);
									}
									if ($CM.outerWidth == null) {
										$BX[++$CE] = $KEY_31() ^ 149;
									} else {
										$CE = $KEY_108($KEY_31, $CM.outerWidth, $BX, $CE);
									}
									let $CZ = [($CX | 0) >>> 24, ($CX | 0) >> 16 & 255, ($CX | 0) >> 8 & 255, ($CX | 0) & 255];
									if ($CF.cpuClass == null) {
										$BX[++$CE] = $KEY_31() ^ 150;
									} else {
										$CE = $BY($CF.cpuClass, $BX, $CE);
									}
									if ($CF.maxTouchPoints == null) {
										$BX[++$CE] = $KEY_31() ^ 151;
									} else {
										$CE = $KEY_108($KEY_31, $CF.maxTouchPoints, $BX, $CE);
									}
									$KEY_3.addEventListener("input", $KEY_14, true);
									$BZ(new Date($KEY_137).toString(), $CK, 3);
									if ($CF.appVersion == null) {
										$BX[++$CE] = $KEY_31() ^ 152;
									} else {
										$CE = $BY($CF.appVersion, $BX, $CE);
									}
									if ($CM.outerHeight == null) {
										$BX[++$CE] = $KEY_31() ^ 153;
									} else {
										$CE = $KEY_108($KEY_31, $CM.outerHeight, $BX, $CE);
									}
									if ($CF.oscpu == null) {
										$BX[++$CE] = $KEY_31() ^ 154;
									} else {
										$CE = $BY($CF.oscpu, $BX, $CE);
									}
									if ($CM.isSecureContext == null) {
										$BX[++$CE] = $KEY_31() ^ 155;
									} else {
										$CV = $CV | $CM.isSecureContext << 2;
									}
									$CX = $KEY_183(($CX | 0) ^ 611719744);
									if ($KEY_218($CM, "devicePixelRatio")) {
										if ($CM.devicePixelRatio == null) {
											$BX[++$CE] = $KEY_31() ^ 157;
										} else {
											$CE = $KEY_108($KEY_31, $CM.devicePixelRatio, $BX, $CE);
										}
									} else {
										$BX[++$CE] = $KEY_31() ^ 156;
									}
									if ($CD.height == null) {
										$BX[++$CE] = $KEY_31() ^ 158;
									} else {
										$CE = $KEY_108($KEY_31, $CD.height, $BX, $CE);
									}
									if ($CF.buildID == null) {
										$BX[++$CE] = $KEY_31() ^ 159;
									} else {
										$CE = $BY($CF.buildID, $BX, $CE);
									}
									if ($CM.innerHeight == null) {
										$BX[++$CE] = $KEY_31() ^ 160;
										$BX[++$CE] = $KEY_31() ^ 1;
									} else {
										$CE = $KEY_108($KEY_31, $CM.innerHeight, $BX, $CE);
									}
									$CX = $KEY_314($CX);
									$KEY_7.next([$CL, $CK]);
									$KEY_7.complete();
									if ($CD.pixelDepth == null) {
										$BX[++$CE] = $KEY_31() ^ 161;
										$BX[++$CE] = $KEY_31() ^ 1;
									} else {
										$CE = $KEY_108($KEY_31, $CD.pixelDepth, $BX, $CE);
									}
									$CX($CP + (($CT || "u") + "\0"), $CZ, 3);
									if ($CF.hardwareConcurrency == null) {
										$BX[++$CE] = $KEY_31() ^ 162;
										$BX[++$CE] = $KEY_31() ^ 1;
									} else {
										$CE = $KEY_108($KEY_31, $CF.hardwareConcurrency, $BX, $CE);
									}
									if ($KEY_218($CF, "webdriver")) {
										if ($CF.webdriver == null) {
											$BX[++$CE] = $KEY_31() ^ 164;
											$BX[++$CE] = $KEY_31() ^ 1;
										} else {
											$CV = $CV | $CF.webdriver << 3;
										}
									} else {
										$BX[++$CE] = $KEY_31() ^ 163;
										$BX[++$CE] = $KEY_31() ^ 1;
									}
									$BX[++$CE] = $CV ^ $KEY_31();
									$KEY_10.next([[], $CZ]);
									$KEY_10.complete();
									if ($CF.productSub == null) {
										$BX[++$CE] = $KEY_31() ^ 165;
										$BX[$CE + 1] = $KEY_31() ^ 1;
									} else {
										$BY($CF.productSub, $BX, $CE);
									}
									$KEY_0.next([$BW, $BX]);
									$KEY_0.complete();
									return void 0;
								}
								$CP = $CP + (($CT || "u") + "\0");
							}
						} else {
							$CP = "";
						}
						if ($CO == null) {
							$CO = $CE + 1;
							$BX[$CO] = $KEY_31() ^ 135;
						} else {
							$CO = $BY($CO, $BX, $CE);
						}
						if ($CF.platform == null) {
							$BX[++$CO] = $KEY_31() ^ 136;
						} else {
							$CO = $BY($CF.platform, $BX, $CO);
						}
						const $DA = $KEY_13.toolbar && typeof $KEY_13.toolbar.visible === "boolean" ? $KEY_13.toolbar.visible : null;
						let $DB;
						if ($DA == null) {
							$BX[++$CO] = $KEY_31() ^ 137;
							$DB = 0;
						} else {
							$DB = $DA | 0;
						}
						function $THREAD_5_86532_2($DG) {
							try {
								if ($DG.target.tagName === "INPUT") {
									if ($DG.inputType === "insertFromPaste") {
										++$KEY_17;
									} else if ($DG.target !== $KEY_3.activeElement) {
										$KEY_4 = true;
									}
								}
							} catch (e) {
								$KEY_16.error(e);
							}
						}
						$KEY_14 = $THREAD_5_86532_2;
						if ($CD.width == null) {
							$BX[++$CO] = $KEY_31() ^ 138;
						} else {
							$CO = $KEY_108($KEY_31, $CD.width, $BX, $CO);
						}
						if ($CF.appName == null) {
							$BX[++$CO] = $KEY_31() ^ 139;
						} else {
							$CO = $BY($CF.appName, $BX, $CO);
						}
						const $DC = $KEY_13.opera && typeof $KEY_13.opera.version === "function" ? $KEY_13.opera.version() : null;
						if ($DC == null) {
							$BX[++$CO] = $KEY_31() ^ 140;
						} else {
							$CO = $BY($DC, $BX, $CO);
						}
						$BZ = $KEY_314($BZ);
						let $DD = Math.random() * 4294967296;
						if ($CM.screenX == null) {
							$BX[++$CO] = $KEY_31() ^ 141;
						} else {
							$CO = $KEY_108($KEY_31, $CM.screenX, $BX, $CO);
						}
						if ($KEY_218($CF, "doNotTrack")) {
							if ($CF.doNotTrack == null) {
								$BX[++$CO] = $KEY_31() ^ 143;
							} else {
								$CO = $BY($CF.doNotTrack, $BX, $CO);
							}
						} else {
							$BX[++$CO] = $KEY_31() ^ 142;
						}
						if ($CD.availHeight == null) {
							$BX[++$CO] = $KEY_31() ^ 144;
						} else {
							$CO = $KEY_108($KEY_31, $CD.availHeight, $BX, $CO);
						}
						if ($CM.innerWidth == null) {
							$BX[++$CO] = $KEY_31() ^ 145;
						} else {
							$CO = $KEY_108($KEY_31, $CM.innerWidth, $BX, $CO);
						}
						const $DE = $KEY_13.locationbar && typeof $KEY_13.locationbar.visible === "boolean" ? $KEY_13.locationbar.visible : null;
						if ($DE == null) {
							$BX[++$CO] = $KEY_31() ^ 146;
						} else {
							$DB = $DB | $DE << 1;
						}
						if ($CD.availWidth == null) {
							$BX[++$CO] = $KEY_31() ^ 147;
						} else {
							$CO = $KEY_108($KEY_31, $CD.availWidth, $BX, $CO);
						}
						if ($CF.vendorSub == null) {
							$BX[++$CO] = $KEY_31() ^ 148;
						} else {
							$CO = $BY($CF.vendorSub, $BX, $CO);
						}
						if ($CM.outerWidth == null) {
							$BX[++$CO] = $KEY_31() ^ 149;
						} else {
							$CO = $KEY_108($KEY_31, $CM.outerWidth, $BX, $CO);
						}
						let $DF = [($DD | 0) >>> 24, ($DD | 0) >> 16 & 255, ($DD | 0) >> 8 & 255, ($DD | 0) & 255];
						if ($CF.cpuClass == null) {
							$BX[++$CO] = $KEY_31() ^ 150;
						} else {
							$CO = $BY($CF.cpuClass, $BX, $CO);
						}
						if ($CF.maxTouchPoints == null) {
							$BX[++$CO] = $KEY_31() ^ 151;
						} else {
							$CO = $KEY_108($KEY_31, $CF.maxTouchPoints, $BX, $CO);
						}
						$KEY_3.addEventListener("input", $KEY_14, true);
						$BZ(new Date($KEY_137).toString(), $CK, 3);
						if ($CF.appVersion == null) {
							$BX[++$CO] = $KEY_31() ^ 152;
						} else {
							$CO = $BY($CF.appVersion, $BX, $CO);
						}
						if ($CM.outerHeight == null) {
							$BX[++$CO] = $KEY_31() ^ 153;
						} else {
							$CO = $KEY_108($KEY_31, $CM.outerHeight, $BX, $CO);
						}
						if ($CF.oscpu == null) {
							$BX[++$CO] = $KEY_31() ^ 154;
						} else {
							$CO = $BY($CF.oscpu, $BX, $CO);
						}
						if ($CM.isSecureContext == null) {
							$BX[++$CO] = $KEY_31() ^ 155;
						} else {
							$DB = $DB | $CM.isSecureContext << 2;
						}
						$DD = $KEY_183(($DD | 0) ^ 611719744);
						if ($KEY_218($CM, "devicePixelRatio")) {
							if ($CM.devicePixelRatio == null) {
								$BX[++$CO] = $KEY_31() ^ 157;
							} else {
								$CO = $KEY_108($KEY_31, $CM.devicePixelRatio, $BX, $CO);
							}
						} else {
							$BX[++$CO] = $KEY_31() ^ 156;
						}
						if ($CD.height == null) {
							$BX[++$CO] = $KEY_31() ^ 158;
						} else {
							$CO = $KEY_108($KEY_31, $CD.height, $BX, $CO);
						}
						if ($CF.buildID == null) {
							$BX[++$CO] = $KEY_31() ^ 159;
						} else {
							$CO = $BY($CF.buildID, $BX, $CO);
						}
						if ($CM.innerHeight == null) {
							$BX[++$CO] = $KEY_31() ^ 160;
							$BX[++$CO] = $KEY_31() ^ 1;
						} else {
							$CO = $KEY_108($KEY_31, $CM.innerHeight, $BX, $CO);
						}
						$DD = $KEY_314($DD);
						$KEY_7.next([$CL, $CK]);
						$KEY_7.complete();
						if ($CD.pixelDepth == null) {
							$BX[++$CO] = $KEY_31() ^ 161;
							$BX[++$CO] = $KEY_31() ^ 1;
						} else {
							$CO = $KEY_108($KEY_31, $CD.pixelDepth, $BX, $CO);
						}
						$DD($CP, $DF, 3);
						if ($CF.hardwareConcurrency == null) {
							$BX[++$CO] = $KEY_31() ^ 162;
							$BX[++$CO] = $KEY_31() ^ 1;
						} else {
							$CO = $KEY_108($KEY_31, $CF.hardwareConcurrency, $BX, $CO);
						}
						if ($KEY_218($CF, "webdriver")) {
							if ($CF.webdriver == null) {
								$BX[++$CO] = $KEY_31() ^ 164;
								$BX[++$CO] = $KEY_31() ^ 1;
							} else {
								$DB = $DB | $CF.webdriver << 3;
							}
						} else {
							$BX[++$CO] = $KEY_31() ^ 163;
							$BX[++$CO] = $KEY_31() ^ 1;
						}
						$BX[++$CO] = $DB ^ $KEY_31();
						$KEY_10.next([[], $DF]);
						$KEY_10.complete();
						if ($CF.productSub == null) {
							$BX[++$CO] = $KEY_31() ^ 165;
							$BX[$CO + 1] = $KEY_31() ^ 1;
						} else {
							$BY($CF.productSub, $BX, $CO);
						}
						$KEY_0.next([$BW, $BX]);
						$KEY_0.complete();
					});
				}