function main(){let $KEY_21,$KEY_11,$KEY_48,$KEY_243,$KEY_22,$KEY_8,$KEY_17,$KEY_20;
function $THREAD_3_86302() {
						if ($KEY_21 == null) {
							return void 0;
						}
						if ($KEY_11 == null) {
							return void 0;
						} else {
							let $DW = $KEY_11;
							let $DX = $KEY_48.concat($KEY_243);
							let i = 0;
							for (; i < $DX.length; i++) {
								delete $DW[$DX[i]];
							}
							let $DY, $DZ;
							if ($KEY_22 == null) {
								$DY = $KEY_22 == null;
								$DZ = void 0;
							} else {
								$DZ = $KEY_22.onBeforeInvoke;
								$DY = $DZ == null;
							}
							if (!$DY) {
								$DZ.unregister($KEY_17);
							}
							let $EA, $EB;
							if ($KEY_8 == null) {
								$EA = $KEY_8 == null;
								$EB = void 0;
							} else {
								$EB = $KEY_8.onBeforeInvoke;
								$EA = $EB == null;
							}
							if (!$EA) {
								$EB.unregister($KEY_20);
							}
							return void 0;
						}
					}
return $THREAD_3_86302;}main();
