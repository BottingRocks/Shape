//                Installed subscription classes and setup; never executed.
function bootstrap() {
{
class $THREAD_8_57126 {
			constructor($BT) {
				this._cleanup = void 0;
				this._observer = $BT;
			}
			unsubscribe() {
				if (this.isComplete()) {
					return void 0;
				} else {
					this._observer = void 0;
					let $BT = this._cleanup;
					if ($BT !== void 0) {
						this._cleanup = void 0;
						try {
							$BT();
						} catch (e) {}
					}
					return void 0;
				}
			}
			isComplete() {
				return this._observer === void 0;
			}
		}
		let $P = { key: "closed" };
		function $THREAD_4_69994() {
			"use strict";
			return this._observer === void 0;
		}
		$P.get = $THREAD_4_69994;
		$P = [$P];
		let i = 0;
		for (; i < $P.length; i++) {
			$P[i].enumerable = $P[i].enumerable || false;
			$P[i].configurable = true;
			if ("value" in $P[i]) {
				$P[i].writable = true;
			}
			let $Q;
			if (typeof $P[i].key != "object" ? typeof $P[i].key != "object" : !$P[i].key) {
				$Q = $P[i].key;
			} else if ($P[i].key[Symbol.toPrimitive] !== void 0) {
				$Q = $P[i].key[Symbol.toPrimitive].call($P[i].key, "string");
				if (typeof $Q == "object") {
					throw new TypeError("@@toPrimitive must return a primitive value.");
				}
			} else {
				$Q = String($P[i].key);
			}
			Object.defineProperty($THREAD_8_57126.prototype, typeof $Q == "symbol" ? $Q : $Q + "", $P[i]);
		}
		Object.defineProperty($THREAD_8_57126, "prototype", { writable: false });
		let $KEY_419 = $THREAD_8_57126;
new $KEY_419({});
}
{
class $THREAD_0_94586 {
			constructor($BT) {
				this._subscription = $BT;
			}
			next($BT) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($BT);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($BT) {
				let $KEY_4 = this._subscription;
				let $BU = $KEY_4._observer;
				$KEY_4._observer = void 0;
				try {
					let $BV = $KEY_4._cleanup;
					let $BW;
					if ($BV === void 0) {
						$BW = $BT;
					} else {
						$KEY_4._cleanup = void 0;
						try {
							$BV();
							$BW = $BT;
						} catch (e) {
							$BW = $BT;
						}
					}
					if ($BU) {
						if ($BU.error) {
							$BU.error($BW);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_5 = this._subscription;
				let $BT = $KEY_5._observer;
				if ($KEY_5._observer === void 0) {
					return void 0;
				}
				$KEY_5._observer = void 0;
				try {
					if (typeof ($BT ? $BT.complete : void 0) === "function") {
						($BT ? $BT.complete : void 0).call($BT);
					}
				} catch (e) {}
				let $BU, $BV;
				try {
					let $BW = $KEY_5._cleanup;
					if ($BW !== void 0) {
						$KEY_5._cleanup = void 0;
						try {
							$BW();
						} catch (e_2) {}
					}
					$BU = false;
					$BV = void 0;
				} catch (e_3) {
					$BU = true;
					$BV = e_3;
				}
				if ($BU) {
					throw $BV;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}
		let $R = { key: "closed" };
		function $THREAD_9_149346() {
			"use strict";
			return this._subscription._observer === void 0;
		}
		$R.get = $THREAD_9_149346;
		$R = [$R];
		let j = 0;
		for (; j < $R.length; j++) {
			$R[j].enumerable = $R[j].enumerable || false;
			$R[j].configurable = true;
			if ("value" in $R[j]) {
				$R[j].writable = true;
			}
			let $S;
			if (typeof $R[j].key != "object" ? typeof $R[j].key != "object" : !$R[j].key) {
				$S = $R[j].key;
			} else if ($R[j].key[Symbol.toPrimitive] !== void 0) {
				$S = $R[j].key[Symbol.toPrimitive].call($R[j].key, "string");
				if (typeof $S == "object") {
					throw new TypeError("@@toPrimitive must return a primitive value.");
				}
			} else {
				$S = String($R[j].key);
			}
			Object.defineProperty($THREAD_0_94586.prototype, typeof $S == "symbol" ? $S : $S + "", $R[j]);
		}
		Object.defineProperty($THREAD_0_94586, "prototype", { writable: false });
		let $KEY_100 = $THREAD_0_94586;
new $KEY_100({});
}
}
bootstrap();
