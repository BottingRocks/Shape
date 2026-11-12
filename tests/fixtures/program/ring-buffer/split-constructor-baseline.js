// Real baseline excerpts; source bytes and SHA recorded in fixture-origins.json.
class $THREAD_6_108461 {
			constructor($AK) {
				this.i = 0;
				this.length = 0;
				let $AL;
				if ($AK === 0 || typeof $AK !== "number") {
					$AL = Infinity;
				} else {
					$AL = $AK;
				}
				this.n = $AL;
				this.arr = [];
			}
			put($AK) {
				this.arr[this.i] = $AK;
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
		let $KEY_403 = $THREAD_6_108461;
			let $KEY_1 = new $KEY_403(20);
						let $BY = $KEY_1;
						let $BZ = $KEY_1.put;
						$BZ.call($BY, {
							visible: $BX.target.visibilityState === "visible",
							timestamp: $KEY_32()
						});

					let $BU = $KEY_1.toArray();
