function $THREAD_1_95315($CG) {
						let $KEY_3 = $CG;
						let $KEY_2 = typeof $KEY_3 === "function";
						let $CH;
						if ($KEY_2) {
							$KEY_3 = $CG;
							$CH = $KEY_272.test($KEY_64.call($KEY_3));
						} else {
							$CH = false;
						}
						let $CI;
						if ($KEY_2) {
							$KEY_3 = $CG;
							$CI = $KEY_221($KEY_3, "prototype");
						} else {
							$CI = false;
						}
						if ($KEY_2) {
							$KEY_3 = $KEY_64.call($CG);
						} else {
							$KEY_3 = "";
						}
						let $CJ;
						if ($KEY_2 && !$CI) {
							$CJ = false;
							try {
								delete $CG.prototype;
								$CJ = true;
							} catch (e) {}
						} else {
							$CJ = false;
						}
						$KEY_2 = {
							a: $KEY_2,
							b: $CH,
							c: !$CI,
							d: $CJ,
							e: $KEY_3
						};
						return $KEY_2;
					}