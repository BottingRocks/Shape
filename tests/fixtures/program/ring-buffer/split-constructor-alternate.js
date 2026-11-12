// Real baseline excerpts; source bytes and SHA recorded in fixture-origins.json.
class $THREAD_9_106813 {
					constructor($BK) {
						this.i = 0;
						this.length = 0;
						let $BL;
						if ($BK === 0 || typeof $BK !== "number") {
							$BL = Infinity;
						} else {
							$BL = $BK;
						}
						this.n = $BL;
						this.arr = [];
					}
					put($BK) {
						this.arr[this.i] = $BK;
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
				let $KEY_432 = $THREAD_9_106813;
					let $KEY_26 = new $KEY_432(30);
								$KEY_26.put({
									a: $KEY_10(),
									b: $CP.pageX == null ? null : Math.round($CP.pageX),
									c: $CP.pageY == null ? null : Math.round($CP.pageY),
									d: [true, false],
									e: $CU.instanceOfUIEvent,
									f: $CU.markedAsTrusted,
									g: $CT.height,
									h: $CT.id,
									i: $CT.name,
									j: $CW,
									k: $CV,
									l: $CT.width,
									m: $CP.button
								});

							let $DA = $KEY_26.toArray();
