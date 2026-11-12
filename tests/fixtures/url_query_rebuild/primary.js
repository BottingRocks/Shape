function main(){ let $carrier=0; let $KEY_58,$KEY_31,$KEY_184;
				$KEY_58 = {
					ftp: "21",
					gopher: "70",
					http: "80",
					https: "443",
					ws: "80",
					wss: "443"
				};
				let $AA = Object.create(null);
				let $AB = Object.getOwnPropertyDescriptor(window.HTMLAnchorElement.prototype, "href");
				let $AC;
				if ($AB != null && $AB.get != null && $AB.set != null) {
					$AA.anchorHrefGet = Object.call.bind($AB.get);
					$AC = Object.call.bind($AB.set);
				} else {
					function $THREAD_0_59429($AI) {
						return $AI.href;
					}
					function $THREAD_5_54625($AI, $AJ) {
						$AI.href = $AJ;
					}
					$AA.anchorHrefGet = $THREAD_0_59429;
					$AC = $THREAD_5_54625;
				}
				$AA.anchorHrefSet = $AC;
				$KEY_31 = $AA;
				$KEY_184 = Object.create(null);
function $THREAD_7_118108($AI, $AJ, $AK) {
					let $KEY_1, $KEY_4, $KEY_5;
					let $KEY_10 = "";
					let $KEY_9 = $AI || window.location + "";
					let $AL = RegExp("^https?:\\/\\/").test($KEY_9);
					if ($AL) {
						if ($KEY_184[$KEY_9] != null) {
							let $KEY_6 = $KEY_184[$KEY_9].protocol;
							let $KEY_12 = $KEY_184[$KEY_9].hostname;
							let $KEY_7 = $KEY_184[$KEY_9].port;
							let $KEY_17 = $KEY_184[$KEY_9].pathname;
							$KEY_9 = $KEY_184[$KEY_9].hash;
							let $AM;
							if ($AI == null || $AI.trim() === "") {
								$AM = [];
							} else {
								let $AN = document.createElement("a");
								$AN.href = $AI;
								let $AO = $AN.search.slice(1);
								if ($AO === "") {
									$AM = [];
								} else {
									$AO = [$AO, $AO.split];
									$AM = $AO[1].call($AO[0], "&").map($BA => {
										let $BB = $BA.split("=");
										const $BC = typeof $BB[0] === "undefined" ? "undefined" : $BB[0] === null ? "null" : decodeURIComponent($BB[0].toString()).replace(new RegExp(RegExp("\\+", "g")), " ");
										let $BD;
										if (typeof $BB[1] === "undefined") {
											$BD = "";
										} else {
											$BD = typeof $BB[1] === "undefined" ? "undefined" : $BB[1] === null ? "null" : decodeURIComponent($BB[1].toString()).replace(new RegExp(RegExp("\\+", "g")), " ");
										}
										$BD = [$BC, $BD];
										return $BD;
									});
								}
							}
							$KEY_5 = typeof $AJ === "undefined" ? "undefined" : $AJ === null ? "null" : encodeURIComponent($AJ.toString()).replace(new RegExp(RegExp("%20", "g")), "+");
							$KEY_1 = typeof $AK === "undefined" ? "undefined" : $AK === null ? "null" : encodeURIComponent($AK.toString()).replace(new RegExp(RegExp("%20", "g")), "+");
							if ($AM.length > 0) {
								$KEY_4 = false;
								$AM.forEach(($BA, $BB) => {
									const $BC = typeof $BA[0] === "undefined" ? "undefined" : $BA[0] === null ? "null" : encodeURIComponent($BA[0].toString()).replace(new RegExp(RegExp("%20", "g")), "+");
									const $BD = typeof $BA[1] === "undefined" ? "undefined" : $BA[1] === null ? "null" : encodeURIComponent($BA[1].toString()).replace(new RegExp(RegExp("%20", "g")), "+");
									if ($BC === $KEY_5 && !$KEY_4) {
										$KEY_10 = $KEY_10 + (($BB === 0 ? $KEY_5 : "&" + $KEY_5) + "=" + $KEY_1);
										$KEY_4 = true;
									} else if ($BC !== $KEY_5) {
										$KEY_10 = $KEY_10 + (($BB === 0 ? $BC + "=" : "&" + $BC + "=") + $BD);
									}
								});
								if (!$KEY_4) {
									$KEY_10 = $KEY_10 + ("&" + $KEY_5 + "=" + $KEY_1);
								}
							} else {
								$KEY_10 = $KEY_5 + "=" + $KEY_1;
							}
							return $KEY_6 + "://" + $KEY_12 + ($KEY_7.length > 0 ? ":" + $KEY_7 : "") + $KEY_17 + ($KEY_10.length > 0 ? "?" + $KEY_10 : "") + ($KEY_9.length > 0 ? "#" + $KEY_9 : "");
						}
					}
					let $AP = $KEY_31;
					let $AQ = document.createElement("a");
					$AP.anchorHrefSet($AQ, $KEY_9);
					if ($AQ.host === "") {
						$AP.anchorHrefSet($AQ, $AP.anchorHrefGet($AQ));
					}
					let $AR = $AQ.protocol.slice(0, $AQ.protocol.length - 1);
					let $AS;
					if ($AQ.pathname[0] === "/") {
						$AS = $AQ.pathname.replace(RegExp("(%(2[146-9a-f]|3[0-9abd]|4[0-9a-f]|5[0-9abdf]|6[1-9a-f]|7[0-9ae]))", "gi"), $BA => decodeURIComponent($BA));
					} else {
						$AS = "/" + $AQ.pathname.replace(RegExp("(%(2[146-9a-f]|3[0-9abd]|4[0-9a-f]|5[0-9abdf]|6[1-9a-f]|7[0-9ae]))", "gi"), $BA => decodeURIComponent($BA));
					}
					let $AT = $AQ.search.replace(RegExp("(%(2[146-9a-f]|3[0-9abd]|4[0-9a-f]|5[0-9abdf]|6[1-9a-f]|7[0-9ae]))", "gi"), $BA => decodeURIComponent($BA)).slice(1);
					let $AU = $AQ.hash.replace(RegExp("(%(2[146-9a-f]|3[0-9abd]|4[0-9a-f]|5[0-9abdf]|6[1-9a-f]|7[0-9ae]))", "gi"), $BA => decodeURIComponent($BA)).slice(1);
					let $AV = {
						protocol: $AR,
						hostname: $AQ.hostname,
						port: $AQ.port === $KEY_58[$AR] ? "" : $AQ.port,
						pathname: $AS,
						search: $AT,
						hash: $AU
					};
					$AV.href = $AR + "://" + $AQ.hostname + (($AQ.port === $KEY_58[$AR] ? "" : $AQ.port).length > 0 ? ":" + ($AQ.port === $KEY_58[$AR] ? "" : $AQ.port) : "") + $AS + ($AT.length > 0 ? "?" + $AT : "") + ($AU.length > 0 ? "#" + $AU : "");
					if ($AL) {
						$KEY_184[$KEY_9] = $AV;
					}
					let $AW;
					if ($AI == null || $AI.trim() === "") {
						$AW = [];
					} else {
						let $AX = document.createElement("a");
						$AX.href = $AI;
						let $AY = $AX.search.slice(1);
						if ($AY === "") {
							$AW = [];
						} else {
							$AY = [$AY, $AY.split];
							$AW = $AY[1].call($AY[0], "&").map($BA => {
								let $BB = $BA.split("=");
								const $BC = typeof $BB[0] === "undefined" ? "undefined" : $BB[0] === null ? "null" : decodeURIComponent($BB[0].toString()).replace(new RegExp(RegExp("\\+", "g")), " ");
								let $BD;
								if (typeof $BB[1] === "undefined") {
									$BD = "";
								} else {
									$BD = typeof $BB[1] === "undefined" ? "undefined" : $BB[1] === null ? "null" : decodeURIComponent($BB[1].toString()).replace(new RegExp(RegExp("\\+", "g")), " ");
								}
								$BD = [$BC, $BD];
								return $BD;
							});
						}
					}
					$KEY_5 = typeof $AJ === "undefined" ? "undefined" : $AJ === null ? "null" : encodeURIComponent($AJ.toString()).replace(new RegExp(RegExp("%20", "g")), "+");
					$KEY_1 = typeof $AK === "undefined" ? "undefined" : $AK === null ? "null" : encodeURIComponent($AK.toString()).replace(new RegExp(RegExp("%20", "g")), "+");
					if ($AW.length > 0) {
						$KEY_4 = false;
						$AW.forEach(($BA, $BB) => {
							const $BC = typeof $BA[0] === "undefined" ? "undefined" : $BA[0] === null ? "null" : encodeURIComponent($BA[0].toString()).replace(new RegExp(RegExp("%20", "g")), "+");
							const $BD = typeof $BA[1] === "undefined" ? "undefined" : $BA[1] === null ? "null" : encodeURIComponent($BA[1].toString()).replace(new RegExp(RegExp("%20", "g")), "+");
							if ($BC === $KEY_5 && !$KEY_4) {
								$KEY_10 = $KEY_10 + (($BB === 0 ? $KEY_5 : "&" + $KEY_5) + "=" + $KEY_1);
								$KEY_4 = true;
							} else if ($BC !== $KEY_5) {
								$KEY_10 = $KEY_10 + (($BB === 0 ? $BC + "=" : "&" + $BC + "=") + $BD);
							}
						});
						if (!$KEY_4) {
							$KEY_10 = $KEY_10 + ("&" + $KEY_5 + "=" + $KEY_1);
						}
					} else {
						$KEY_10 = $KEY_5 + "=" + $KEY_1;
					}
					let $AZ;
					if ($AV.port.length > 0) {
						$AZ = ":" + $AV.port;
					} else {
						$AZ = "";
					}
					return $AV.protocol + "://" + $AV.hostname + $AZ + $AV.pathname + ($KEY_10.length > 0 ? "?" + $KEY_10 : "") + ($AV.hash.length > 0 ? "#" + $AV.hash : "");
				}
return $THREAD_7_118108;}main();
