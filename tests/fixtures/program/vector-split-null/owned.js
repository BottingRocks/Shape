function main(){
function $THREAD_2_100633($AU, $AV) {
					let $KEY_0 = $AU;
					if ($AV.a.some($AW => {
						let $AX;
						if ($KEY_0 == null) {
							$AX = true;
						} else {
							$AX = $AW == null;
						}
						if ($AX || $KEY_0.length !== $AW.length) {
							return false;
						} else {
							let $AY = 0;
							let $AZ;
							for (;;) {
								$AZ = void 0;
								if ($AY >= $KEY_0.length) {
									$AZ = true;
									break;
								}
								if ($KEY_0[$AY] === $AW[$AY]) {
									++$AY;
									continue;
								}
								$AZ = false;
								break;
							}
							return $AZ;
						}
					})) {
						return void 0;
					}
					$AV.b = Number($AV.b) + 1;
					if ($AV.a.length > 5) {
						return void 0;
					} else {
						$AV.a.push(Array.prototype.slice.call($KEY_0));
						return void 0;
					}
				}
return $THREAD_2_100633;
}main();
