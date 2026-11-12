function $THREAD_1_114447() {
						let $BS = $KEY_390.concat($KEY_78);
						let i = 0;
						for (; i < $BS.length; i++) {
							delete $KEY_18[$BS[i]];
						}
						if ($KEY_10.onBeforeInvoke != null) {
							$KEY_10.onBeforeInvoke.unregister($KEY_1);
						}
					}
