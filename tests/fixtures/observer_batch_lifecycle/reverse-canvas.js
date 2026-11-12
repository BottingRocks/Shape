// Invented opaque captures around an exact owned body; actual seed verified by CLI.
function bootstrap(){
function queueFactory(){} function hashFactory(){} function numericWriter(){}
let $KEY_95=queueFactory, $KEY_19=hashFactory, $KEY_74=numericWriter;
function $THREAD_5_51829($AI, $AJ, $AK, $AL) {
					let $KEY_1 = $AL;
					let $KEY_2, $KEY_4;
					let $KEY_5 = $AI;
					let $KEY_6;
					$KEY_4 = [];
					$KEY_6 = $AJ.createElement("canvas");
					$KEY_2 = $KEY_95($AN => {
						let $AO = $KEY_4;
						let $AP = $KEY_4.push;
						let $KEY_9 = $KEY_6;
						$KEY_9.height = 96;
						$KEY_9.width = 96;
						let $AQ = [];
						try {
							let $AR = $KEY_9.getContext("2d", { willReadFrequently: true });
							$AR.font = $KEY_9.height - 2 + "px sans-serif";
							$AR.fillStyle = "#000";
							let $AS = [$AR, $AR.fillText];
							$AS[1].call($AS[0], $AN[0], -1, $KEY_9.height);
							let i = 0;
							for (; i < $AN[1].length; i++) {
								let $AT = $AR.getImageData($AN[1][i][0], $AN[1][i][1], 1, 1).data;
								$AQ.push($AT[0] << 24 | $AT[1] << 16 | $AT[2] << 8 | $AT[3]);
							}
							$AP.call($AO, $AQ);
							let $AU = Math.random() * 4294967296;
							let $AV = [($AU | 0) >>> 24, ($AU | 0) >> 16 & 255, ($AU | 0) >> 8 & 255, ($AU | 0) & 255];
							$AU = $KEY_19(($AU | 0) ^ 2402134385);
							let $AW = $KEY_4.length - 1;
							let $AX = $KEY_74($AU, $KEY_4.length, $AV, 3);
							while ($AW >= 0) {
								let $AY = $KEY_4[$AW];
								let $AZ = $AY.length;
								$AX = $KEY_74($AU, $AZ, $AV, $AX);
								for (;;) {
									--$AZ;
									if (!($AZ >= 0)) break;
									$AX = $KEY_74($AU, $AY[$AZ], $AV, $AX);
									$AZ = Number($AZ);
								}
								--$AW;
							}
							$KEY_5.next([[], $AV]);
							return void 0;
						} catch (e) {
							$AP.call($AO, $AQ);
							let $BA = Math.random() * 4294967296;
							let $BB = [($BA | 0) >>> 24, ($BA | 0) >> 16 & 255, ($BA | 0) >> 8 & 255, ($BA | 0) & 255];
							$BA = $KEY_19(($BA | 0) ^ 2402134385);
							let $BC = $KEY_4.length - 1;
							let $BD = $KEY_74($BA, $KEY_4.length, $BB, 3);
							while ($BC >= 0) {
								let $BE = $KEY_4[$BC];
								let $BF = $BE.length;
								$BD = $KEY_74($BA, $BF, $BB, $BD);
								for (;;) {
									--$BF;
									if (!($BF >= 0)) break;
									$BD = $KEY_74($BA, $BE[$BF], $BB, $BD);
									$BF = Number($BF);
								}
								--$BC;
							}
							$KEY_5.next([[], $BB]);
							return void 0;
						}
					}, 45, $AK);
					let $AM = [$KEY_2, $KEY_2.onEmpty];
					$AM[1].call($AM[0], () => {
						$KEY_1($KEY_4);
					});
					$KEY_2.start();
					function $THREAD_7_116958() {
						if ($KEY_2) {
							$KEY_2.empty();
						}
					}
					return $THREAD_7_116958;
				}
return $THREAD_5_51829;
}
bootstrap();
