function $THREAD_8_61920() {
						let $CH = $KEY_15.persistentId.key;
						let $CI = $KEY_15.transactionToken;
						try {
							$KEY_137 = false;
							if ($KEY_67) {
								if (!$KEY_67.getItem($CH)) {
									$KEY_67.setItem($CH, $CI);
								}
								$KEY_137 = true;
							}
						} catch (e) {}
						let $CJ, $CK;
						try {
							if ($KEY_67) {
								$KEY_365 = "l_" + $KEY_15.persistentId.key;
								let $CL = Date.now();
								$CK = void 0;
								$CJ = void 0;
								$CJ = Math.random() * 4294967296 >>> 0;
								$CK = 0;
								let $CM = 0;
								try {
									$CM = performance.now() >>> 0;
								} catch (e_2) {}
								let $CN = Number(new Date()) >>> 0;
								let $CO = 0;
								try {
									$CO = crypto.getRandomValues(new Uint32Array(1))[0];
								} catch (e_3) {}
								$CJ = $CJ ^ 1103515245;
								$CM = $CM === $CM ? $CJ ^ $CM : $CJ;
								$CN = $CN === $CN ? $CM ^ $CN : $CM;
								++$KEY_142;
								$CK = ($CN ^ $CO ^ $KEY_142) >>> 0;
								$KEY_61 = "" + $CL + $CK;
								if (!$KEY_67.getItem($KEY_365)) {
									$KEY_67.setItem($KEY_365, $KEY_61);
								}
							}
							$CJ = 62004;
							$CK = 2;
						} catch (e_4) {}
						$KEY_378 = $KEY_53($KEY_15.transactionToken);
						$KEY_115();
						try {
							$KEY_10.forEach($CQ => {
								$CQ.init($KEY_307());
								if ($CQ.afterReady != null) {
									if (typeof setTimeout !== "function") {
										$CQ.afterReady.call(void 0);
									} else if ($KEY_424) {
										setTimeout($CQ.afterReady, 0);
									} else if ($KEY_436) {
										$KEY_436.push($CQ.afterReady);
									}
								}
							});
						} catch (e_5) {
							let $CP = {};
							$CP.message = "3.1:" + e_5;
							$KEY_311($CP);
						}
					}