class $THREAD_7_110550 {
			constructor($BP) {
				this._subscription = $BP;
			}
			next($BP) {
				try {
					if (this._subscription._observer == null) {
						return void 0;
					}
					if (this._subscription._observer.next) {
						this._subscription._observer.next($BP);
						return void 0;
					} else {
						return void 0;
					}
				} catch (e) {
					return void 0;
				}
			}
			error($BP) {
				let $KEY_0 = this._subscription;
				let $BQ = $KEY_0._observer;
				$KEY_0._observer = void 0;
				try {
					let $BR = $KEY_0._cleanup;
					let $BS;
					if ($BR === void 0) {
						$BS = $BP;
					} else {
						$KEY_0._cleanup = void 0;
						try {
							$BR();
							$BS = $BP;
						} catch (e) {
							$BS = $BP;
						}
					}
					if ($BQ) {
						if ($BQ.error) {
							let $BT = [$BQ.error, $BQ.error.call];
							$BT[1].call($BT[0], $BQ, $BS);
						}
					}
				} catch (e_2) {}
			}
			complete() {
				let $KEY_3 = this._subscription;
				let $BP = $KEY_3._observer;
				if ($KEY_3._observer === void 0) {
					return void 0;
				}
				$KEY_3._observer = void 0;
				try {
					if (typeof ($BP ? $BP.complete : void 0) === "function") {
						($BP ? $BP.complete : void 0).call($BP);
					}
				} catch (e) {}
				let $BQ, $BR;
				try {
					let $BS = $KEY_3._cleanup;
					if ($BS !== void 0) {
						$KEY_3._cleanup = void 0;
						try {
							$BS();
						} catch (e_2) {}
					}
					$BQ = false;
					$BR = void 0;
				} catch (e_3) {
					$BQ = true;
					$BR = e_3;
				}
				if ($BQ) {
					throw $BR;
				} else {
					return void 0;
				}
			}
			isComplete() {
				return this._subscription.isComplete();
			}
		}
		let $S = { key: "closed" };
		function $THREAD_0_110595() {
			"use strict";
			return this._subscription._observer === void 0;
		}
		$S.get = $THREAD_0_110595;
		$S = [$S];
		for (let j = 0; j < $S.length; j++) {
			$S[j].enumerable = $S[j].enumerable || false;
			$S[j].configurable = true;
			if ("value" in $S[j]) {
				$S[j].writable = true;
			}
			Object.defineProperty($THREAD_7_110550.prototype, $S[j].key, $S[j]);
		}
		Object.defineProperty($THREAD_7_110550, "prototype", { writable: false });