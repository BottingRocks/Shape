//                              Scheduler with a delayed running mean.
function bootstrap() {
let $KEY_257, $KEY_311;
		let $E = (function(sourceString) {
			return sourceString.charCodeAt();
		});

let $KEY_78 = $E;
		function $THREAD_9_129514($BS) {
			let $BT;
			if ($BS && $BS.performance && typeof $BS.performance.now === "function") {
				$BT = $BS.performance.now.bind($BS.performance);
			} else {
				function $THREAD_2_76768() {
					return Number(new Date());
				}
				$BT = $THREAD_2_76768;
			}
			let $KEY_3 = $BT;
			let $KEY_2 = $BT();
			function $THREAD_3_142038() {
				return $KEY_3() - $KEY_2;
			}
			$THREAD_3_142038.start = $KEY_2;
			return $THREAD_3_142038;
		}
$KEY_311 = $THREAD_9_129514;
		class $THREAD_3_61704 {
			constructor($BS) {
				this.targetBatchTimeMs = $BS;
				let $BT;
				if (typeof global !== "undefined" && global != null && global.global === global) {
					$BT = global;
				} else if (typeof self !== "undefined" && self != null && self.self === self) {
					$BT = self;
				} else if (typeof window !== "undefined" && window != null && window.window === window) {
					$BT = window;
				} else {
					$BT = void 0;
				}
				this._internalNow = $KEY_311($BT);
			}
			now() {
				return this._internalNow();
			}
			schedule($BS) {
				setTimeout(this._process.bind(this, $BS), 0);
			}
			_process($BS) {
				let $BT = this.targetBatchTimeMs;
				let $BU = this.now();
				let $BV = 0;
				let $BW = 0;
				let $BX;
				for (;;) {
					let $BY = this.now();
					$BX = $BS();
					let $BZ = this.now();
					++$BW;
					if (!($BZ + ($BV * $BW + ($BZ - $BY)) / ($BW + 1) < $BU + $BT ? $BX : $BZ + ($BV * $BW + ($BZ - $BY)) / ($BW + 1) < $BU + $BT)) break;
					$BV = ($BV * $BW + ($BZ - $BY)) / ($BW + 1);
				}
				if ($BX) {
					setTimeout(this._process.bind(this, $BS), 0);
				}
			}
		}
$KEY_257 = $THREAD_3_61704;
		function $THREAD_3_106676($BS, $BT, $BU, $BV) {
			let $KEY_3 = $BS;
			let $KEY_4;
			let $KEY_5 = $BV;
			let $KEY_7 = $BU;
			let $KEY_8 = $BS.length >> 1 << 1;
			let $KEY_0 = 0;
			$KEY_4 = 0;
			new $KEY_257($BT).schedule(() => {
				try {
					let $BW = $KEY_0 + 400 < $KEY_8 ? $KEY_0 + 400 : $KEY_8;
					for (; $KEY_0 < $BW; $KEY_0 = $KEY_0 + 2) {
						$KEY_4 = ($KEY_4 << 5) - $KEY_4 + ($KEY_78($KEY_3[$KEY_0]) << 16 | $KEY_78($KEY_3[$KEY_0 + 1])) | 0;
					}
					if ($KEY_0 < $KEY_8) {
						return true;
					} else {
						if ($KEY_8 !== $KEY_3.length) {
							$KEY_4 = ($KEY_4 << 5) - $KEY_4 + ($KEY_78($KEY_3[$KEY_8]) << 16) | 0;
						}
						$KEY_7($KEY_4);
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
