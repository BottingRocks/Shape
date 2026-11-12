function $THREAD_3_133961($DW, $DX) {
												try {
													if ($DX != null && $DX.bypassResult != null) {
														return $DX;
													}
													if ($DW.args == null || $DW.args.length < 2) {
														return $DX;
													} else {
														if ($DW.args[0] != null) {
															$KEY_0 = "" + $DW.args[0].toLowerCase();
															$KEY_2 = $DW.args[1];
															let $DY = {
																writable: true,
																configurable: true,
																enumerable: false
															};
															let $DZ = {
																method: $KEY_0,
																url: $KEY_2
															};
															$DY.value = $DZ;
															Object.defineProperty($DW.thisObj, $KEY_346[0], $DY);
														}
														return $DX;
													}
												} catch (e) {
													let $EA = {
														message: "2.31",
														stack: void 0
													};
													$EA.stack = e + "";
													$KEY_102($EA);
													return $DX;
												}
											}