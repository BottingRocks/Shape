//                    Font sampler factory with direct context calls.
// Inert carrier; no captured JavaScript is executed.
function bootstrap(){
let $KEY_0, $KEY_1, $KEY_5, $KEY_7, $KEY_336, $KEY_11, $KEY_344, $KEY_390, $KEY_4, $KEY_2, $KEY_70;
function $THREAD_1_119865($BU) {
			let $KEY_0 = $BU;
			function $THREAD_4_7170($BV, $BW, $BX, $BY, $BZ, $CA) {
				let $KEY_1 = $BV;
				let $KEY_5, $KEY_7;
				$KEY_5 = $BW.global.document;
				$KEY_7 = null;
				$BY(() => {
					$KEY_7 = $KEY_336($KEY_0, $KEY_5, $CB => {
						let $CC = Math.random() * 4294967296;
						let $CD = [($CC | 0) >>> 24, ($CC | 0) >> 16 & 255, ($CC | 0) >> 8 & 255, ($CC | 0) & 255];
						let $KEY_11 = $KEY_344(($CC | 0) ^ 75749485);
						let i = 0;
						let $CE = 0;
						let $CF = 0;
						let $CG = $KEY_390($KEY_11, $CB.length, $CD, $KEY_390($KEY_11, $KEY_0.version, $CD, 3));
						for (; i < $CB.length; i++) {
							let $CH = $CB[i] << $CF;
							++$CF;
							if ($CF === 8) {
								$CD[++$CG] = ($CE | $CH) ^ $KEY_11();
								$CE = 0;
								$CF = 0;
							} else {
								$CE = $CE | $CH;
							}
						}
						if ($CF !== 0) {
							$CD[$CG + 1] = $CE ^ $KEY_11();
						}
						$KEY_1.next([[], $CD]);
						$KEY_1.complete();
					});
				}, true);
				$CA(() => {
					if (typeof $KEY_7 === "function") {
						$KEY_7();
					}
				});
			}
			return $THREAD_4_7170;
		}
function $THREAD_5_30885($BU, $BV, $BW) {
			let $KEY_5 = $BW;
			let $KEY_7;
			let $KEY_4 = [];
			let $BX = $BV.createElement("canvas");
			let $KEY_2 = $BX && $BX.getContext && $BX.getContext("2d");
			if ($KEY_2 && typeof $KEY_2.measureText === "function") {
				$KEY_2.textBaseline = "top";
				$KEY_2.font = "50px  sans-serif";
				let $KEY_0 = $KEY_2.measureText("0-_{w.").width;
				$KEY_2.font = "50px  serif";
				let $KEY_1 = $KEY_2.measureText("0-_{w.").width;
				$KEY_7 = $KEY_70(($BY, $BZ) => {
					let $CA = $KEY_4;
					let $CB = $KEY_4.push;
					$KEY_2.font = "50px " + ($BY == null ? "" : "'" + $BY + "', ") + " sans-serif";
					let $CC;
					if ($KEY_2.measureText("0-_{w.").width !== $KEY_0) {
						$CC = true;
					} else {
						$KEY_2.font = "50px " + ($BY == null ? "" : "'" + $BY + "', ") + " serif";
						$CC = $KEY_2.measureText("0-_{w.").width !== $KEY_1;
					}
					$CB.call($CA, $CC);
					$BZ();
				}, $BU.fonts);
				$KEY_7.onEmpty(() => {
					$KEY_5($KEY_4);
				});
				$KEY_7.start();
			}
			function $THREAD_7_138942() {
				if ($KEY_7) {
					$KEY_7.empty();
				}
			}
			return $THREAD_7_138942;
		}
$KEY_336 = $THREAD_5_30885;
return $THREAD_1_119865;
}
bootstrap();
