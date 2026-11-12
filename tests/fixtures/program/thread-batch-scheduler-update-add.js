//                    Clock and update-add scheduler; harness supplies bootstrap and consumer.
//                                                    Preserve the whole class body and its source identities.
function bootstrap() {
let decode = function(sourceString) { return sourceString.charCodeAt(); };
let decoder = decode;
function $THREAD_6_20734($BM) {
					let $BN;
					if ($BM && $BM.performance && typeof $BM.performance.now === "function") {
						$BN = $BM.performance.now.bind($BM.performance);
					} else {
						function $THREAD_7_52261() {
							return Number(new Date());
						}
						$BN = $THREAD_7_52261;
					}
					let $KEY_1 = $BN;
					let $KEY_3 = $BN();
					function $THREAD_3_23184() {
						return $KEY_1() - $KEY_3;
					}
					$THREAD_3_23184.start = $KEY_3;
					return $THREAD_3_23184;
				}
let $KEY_434;
$KEY_434 = $THREAD_6_20734;
class $THREAD_1_143030 {
					constructor($BM) {
						this.targetBatchTimeMs = $BM;
						this._internalNow = $KEY_434(typeof global !== "undefined" && global != null && global.global === global ? global : typeof self !== "undefined" && self != null && self.self === self ? self : typeof window !== "undefined" && window != null && window.window === window ? window : void 0);
					}
					now() {
						return this._internalNow();
					}
					schedule($BM) {
						setTimeout(this._process.bind(this, $BM), 0);
					}
					_process($BM) {
						let $BN = this.targetBatchTimeMs;
						let $BO = this.now();
						let $BP = 0;
						let i = 0;
						let $BQ, $BR;
						for (;; i = $BR + 1) {
							let $BS = this.now();
							$BQ = $BM();
							let $BT = this.now();
							$BP = ($BP * i + ($BT - $BS)) / (i + 1);
							$BR = Number(i);
							if (!($BT + $BP < $BO + $BN ? $BQ : $BT + $BP < $BO + $BN)) break;
						}
						if ($BQ) {
							setTimeout(this._process.bind(this, $BM), 0);
						}
					}
				}
let $KEY_83;
$KEY_83 = $THREAD_1_143030;
let scheduler = new $KEY_83(8); scheduler.schedule(function work() { return false; });
}
bootstrap();
