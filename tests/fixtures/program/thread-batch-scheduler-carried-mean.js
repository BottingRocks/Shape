//                           Scheduler with a carried running mean.
function bootstrap() {
let $KEY_154;
		let $D = (function(sourceString) {
			return sourceString.charCodeAt();
		});

let $KEY_120 = $D;
		function $THREAD_6_124464($BT) {
			let $BU;
			if ($BT && $BT.performance && typeof $BT.performance.now === "function") {
				$BU = $BT.performance.now.bind($BT.performance);
			} else {
				function $THREAD_7_44290() {
					return Number(new Date());
				}
				$BU = $THREAD_7_44290;
			}
			let $KEY_3 = $BU;
			let $KEY_1 = $BU();
			function $THREAD_2_31748() {
				return $KEY_3() - $KEY_1;
			}
			$THREAD_2_31748.start = $KEY_1;
			return $THREAD_2_31748;
		}
let $KEY_107 = $THREAD_6_124464;
		class $THREAD_6_95449 {
			constructor($BT) {
				this.targetBatchTimeMs = $BT;
				this._internalNow = $KEY_107(typeof global !== "undefined" && global != null && global.global === global ? global : typeof self !== "undefined" && self != null && self.self === self ? self : typeof window !== "undefined" && window != null && window.window === window ? window : void 0);
			}
			now() {
				return this._internalNow();
			}
			schedule($BT) {
				setTimeout(this._process.bind(this, $BT), 0);
			}
			_process($BT) {
				let $BU = this.targetBatchTimeMs;
				let $BV = this.now();
				let $BW = 0;
				let i = 0;
				let $BX, $BY;
				for (;; i = $BY) {
					let $BZ = this.now();
					$BX = $BT();
					let $CA = this.now();
					$BY = (i * $BW + ($CA - $BZ)) / ($BW + 1);
					++$BW;
					if (!($CA + $BY < $BV + $BU ? $BX : $CA + $BY < $BV + $BU)) break;
				}
				if ($BX) {
					setTimeout(this._process.bind(this, $BT), 0);
				}
			}
		}
$KEY_154 = $THREAD_6_95449;
		function $THREAD_2_129128($BT, $BU, $BV, $BW) {
			let $KEY_3 = $BV;
			let $KEY_4;
			let $KEY_5 = $BW;
			let $KEY_6 = $BT;
			let $KEY_1 = $BT.length >> 1 << 1;
			let $KEY_2 = 0;
			$KEY_4 = 0;
			new $KEY_154($BU).schedule(() => {
				try {
					let $BX = $KEY_2 + 400 < $KEY_1 ? $KEY_2 + 400 : $KEY_1;
					for (; $KEY_2 < $BX; $KEY_2 = $KEY_2 + 2) {
						$KEY_4 = ($KEY_4 << 5) - $KEY_4 + ($KEY_120($KEY_6[$KEY_2]) << 16 | $KEY_120($KEY_6[$KEY_2 + 1])) | 0;
					}
					if ($KEY_2 < $KEY_1) {
						return true;
					} else {
						if ($KEY_1 !== $KEY_6.length) {
							$KEY_4 = ($KEY_4 << 5) - $KEY_4 + ($KEY_120($KEY_6[$KEY_1]) << 16) | 0;
						}
						$KEY_3($KEY_4);
						return false;
					}
				} catch (e) {
					$KEY_5(e);
					return false;
				}
			});
		}
}
bootstrap();
