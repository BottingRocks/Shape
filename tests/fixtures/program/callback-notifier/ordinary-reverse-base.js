function $THREAD_5_99971() {
			"use strict";
			this.callbacks = [];
		}
function $THREAD_9_93980($BT) {
			"use strict";
			this.callbacks.push($BT);
			return this;
		}
		$THREAD_5_99971.prototype.register = $THREAD_9_93980;
		function $THREAD_2_77235($BT) {
			"use strict";
			let i = 0;
			for (; i < this.callbacks.length; i++) {
				if (this.callbacks[i] === $BT) {
					this.callbacks[i] = void 0;
				}
			}
			return this;
		}
		$THREAD_5_99971.prototype.unregister = $THREAD_2_77235;
		function $THREAD_4_158679() {
			"use strict";
			this.callbacks = [];
			return this;
		}
		$THREAD_5_99971.prototype.clear = $THREAD_4_158679;
		function $THREAD_2_99666($BT) {
			"use strict";
			let $BU = Array.prototype.slice.call(this.callbacks);
			let $BV = $BU.length;
			let $BW = null;
			while ($BV - 1 >= 0) {
				if ($BU[$BV - 1] != null) {
					try {
						$BW = $BU[$BV - 1].call(void 0, $BT, $BW);
					} catch (e) {}
				}
				--$BV;
			}
			return $BW;
		}
		$THREAD_5_99971.prototype.notify = $THREAD_2_99666;
function $THREAD_3_76512($BT) {
			class $THREAD_8_75676 extends $BT {
				constructor() {
					super(...arguments);
				}
				register($BU) {
					$KEY_328();
					super.register($BU);
					return this;
				}
			}
			return $THREAD_8_75676;
		}
		let $BR = $THREAD_3_76512($THREAD_5_99971);
		$KEY_148 = new $BR();
		$KEY_300 = new $BR();
		$KEY_199 = new $BR();
		$KEY_405 = new $BR();
