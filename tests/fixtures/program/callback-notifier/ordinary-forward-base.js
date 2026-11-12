function $THREAD_5_68382() {
					"use strict";
					this.callbacks = [];
				}
function $THREAD_2_112022($AH) {
					"use strict";
					this.callbacks.push($AH);
					return this;
				}
				$THREAD_5_68382.prototype.register = $THREAD_2_112022;
				function $THREAD_1_86713($AH) {
					"use strict";
					let i = 0;
					for (; i < this.callbacks.length; i++) {
						if (this.callbacks[i] === $AH) {
							this.callbacks[i] = void 0;
						}
					}
					return this;
				}
				function $THREAD_8_124288() {
					"use strict";
					this.callbacks = [];
					return this;
				}
				$THREAD_5_68382.prototype.unregister = $THREAD_1_86713;
				$THREAD_5_68382.prototype.clear = $THREAD_8_124288;
				function $THREAD_5_11594($AH) {
					"use strict";
					let i = 0;
					let $AI = null;
					for (; i < this.callbacks.length; i++) {
						if (this.callbacks[i] != null) {
							try {
								$AI = this.callbacks[i].call(void 0, $AH, $AI);
							} catch (e) {}
						}
					}
					return $AI;
				}
				$THREAD_5_68382.prototype.notify = $THREAD_5_11594;
function $THREAD_6_103742($AH) {
					class $THREAD_4_126638 extends $AH {
						constructor() {
							super(...arguments);
						}
						register($AI) {
							$KEY_279();
							super.register($AI);
							return this;
						}
					}
					return $THREAD_4_126638;
				}
				let $AF = $THREAD_6_103742($THREAD_5_68382);
				$KEY_212 = new $AF();
				$KEY_198 = new $AF();
				$KEY_62 = new $AF();
