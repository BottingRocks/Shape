class $THREAD_3_37776 {
						constructor() {
							this.callbacks = [];
						}
						register($AH) {
							this.callbacks.push($AH);
							return this;
						}
						unregister($AH) {
							let i = 0;
							for (; i < this.callbacks.length; i++) {
								if (this.callbacks[i] === $AH) {
									this.callbacks[i] = void 0;
								}
							}
							return this;
						}
						clear() {
							this.callbacks = [];
							return this;
						}
						notify($AH) {
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
					}
Object.defineProperty($THREAD_3_37776, "prototype", { writable: false });
let $KEY_31;
function $THREAD_2_95998($AH) {
						class $THREAD_5_72622 extends $AH {
							constructor() {
								super(...arguments);
							}
							register($AI) {
								$KEY_218();
								super.register($AI);
								return this;
							}
						}
						return $THREAD_5_72622;
					}
					let $AF = $THREAD_2_95998($THREAD_3_37776);
					$KEY_31 = new $AF();
