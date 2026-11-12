//           Exact ring-buffer excerpts; see residual-origins.json.
// Test harness supplies bootstrap; excerpts retain constructor carrier and consumers.
class $THREAD_7_110556 {
					constructor($BP) {
						this.i = 0;
						this.length = 0;
						let $BQ;
						if ($BP === 0) {
							$BQ = true;
						} else {
							$BQ = typeof $BP !== "number";
						}
						this.n = $BQ ? Infinity : $BP;
						this.arr = [];
					}
					put($BP) {
						this.arr[this.i] = $BP;
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
let $KEY_386 = $THREAD_7_110556;
let $KEY_39 = new $KEY_386(20);
let $CG = $KEY_39.toArray();
$KEY_39.put($KEY_1);
