// Invented captures around an exact saved own body; not a source-backed origin witness.
function bootstrap(){
function queueFactory(){return {};}
function hashFactory(){return 0;}
function numericWriter(){return 0;}
let $KEY_321 = queueFactory;
let $KEY_267 = hashFactory;
let $KEY_229;
$KEY_229 = numericWriter;
function $THREAD_7_68924($BU, $BV, $BW, $BX) {
			let $KEY_0 = $BX;
			let $KEY_1, $KEY_3, $KEY_5;
			let $KEY_6 = $BU;
			$KEY_5 = [];
			$KEY_1 = $BV.createElement("canvas");
			$KEY_3 = $KEY_321($BY => {
				let $BZ = $KEY_5;
				let $CA = $BZ.push;
				let $KEY_7 = $KEY_1;
				$KEY_7.height = 96;
				$KEY_7.width = 96;
				let $CB = [];
				try {
					let $CC = $KEY_7.getContext("2d", { willReadFrequently: true });
					$CC.font = $KEY_7.height - 2 + "px sans-serif";
					$CC.fillStyle = "#000";
					$CC.fillText($BY[0], -1, $KEY_7.height);
					let i = 0;
					for (; i < $BY[1].length; i++) {
						let $CD = $CC.getImageData($BY[1][i][0], $BY[1][i][1], 1, 1).data;
						$CB.push($CD[0] << 24 | $CD[1] << 16 | $CD[2] << 8 | $CD[3]);
					}
					$CA.call($BZ, $CB);
					let $CE = Math.random() * 4294967296;
					let $CF = [($CE | 0) >>> 24, ($CE | 0) >> 16 & 255, ($CE | 0) >> 8 & 255, ($CE | 0) & 255];
					$CE = $KEY_267(($CE | 0) ^ 1332488180);
					let $CG = $KEY_5.length;
					let $CH = 0;
					let $CI = $KEY_229($CE, $CG, $CF, 3);
					while ($CH < $CG) {
						let $CJ = $KEY_5[$CH];
						let j = $CJ.length - 1;
						$CI = $KEY_229($CE, $CJ.length, $CF, $CI);
						for (; j >= 0; j--) {
							$CI = $KEY_229($CE, $CJ[j], $CF, $CI);
						}
						++$CH;
					}
					$KEY_6.next([[], $CF]);
					return void 0;
				} catch (e) {
					$CA.call($BZ, $CB);
					let $CK = Math.random() * 4294967296;
					let $CL = [($CK | 0) >>> 24, ($CK | 0) >> 16 & 255, ($CK | 0) >> 8 & 255, ($CK | 0) & 255];
					$CK = $KEY_267(($CK | 0) ^ 1332488180);
					let $CM = $KEY_5.length;
					let $CN = 0;
					let $CO = $KEY_229($CK, $CM, $CL, 3);
					while ($CN < $CM) {
						let $CP = $KEY_5[$CN];
						let $CQ = $CP.length - 1;
						$CO = $KEY_229($CK, $CP.length, $CL, $CO);
						while ($CQ >= 0) {
							$CO = $KEY_229($CK, $CP[$CQ], $CL, $CO);
							--$CQ;
						}
						++$CN;
					}
					$KEY_6.next([[], $CL]);
					return void 0;
				}
			}, 45, $BW);
			$KEY_3.onEmpty(() => {
				$KEY_0($KEY_5);
			});
			$KEY_3.start();
			function $THREAD_7_137861() {
				if ($KEY_3) {
					$KEY_3.empty();
				}
			}
			return $THREAD_7_137861;
		}
return $THREAD_7_68924;
}
bootstrap();
