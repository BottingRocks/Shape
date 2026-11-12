// Real baseline excerpts; source bytes and SHA recorded in fixture-origins.json.
class $THREAD_1_54787 {
			constructor($BR) {
				this.i = 0;
				this.length = 0;
				this.n = $BR === 0 || typeof $BR !== "number" ? Infinity : $BR;
				this.arr = [];
			}
			put($BR) {
				this.arr[this.i] = $BR;
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
		let $KEY_176 = $THREAD_1_54787;
			let $KEY_11 = new $KEY_176(20);
						let $CM = $KEY_11;
						let $CN = $KEY_11.put;
						$CN.call($CM, {
							visible: $CL.target.visibilityState === "visible",
							timestamp: $KEY_0()
						});

					let $BX = $KEY_11.toArray();
