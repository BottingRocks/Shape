function $THREAD_6_78374() {
					function $THREAD_5_105024($AO) {
						let $KEY_2 = $KEY_1.getElementById("rememberMe") || $KEY_1.getElementById("TravelerLoginForm_rememberMe");
						let $AP = $KEY_1.getElementsByName("shouldRememberUser");
						$KEY_2 = $KEY_2 != null ? $KEY_2.checked : $AP.length > 0 ? $AP[0].className.indexOf("checked") > -1 : "";
						$AO.next($KEY_2);
					}
					let $AN = {
						setup: void 0,
						lastChance: void 0
					};
					let $KEY_1;
					function $THREAD_8_131385($AO) {
						$KEY_1 = $AO.global.document;
					}
					$AN.setup = $THREAD_8_131385;
					$AN.lastChance = $THREAD_5_105024;
					return $AN;
				}
