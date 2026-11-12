//                                  Direct-store scheduler with its own clock and aliases.
function bootstrap() {
let $KEY_63, $KEY_98;
let $D = (function(sourceString) {
			return sourceString.charCodeAt();
		});
$KEY_98 = $D;
function $THREAD_0_59817($BR) {
			let $BS;
			if ($BR && $BR.performance && typeof $BR.performance.now === "function") {
				$BS = $BR.performance.now.bind($BR.performance);
			} else {
				function $THREAD_5_150324() {
					return Number(new Date());
				}
				$BS = $THREAD_5_150324;
			}
			let $KEY_2 = $BS;
			let $KEY_0 = $BS();
			function $THREAD_1_12866() {
				return $KEY_2() - $KEY_0;
			}
			$THREAD_1_12866.start = $KEY_0;
			return $THREAD_1_12866;
		}
		let $KEY_250 = $THREAD_0_59817;
class $THREAD_4_63087 {
			constructor($BR) {
				this.targetBatchTimeMs = $BR;
				this._internalNow = $KEY_250(typeof global !== "undefined" && global != null && global.global === global ? global : typeof self !== "undefined" && self != null && self.self === self ? self : typeof window !== "undefined" && window != null && window.window === window ? window : void 0);
			}
			now() {
				return this._internalNow();
			}
			schedule($BR) {
				setTimeout(this._process.bind(this, $BR), 0);
			}
			_process($BR) {
				let $BS = this.targetBatchTimeMs;
				let $BT = this.now();
				let $BU = 0;
				let $BV = 0;
				let $BW;
				for (;;) {
					let $BX = this.now();
					$BW = $BR();
					let $BY = this.now();
					$BU = ($BU * $BV + ($BY - $BX)) / ($BV + 1);
					++$BV;
					if (!($BY + $BU < $BT + $BS ? $BW : $BY + $BU < $BT + $BS)) break;
				}
				if ($BW) {
					setTimeout(this._process.bind(this, $BR), 0);
				}
			}
		}
$KEY_63 = $THREAD_4_63087;
function $THREAD_1_151350($BR, $BS, $BT, $BU) {
			let $KEY_0 = $BR;
			let $KEY_1 = $BU;
			let $KEY_2;
			let $KEY_3 = $BT;
			let $KEY_4, $KEY_5;
			$KEY_4 = $BR.length >> 1 << 1;
			$KEY_5 = 0;
			$KEY_2 = 0;
			new $KEY_63($BS).schedule(() => {
				try {
					let $BV = $KEY_5 + 400 < $KEY_4 ? $KEY_5 + 400 : $KEY_4;
					for (; $KEY_5 < $BV; $KEY_5 = $KEY_5 + 2) {
						$KEY_2 = ($KEY_2 << 5) - $KEY_2 + ($KEY_98($KEY_0[$KEY_5]) << 16 | $KEY_98($KEY_0[$KEY_5 + 1])) | 0;
					}
					if ($KEY_5 < $KEY_4) {
						return true;
					} else {
						if ($KEY_4 !== $KEY_0.length) {
							$KEY_2 = ($KEY_2 << 5) - $KEY_2 + ($KEY_98($KEY_0[$KEY_4]) << 16) | 0;
						}
						$KEY_3($KEY_2);
						return false;
					}
				} catch (e) {
					$KEY_1(e);
					return false;
				}
			});
		}
}
bootstrap();
