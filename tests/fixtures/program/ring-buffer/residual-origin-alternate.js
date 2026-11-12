//           Exact ring-buffer excerpts; see residual-origins.json.
// Test harness supplies bootstrap; excerpts retain constructor carrier and consumers.
class $THREAD_3_141163 {
			constructor($AL) {
				this.i = 0;
				this.length = 0;
				let $AM;
				if ($AL === 0) {
					$AM = true;
				} else {
					$AM = typeof $AL !== "number";
				}
				this.n = $AM ? Infinity : $AL;
				this.arr = [];
			}
			put($AL) {
				this.arr[this.i] = $AL;
				this.i = Number(this.i) + 1;
				if (this.length < this.n) {
					this.length = Number(this.length) + 1;
				}
				if (this.i >= this.n) {
					this.i = 0;
				}
			}
			toArray() {
				if (this.length < this.n) {
					return this.arr.slice(0, this.length);
				} else {
					return this.arr.slice(this.i, this.n).concat(this.arr.slice(0, this.i));
				}
			}
			clear() {
				this.arr = [];
				this.length = 0;
				this.i = 0;
			}
		}
let $KEY_387 = $THREAD_3_141163;
let $KEY_21 = new $KEY_387(20);
let $BA = $KEY_21.toArray();
let $BK = $KEY_21;
						let $BL = $KEY_21.put;
						$BL.call($BK, {
							visible: $BJ.target.visibilityState === "visible",
							timestamp: $KEY_7()
						});
