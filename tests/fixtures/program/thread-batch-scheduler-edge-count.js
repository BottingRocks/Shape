//                          Owned clock and edge-count scheduler class.
//                Bootstrap, aliases, and consumer are supplied by the fixture harness.
function bootstrap() {
// MAIN-selection fixture glue, matching the existing scheduler fixtures.
let decode = function(sourceString) { return sourceString.charCodeAt(); };
let decoder = decode;
function $THREAD_0_76570($BN) {
			let $BO;
			if ($BN && $BN.performance && typeof $BN.performance.now === "function") {
				$BO = $BN.performance.now.bind($BN.performance);
			} else {
				function $THREAD_8_120567() {
					return Number(new Date());
				}
				$BO = $THREAD_8_120567;
			}
			let $KEY_1 = $BO;
			let $KEY_0 = $BO();
			function $THREAD_4_58393() {
				return $KEY_1() - $KEY_0;
			}
			$THREAD_4_58393.start = $KEY_0;
			return $THREAD_4_58393;
		}
let $KEY_203 = $THREAD_0_76570;
class $THREAD_6_146846 {
			constructor($BN) {
				this.targetBatchTimeMs = $BN;
				this._internalNow = $KEY_203(typeof global !== "undefined" && global != null && global.global === global ? global : typeof self !== "undefined" && self != null && self.self === self ? self : typeof window !== "undefined" && window != null && window.window === window ? window : void 0);
			}
			now() {
				return this._internalNow();
			}
			schedule($BN) {
				setTimeout(this._process.bind(this, $BN), 0);
			}
			_process($BN) {
				let $BO = this.targetBatchTimeMs;
				let $BP = this.now();
				let $BQ = 0;
				let i = 0;
				let $BR, $BS;
				for (;; i = $BS) {
					let $BT = this.now();
					$BR = $BN();
					let $BU = this.now();
					$BS = Number(i) + 1;
					if (!($BU + ($BQ * i + ($BU - $BT)) / (i + 1) < $BP + $BO ? $BR : $BU + ($BQ * i + ($BU - $BT)) / (i + 1) < $BP + $BO)) break;
					$BQ = ($BQ * i + ($BU - $BT)) / (i + 1);
				}
				if ($BR) {
					setTimeout(this._process.bind(this, $BN), 0);
				}
			}
		}
let Scheduler = $THREAD_6_146846; let scheduler = new Scheduler(8); scheduler.schedule(function work() { return false; });
}
bootstrap();
