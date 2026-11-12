//                    Font sampler factory with holder and push aliases.
// Inert carrier; no captured JavaScript is executed.
function bootstrap(){
let $KEY_0, $KEY_3, $KEY_6, $KEY_7, $KEY_53, $KEY_5, $KEY_232, $KEY_90, $KEY_10, $KEY_2, $KEY_389;
function $THREAD_2_40476($BM) {
			let $KEY_0 = $BM;
			function $THREAD_2_31406($BN, $BO, $BP, $BQ, $BR, $BS) {
				let $KEY_3 = $BN;
				let $KEY_6;
				let $KEY_7 = $BO.global.document;
				$KEY_6 = null;
				$BQ(() => {
					$KEY_6 = $KEY_53($KEY_0, $KEY_7, $BT => {
						let $BU = Math.random() * 4294967296;
						let $BV = [($BU | 0) >>> 24, ($BU | 0) >> 16 & 255, ($BU | 0) >> 8 & 255, ($BU | 0) & 255];
						let $KEY_5 = $KEY_232(($BU | 0) ^ 1142140036);
						let i = 0;
						let $BW = 0;
						let $BX = 0;
						let $BY = $KEY_90($KEY_5, $BT.length, $BV, 3);
						for (; i < $BT.length; i++) {
							let $BZ = $BT[i] << $BW;
							++$BW;
							if ($BW === 8) {
								$BV[++$BY] = ($BX | $BZ) ^ $KEY_5();
								$BW = 0;
								$BX = 0;
							} else {
								$BX = $BX | $BZ;
							}
						}
						if ($BW !== 0) {
							$BV[++$BY] = $BX ^ $KEY_5();
						}
						$KEY_90($KEY_5, $KEY_0.version, $BV, $BY);
						$KEY_3.next([[], $BV]);
						$KEY_3.complete();
					});
				}, true);
				$BS(() => {
					if (typeof $KEY_6 === "function") {
						$KEY_6();
					}
				});
			}
			return $THREAD_2_31406;
		}
function $THREAD_9_43168($BM, $BN, $BO) {
			let $KEY_7 = $BO;
			let $KEY_10;
			let $KEY_3 = [];
			let $BP = $BN.createElement("canvas");
			let $KEY_5 = $BP && $BP.getContext && $BP.getContext("2d");
			let $KEY_2;
			if ($KEY_5 && typeof $KEY_5.measureText === "function") {
				$KEY_5.textBaseline = "top";
				$KEY_5.font = "50px  sans-serif";
				$KEY_10 = $KEY_5.measureText("0-_{w.").width;
				$KEY_5.font = "50px  serif";
				let $KEY_0 = $KEY_5.measureText("0-_{w.").width;
				$KEY_2 = $KEY_389(($BQ, $BR) => {
					let $BS = $KEY_3;
					let $BT = $KEY_3.push;
					$KEY_5.font = "50px " + ($BQ == null ? "" : "'" + $BQ + "', ") + " sans-serif";
					let $BU;
					if ($KEY_5.measureText("0-_{w.").width !== $KEY_10) {
						$BU = true;
					} else {
						$KEY_5.font = "50px " + ($BQ == null ? "" : "'" + $BQ + "', ") + " serif";
						$BU = $KEY_5.measureText("0-_{w.").width !== $KEY_0;
					}
					$BT.call($BS, $BU);
					$BR();
				}, $BM.fonts);
				$KEY_2.onEmpty(() => {
					$KEY_7($KEY_3);
				});
				$KEY_2.start();
			}
			function $THREAD_9_79413() {
				if ($KEY_2) {
					$KEY_2.empty();
				}
			}
			return $THREAD_9_79413;
		}
$KEY_53 = $THREAD_9_43168;
return $THREAD_2_40476;
}
bootstrap();
