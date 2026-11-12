function main(){
function $THREAD_8_77279() {
					if (typeof Proxy !== "function") {
						return false;
					} else {
						let $KEY_2 = false;
						let $KEY_1 = {};
						function $THREAD_8_21212() {
							$KEY_2 = true;
							return $KEY_1;
						}
						let $AF = Object.getPrototypeOf(new Proxy({}, { getPrototypeOf: $THREAD_8_21212 }));
						let $AG = $KEY_1;
						if (!$AG) {
							throw new TypeError("Cannot set properties of undefined (setting 'value')");
						}
						if ($AF === $AG) {
							if ($KEY_2 !== null) {}
							return $KEY_2;
						} else {
							return false;
						}
					}
				}
}
main();
