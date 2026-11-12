function $THREAD_3_66000_2($AZ) {
					try {
						let $BA = {
							instanceOfUIEvent: $AZ instanceof UIEvent,
							markedAsTrusted: !!$AZ.isTrusted
						};
						let $BB = [];
						let $BC = $AZ[$AZ.touches && $AZ.touches.length === 0 ? "changedTouches" : "touches"];
						if ($BC && $BC.length !== 0) {
							let $BD = $BC.length;
							let i = 0;
							for (; i < $BD; i++) {
								let $BE = $BC[i];
								let $BF = $BE.target;
								$BF = $BF instanceof Element ? $BF.getBoundingClientRect() : null;
								const $BG = $BF == null ? 0 : Math.round($BE.clientX - $BF.left);
								$BF = $BF == null ? 0 : Math.round($BE.clientY - $BF.top);
								$BF = {
									a: Math.round($BE.pageX),
									b: Math.round($BE.pageY),
									c: $BG,
									d: $BF,
									e: $BE.force
								};
								$BB.push($BF);
							}
						}
						let $BH = $KEY_25;
						let $BI = { a: [true, false] };
						$BI.b = $KEY_38();
						$BI.c = $BB;
						$BI.d = $BA.instanceOfUIEvent;
						$BI.e = $BA.markedAsTrusted;
						$BH.put($BI);
					} catch (e) {
						$KEY_36.error(e);
					}
				}