let __pid0 = __pid1.persistentId.key;
						try {
							if (__pid2 != null) {
								if (__pid2.getItem(__pid0) == null) {
									let __pid3 = __pid2.getItem("f");
									if (__pid3 != null && typeof __pid3 === "string") {
										let __pid4 = {};
										let __pid5 = 0;
										for (; __pid5 < "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=".length; __pid5++) {
											__pid4["ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_="[__pid5]] = __pid5;
										}
										let __pid6 = [];
										let __pid7 = 0;
										while (__pid7 < __pid3.length) {
											if (__pid4[__pid3[__pid7 + 2]] === 64 && __pid4[__pid3[__pid7 + 3]] !== 64) {
												throw new Error("The string to be decoded is not correctly padded.");
											} else {
												__pid6.push(__pid4[__pid3[__pid7]] << 2 | __pid4[__pid3[__pid7 + 1]] >> 4);
												if (__pid4[__pid3[__pid7 + 2]] !== 64 && __pid7 + 2 < __pid3.length) {
													__pid6.push((__pid4[__pid3[__pid7 + 1]] & 15) << 4 | __pid4[__pid3[__pid7 + 2]] >> 2);
												}
												if (__pid4[__pid3[__pid7 + 3]] !== 64 && __pid7 + 3 < __pid3.length) {
													__pid6.push((__pid4[__pid3[__pid7 + 2]] & 3) << 6 | __pid4[__pid3[__pid7 + 3]]);
												}
												__pid7 += 4;
												continue;
											}
										}
										if (__pid6[5] >= 90 && __pid6[5] <= 190 && __pid6[6] === 1 && __pid6[7] === 0 && __pid6[8] === 0) {
											__pid2.removeItem("f");
											try {
												__pid8 = false;
												if (__pid2) {
													if (!__pid2.getItem(__pid0)) {
														__pid2.setItem(__pid0, __pid3);
													}
													__pid8 = true;
												}
											} catch (__pid9) {}
										}
									}
								}
							}
						} catch (__pid10) {}
						let __pid11 = __pid1.persistentId.key;
						let __pid12 = __pid1.transactionToken;
						try {
							__pid8 = false;
							if (__pid2) {
								if (typeof __pid1.persistentId.resetKey !== "undefined") {
									__pid2.removeItem(__pid1.persistentId.resetKey);
								}
								if (!__pid2.getItem(__pid11)) {
									__pid2.setItem(__pid11, __pid12);
								}
								__pid8 = true;
							}
						} catch (__pid13) {}
