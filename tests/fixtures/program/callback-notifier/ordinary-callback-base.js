class $THREAD_2_71325 {
			constructor() {
				this.callbacks = [];
			}
			register($AJ) {
				this.callbacks.push($AJ);
				return this;
			}
			unregister($AJ) {
				let i = 0;
				for (; i < this.callbacks.length; i++) {
					if (this.callbacks[i] === $AJ) {
						this.callbacks[i] = void 0;
					}
				}
				return this;
			}
			clear() {
				this.callbacks = [];
				return this;
			}
			notify($AJ) {
				let $AK = Array.prototype.slice.call(this.callbacks);
				let i = $AK.length;
				let $AL = null;
				let $AM;
				for (;; i = Number($AM)) {
					$AM = i - 1;
					if (!($AM >= 0)) break;
					let $AN = $AK[$AM];
					if ($AN != null) {
						try {
							$AL = $AN($AJ, $AL);
						} catch (e) {}
					}
				}
				return $AL;
			}
		}
function $THREAD_6_10401($AJ) {
			class $THREAD_8_91336 extends $AJ {
				constructor() {
					super(...arguments);
				}
				register($AK) {
					$KEY_309();
					super.register($AK);
					return this;
				}
			}
			return $THREAD_8_91336;
		}
		let $AI = $THREAD_6_10401($THREAD_2_71325);
		$KEY_0 = new $AI();
		$KEY_375 = new $AI();
		$KEY_131 = new $AI();
