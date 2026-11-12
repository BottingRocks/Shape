// Real baseline excerpts; source bytes and SHA recorded in fixture-origins.json.
class $THREAD_2_113155 {
					constructor($BQ) {
						this.i = 0;
						this.length = 0;
						let $BR;
						if ($BQ === 0 || typeof $BQ !== "number") {
							$BR = Infinity;
						} else {
							$BR = $BQ;
						}
						this.n = $BR;
						this.arr = [];
					}
					put($BQ) {
						this.arr[this.i] = $BQ;
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
				let $KEY_112 = $THREAD_2_113155;
					let $KEY_4 = new $KEY_112(20);
								let $CJ = $KEY_4;
								let $CK = $KEY_4.put;
								$CK.call($CJ, {
									visible: $CI.target.visibilityState === "visible",
									timestamp: $KEY_34()
								});

							let $CD = $KEY_4.toArray();
