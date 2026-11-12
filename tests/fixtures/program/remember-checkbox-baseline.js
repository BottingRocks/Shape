function $THREAD_4_11611() {
					function $THREAD_8_18384($AJ) {
						let $KEY_4 = $KEY_0.getElementById("rememberMe") || $KEY_0.getElementById("TravelerLoginForm_rememberMe");
						let $AK = $KEY_0.getElementsByName("shouldRememberUser");
						$KEY_4 = $KEY_4 != null ? $KEY_4.checked : $AK.length > 0 ? $AK[0].className.indexOf("checked") > -1 : "";
						$AJ.next($KEY_4);
					}
					let $AI = {
						setup: void 0,
						lastChance: void 0
					};
					let $KEY_0;
					function $THREAD_4_49952($AJ) {
						$KEY_0 = $AJ.global.document;
					}
					$AI.setup = $THREAD_4_49952;
					$AI.lastChance = $THREAD_8_18384;
					return $AI;
				}
