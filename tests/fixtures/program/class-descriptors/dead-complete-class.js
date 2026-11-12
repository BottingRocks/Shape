class $THREAD_9_151833 {
					constructor($BW) {
						this._subscription = $BW;
					}
					next($BW) {
						try {
							if (this._subscription._observer == null) {
								return void 0;
							}
							if (this._subscription._observer.next) {
								this._subscription._observer.next($BW);
								return void 0;
							} else {
								return void 0;
							}
						} catch (e) {
							return void 0;
						}
					}
					error($BW) {
						let $KEY_0 = this._subscription;
						let $BX = $KEY_0._observer;
						$KEY_0._observer = void 0;
						try {
							let $BY = $KEY_0._cleanup;
							let $BZ;
							if ($BY === void 0) {
								$BZ = $BW;
							} else {
								$KEY_0._cleanup = void 0;
								try {
									$BY();
									$BZ = $BW;
								} catch (e) {
									$BZ = $BW;
								}
							}
							if ($BX) {
								if ($BX.error) {
									$BX.error($BZ);
								}
							}
						} catch (e_2) {}
					}
					complete() {
						this._subscription._observer;
						this._subscription._observer === void 0;
					}
					isComplete() {
						return this._subscription.isComplete();
					}
				}