function main(){
function $THREAD_1_140150($BQ, $BR) {
					let $KEY_0 = $BR;
					let $BS;
					if ($BQ == null) {
						$BS = true;
					} else {
						$BS = $BQ.trim() === "";
					}
					if ($BS || $KEY_0 == null) {
						return [];
					}
					let $BT = $BQ.trim();
					let $BU;
					if ($BT == null || $BT.trim() === "") {
						$BU = [];
					} else {
						let $BV = document.createElement("a");
						$BV.href = $BT;
						let $BW = $BV.search.slice(1);
						if ($BW === "") {
							$BU = [];
						} else {
							$BW = [$BW, $BW.split];
							$BU = $BW[1].call($BW[0], "&").map($BX => {
								let $KEY_0 = $BX.split("=");
								let $KEY_3 = typeof $KEY_0[0] === "undefined" ? "undefined" : $KEY_0[0] === null ? "null" : decodeURIComponent($KEY_0[0].toString()).replace(new RegExp(RegExp("\\+", "g")), " ");
								let $BY;
								if (typeof $KEY_0[1] === "undefined") {
									$BY = "";
								} else {
									$BY = typeof $KEY_0[1] === "undefined" ? "undefined" : $KEY_0[1] === null ? "null" : decodeURIComponent($KEY_0[1].toString()).replace(new RegExp(RegExp("\\+", "g")), " ");
								}
								$KEY_3 = [$KEY_3, $BY];
								return $KEY_3;
							});
						}
					}
					if ($BU.length) {
						return $BU.filter($BX => $BX[0] === $KEY_0).map($BX => $BX[1]);
					} else {
						return [];
					}
				}
return $THREAD_1_140150;
}main();
