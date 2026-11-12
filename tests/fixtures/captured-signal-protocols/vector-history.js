function $THREAD_6_105626($CV, $CW) {
					let $KEY_0 = $CV;
					if ($CW.a.some($CY => {
						let $CZ;
						if ($KEY_0 == null || $CY == null) {
							$CZ = true;
						} else {
							$CZ = $KEY_0.length !== $CY.length;
						}
						if ($CZ) {
							return false;
						} else {
							let $DA = 0;
							let $DB;
							for (;;) {
								$DB = void 0;
								if ($DA >= $KEY_0.length) {
									$DB = true;
									break;
								}
								if ($KEY_0[$DA] === $CY[$DA]) {
									++$DA;
									continue;
								}
								$DB = false;
								break;
							}
							return $DB;
						}
					})) {
						return void 0;
					}
					$CW.b = Number($CW.b) + 1;
					if ($CW.a.length > 5) {
						return void 0;
					} else {
						let $CX = Array.prototype.slice.call($KEY_0);
						$CW.a.push($CX);
						return void 0;
					}
				}