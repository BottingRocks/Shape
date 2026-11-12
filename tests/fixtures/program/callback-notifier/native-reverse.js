class $THREAD_1_2521 {
			constructor() {
				this.callbacks = [];
			}
			register($BR) {
				this.callbacks.push($BR);
				return this;
			}
			unregister($BR) {
				let i = 0;
				for (; i < this.callbacks.length; i++) {
					if (this.callbacks[i] === $BR) {
						this.callbacks[i] = void 0;
					}
				}
				return this;
			}
			clear() {
				this.callbacks = [];
				return this;
			}
			notify($BR) {
				let $BS = Array.prototype.slice.call(this.callbacks);
				let $BT = $BS.length;
				let $BU = null;
				while ($BT - 1 >= 0) {
					if ($BS[$BT - 1] != null) {
						try {
							$BU = $BS[$BT - 1].call(void 0, $BR, $BU);
						} catch (e) {}
					}
					--$BT;
				}
				return $BU;
			}
		}
Object.defineProperty($THREAD_1_2521, "prototype", { writable: false });
let $KEY_368;
function $THREAD_8_110436($BR) {
			class $THREAD_7_64696 extends $BR {
				constructor() {
					super(...arguments);
				}
				register($BS) {
					$KEY_28();
					super.register($BS);
					return this;
				}
			}
			return $THREAD_7_64696;
		}
		let $BP = $THREAD_8_110436($THREAD_1_2521);
		$KEY_368 = new $BP();
