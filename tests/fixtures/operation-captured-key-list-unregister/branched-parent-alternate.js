function main(){let $KEY_8,$KEY_2,$KEY_331,$KEY_281,$KEY_9,$KEY_0,$KEY_7,$KEY_5;
function $THREAD_6_110682() {
							if ($KEY_8 == null) {
								return void 0;
							}
							if ($KEY_2 == null) {
								return void 0;
							} else {
								let $CM = $KEY_2;
								let $CN = $KEY_331.concat($KEY_281);
								let i = 0;
								for (; i < $CN.length; i++) {
									delete $CM[$CN[i]];
								}
								let $CO, $CP;
								if ($KEY_9 == null) {
									$CO = $KEY_9 == null;
									$CP = void 0;
								} else {
									$CP = $KEY_9.onBeforeInvoke;
									$CO = $CP == null;
								}
								if (!$CO) {
									$CP.unregister($KEY_7);
								}
								let $CQ, $CR;
								if ($KEY_0 == null) {
									$CQ = $KEY_0 == null;
									$CR = void 0;
								} else {
									$CR = $KEY_0.onBeforeInvoke;
									$CQ = $CR == null;
								}
								if (!$CQ) {
									$CR.unregister($KEY_5);
								}
								return void 0;
							}
						}
return $THREAD_6_110682;}main();
