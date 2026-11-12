//                 Font sampler factory with alternate context calls.
// Inert carrier; no captured JavaScript is executed.
function bootstrap(){
let $KEY_0, $KEY_6, $KEY_5, $KEY_4, $KEY_240, $KEY_304, $KEY_214, $KEY_7, $KEY_1, $KEY_3, $KEY_395;
function $THREAD_9_22617($BM) {
			let $KEY_0 = $BM;
			function $THREAD_7_139974($BN, $BO, $BP, $BQ, $BR, $BS) {
				let $KEY_6 = $BN;
				let $KEY_5 = $BO.global.document;
				let $KEY_4 = null;
				$BQ(() => {
					$KEY_4 = $KEY_240($KEY_0, $KEY_5, $BT => {
						let $BU = Math.random() * 4294967296;
						let $BV = [];
						let $BW = [($BU | 0) >>> 24, ($BU | 0) >> 16 & 255, ($BU | 0) >> 8 & 255, ($BU | 0) & 255];
						let $KEY_5 = $KEY_304(($BU | 0) ^ 4065333542);
						let $BX = $BT.length;
						let $BY = $KEY_214($KEY_5, $BX, $BW, $KEY_214($KEY_5, $KEY_0.version, $BW, 3));
						let $BZ = 0;
						let $CA = 0;
						while ($BX - 1 >= 0) {
							$CA = $CA | $BT[$BX - 1] << $BZ;
							++$BZ;
							if ($BZ === 8) {
								$BW[++$BY] = $CA ^ $KEY_5();
								$BZ = 0;
								$CA = 0;
							}
							--$BX;
						}
						if ($BZ !== 0) {
							$BW[$BY + 1] = $CA ^ $KEY_5();
						}
						$KEY_6.next([$BV, $BW]);
						$KEY_6.complete();
					});
				}, true);
				$BS(() => {
					if (typeof $KEY_4 === "function") {
						$KEY_4();
					}
				});
			}
			return $THREAD_7_139974;
		}
function $THREAD_7_111058($BM, $BN, $BO) {
			let $KEY_7 = $BO;
			let $KEY_0 = [];
			let $BP = $BN.createElement("canvas");
			let $KEY_5 = $BP && $BP.getContext && $BP.getContext("2d");
			let $KEY_6;
			if ($KEY_5 && typeof $KEY_5.measureText === "function") {
				$KEY_5.textBaseline = "top";
				$KEY_5.font = "50px  sans-serif";
				let $KEY_1 = $KEY_5.measureText("0-_{w.").width;
				$KEY_5.font = "50px  serif";
				let $KEY_3 = $KEY_5.measureText("0-_{w.").width;
				$KEY_6 = $KEY_395(($BQ, $BR) => {
					let $BS = $KEY_0;
					let $BT = $KEY_0.push;
					$KEY_5.font = "50px " + ($BQ == null ? "" : "'" + $BQ + "', ") + " sans-serif";
					let $BU;
					if ($KEY_5.measureText("0-_{w.").width !== $KEY_1) {
						$BU = true;
					} else {
						$KEY_5.font = "50px " + ($BQ == null ? "" : "'" + $BQ + "', ") + " serif";
						$BU = $KEY_5.measureText("0-_{w.").width !== $KEY_3;
					}
					$BT.call($BS, $BU);
					$BR();
				}, $BM.fonts);
				$KEY_6.onEmpty(() => {
					$KEY_7($KEY_0);
				});
				$KEY_6.start();
			}
			function $THREAD_7_68425() {
				if ($KEY_6) {
					$KEY_6.empty();
				}
			}
			return $THREAD_7_68425;
		}
$KEY_240 = $THREAD_7_111058;
return $THREAD_9_22617;
}
bootstrap();
