//                                  Count-first clock factory, alias, and scheduler class.
function bootstrap() {
// Inert harness bindings make this invoked wrapper the broadest main scope.
let fixture0, fixture1, fixture2, fixture3, fixture4, fixture5, fixture6, fixture7;
function $THREAD_2_127412($AL) {
			let $AM;
			if ($AL && $AL.performance && typeof $AL.performance.now === "function") {
				$AM = $AL.performance.now.bind($AL.performance);
			} else {
				function $THREAD_9_93967() {
					return Number(new Date());
				}
				$AM = $THREAD_9_93967;
			}
			let $KEY_3 = $AM;
			let $KEY_1 = $AM();
			function $THREAD_3_11557() {
				return $KEY_3() - $KEY_1;
			}
			$THREAD_3_11557.start = $KEY_1;
			return $THREAD_3_11557;
		}
let $KEY_289 = $THREAD_2_127412;
class $THREAD_3_75778 {
			constructor($AL) {
				this.targetBatchTimeMs = $AL;
				this._internalNow = $KEY_289(typeof global !== "undefined" && global != null && global.global === global ? global : typeof self !== "undefined" && self != null && self.self === self ? self : typeof window !== "undefined" && window != null && window.window === window ? window : void 0);
			}
			now() {
				return this._internalNow();
			}
			schedule($AL) {
				setTimeout(this._process.bind(this, $AL), 0);
			}
			_process($AL) {
				let $AM = this.targetBatchTimeMs;
				let $AN = this.now();
				let $AO = 0;
				let $AP = 0;
				let $AQ;
				for (;;) {
					let $AR = this.now();
					$AQ = $AL();
					let $AS = this.now();
					$AP = ($AP * $AO + ($AS - $AR)) / ($AO + 1);
					++$AO;
					if (!($AS + $AP < $AN + $AM ? $AQ : $AS + $AP < $AN + $AM)) break;
				}
				if ($AQ) {
					setTimeout(this._process.bind(this, $AL), 0);
				}
			}
		}
new $THREAD_3_75778(8);
}
bootstrap();
