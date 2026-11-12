function main(){
function $THREAD_9_67964($BW, $BX) {
					let $KEY_1 = $BX;
					if ($BW == null || $BW.trim() === "" || $KEY_1 == null) {
						return [];
					}
					let $KEY_0 = $BW.trim();
					let $BY;
					if ($KEY_0 == null || $KEY_0.trim() === "") {
						$BY = [];
					} else {
						let $KEY_3 = document.createElement("a");
						$KEY_3.href = $KEY_0;
						let $BZ = $KEY_3.search.slice(1);
						if ($BZ === "") {
							$BY = [];
						} else {
							$BZ = [$BZ, $BZ.split];
							$BY = $BZ[1].call($BZ[0], "&").map($CA => {
								let $KEY_4 = $CA.split("=");
								let $KEY_0 = typeof $KEY_4[0] === "undefined" ? "undefined" : $KEY_4[0] === null ? "null" : decodeURIComponent($KEY_4[0].toString()).replace(new RegExp(RegExp("\\+", "g")), " ");
								let $CB;
								if (typeof $KEY_4[1] === "undefined") {
									$CB = "";
								} else {
									$CB = typeof $KEY_4[1] === "undefined" ? "undefined" : $KEY_4[1] === null ? "null" : decodeURIComponent($KEY_4[1].toString()).replace(new RegExp(RegExp("\\+", "g")), " ");
								}
								$KEY_0 = [$KEY_0, $CB];
								return $KEY_0;
							});
						}
					}
					if ($BY.length) {
						return $BY.filter($CA => $CA[0] === $KEY_1).map($CA => $CA[1]);
					} else {
						return [];
					}
				}
return $THREAD_9_67964;
}main();
