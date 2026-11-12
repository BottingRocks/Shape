//                                      Direct-store scheduler with its own clock and aliases.
function bootstrap() {
let $KEY_125;
let $B = (function(sourceString) {
					return sourceString.charCodeAt();
				});
let $KEY_224 = $B;
function $THREAD_6_38785($AH) {
					let $AI;
					if ($AH && $AH.performance && typeof $AH.performance.now === "function") {
						$AI = $AH.performance.now.bind($AH.performance);
					} else {
						function $THREAD_5_2980() {
							return Number(new Date());
						}
						$AI = $THREAD_5_2980;
					}
					let $KEY_3 = $AI;
					let $KEY_1 = $AI();
					function $THREAD_1_22656() {
						return $KEY_3() - $KEY_1;
					}
					$THREAD_1_22656.start = $KEY_1;
					return $THREAD_1_22656;
				}
				let $KEY_95 = $THREAD_6_38785;
class $THREAD_0_63645 {
					constructor($AH) {
						this.targetBatchTimeMs = $AH;
						this._internalNow = $KEY_95(typeof global !== "undefined" && global != null && global.global === global ? global : typeof self !== "undefined" && self != null && self.self === self ? self : typeof window !== "undefined" && window != null && window.window === window ? window : void 0);
					}
					now() {
						return this._internalNow();
					}
					schedule($AH) {
						setTimeout(this._process.bind(this, $AH), 0);
					}
					_process($AH) {
						let $AI = this.targetBatchTimeMs;
						let $AJ = this.now();
						let $AK = 0;
						let i = 0;
						let $AL, $AM;
						for (;; i = $AM) {
							let $AN = this.now();
							$AL = $AH();
							let $AO = this.now();
							$AK = ($AK * i + ($AO - $AN)) / (i + 1);
							$AM = Number(i) + 1;
							if (!($AO + $AK < $AJ + $AI ? $AL : $AO + $AK < $AJ + $AI)) break;
						}
						if ($AL) {
							setTimeout(this._process.bind(this, $AH), 0);
						}
					}
				}
$KEY_125 = $THREAD_0_63645;
function $THREAD_9_129383($AH, $AI, $AJ, $AK) {
					let $KEY_1 = $AH;
					let $KEY_2;
					let $KEY_3 = $AK;
					let $KEY_4;
					let $KEY_6 = $AJ;
					$KEY_2 = $AH.length >> 1 << 1;
					let $KEY_0 = 0;
					$KEY_4 = 0;
					new $KEY_125($AI).schedule(() => {
						try {
							let $AL = $KEY_0 + 400 < $KEY_2 ? $KEY_0 + 400 : $KEY_2;
							for (; $KEY_0 < $AL; $KEY_0 = $KEY_0 + 2) {
								$KEY_4 = ($KEY_4 << 5) - $KEY_4 + ($KEY_224($KEY_1[$KEY_0]) << 16 | $KEY_224($KEY_1[$KEY_0 + 1])) | 0;
							}
							if ($KEY_0 < $KEY_2) {
								return true;
							} else {
								if ($KEY_2 !== $KEY_1.length) {
									$KEY_4 = ($KEY_4 << 5) - $KEY_4 + ($KEY_224($KEY_1[$KEY_2]) << 16) | 0;
								}
								$KEY_6($KEY_4);
								return false;
							}
						} catch (e) {
							$KEY_3(e);
							return false;
						}
					});
				}
}
bootstrap();
