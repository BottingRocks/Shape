//                                Count-first clock factory, alias, and scheduler class.
function bootstrap() {
// Inert harness bindings make this invoked wrapper the broadest main scope.
let fixture0, fixture1, fixture2, fixture3, fixture4, fixture5, fixture6, fixture7;
function $THREAD_5_105584($BU) {
			let $BV;
			if ($BU && $BU.performance && typeof $BU.performance.now === "function") {
				$BV = $BU.performance.now.bind($BU.performance);
			} else {
				function $THREAD_0_66448() {
					return Number(new Date());
				}
				$BV = $THREAD_0_66448;
			}
			let $KEY_1 = $BV;
			let $KEY_3 = $BV();
			function $THREAD_5_99895() {
				return $KEY_1() - $KEY_3;
			}
			$THREAD_5_99895.start = $KEY_3;
			return $THREAD_5_99895;
		}
let $KEY_354 = $THREAD_5_105584;
class $THREAD_5_114565 {
			constructor($BU) {
				this.targetBatchTimeMs = $BU;
				this._internalNow = $KEY_354(typeof global !== "undefined" && global != null && global.global === global ? global : typeof self !== "undefined" && self != null && self.self === self ? self : typeof window !== "undefined" && window != null && window.window === window ? window : void 0);
			}
			now() {
				return this._internalNow();
			}
			schedule($BU) {
				setTimeout(this._process.bind(this, $BU), 0);
			}
			_process($BU) {
				let $BV = this.targetBatchTimeMs;
				let $BW = this.now();
				let $BX = 0;
				let $BY = 0;
				let $BZ;
				for (;;) {
					let $CA = this.now();
					$BZ = $BU();
					let $CB = this.now();
					$BY = ($BY * $BX + ($CB - $CA)) / ($BX + 1);
					++$BX;
					if (!($CB + $BY < $BW + $BV ? $BZ : $CB + $BY < $BW + $BV)) break;
				}
				if ($BZ) {
					setTimeout(this._process.bind(this, $BU), 0);
				}
			}
		}
new $THREAD_5_114565(8);
}
bootstrap();
