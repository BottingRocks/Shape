function main(){
let $KEY_246=tester,$KEY_24=reader,$KEY_198=predicate;
function $THREAD_7_66467($AM) {
						let $KEY_1 = $AM;
						let $KEY_4 = typeof $KEY_1 === "function";
						let $AN;
						if ($KEY_4) {
							$KEY_1 = $AM;
							$AN = $KEY_246.test($KEY_24.call($KEY_1));
						} else {
							$AN = false;
						}
						if ($KEY_4) {
							$KEY_1 = $KEY_198($AM, "prototype");
						} else {
							$KEY_1 = false;
						}
						let $AO;
						if ($KEY_4 && !$KEY_1) {
							$AO = false;
							try {
								delete $AM.prototype;
								$AO = true;
							} catch (e) {}
						} else {
							$AO = false;
						}
						$KEY_4 = { a: $KEY_4 };
						$KEY_4.b = $AN;
						$KEY_4.c = !$KEY_1;
						$KEY_4.d = $AO;
						return $KEY_4;
					}
}
main();
