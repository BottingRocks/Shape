function main(){let captured143=true;
function $THREAD_11_110($AP, $AQ, $AR, $AS) {
			let captured3 = $AS;
			let captured4 = $AR;
			let captured5, captured7, $AT;
			if ($AP.byteLength % 4 === 0) {
				$AT = $AP.buffer;
			} else {
				$AT = new ArrayBuffer($AP.byteLength + (4 - $AP.byteLength % 4));
				new Int8Array($AT).set(new Int8Array($AP.buffer));
			}
			captured7 = new Int32Array($AT);
			captured5 = captured7.length;
			let captured2 = 0;
			let captured1 = 0;
			new captured92($AQ).schedule(() => {
				try {
					let $AU = captured2 + 400 < captured5 ? captured2 + 400 : captured5;
					while (captured2 < $AU) {
						captured1 = (captured1 << 5) - captured1 + captured7[captured2] | 0;
						++captured2;
					}
					if (captured2 < captured5) {
						return true;
					} else {
						captured4(captured1);
						return false;
					}
				} catch (e) {
					captured3(e);
					return false;
				}
			});
		}
function $THREAD_12_120($AP, $AQ, $AR, $AS) {
			let captured0 = $AR;
			let captured1, captured3, captured4;
			let captured5 = $AS;
			let captured6 = $AP;
			captured3 = $AP.length >> 1 << 1;
			captured4 = 0;
			captured1 = 0;
			new captured92($AQ).schedule(() => {
				try {
					let $AT = captured4 + 400 < captured3 ? captured4 + 400 : captured3;
					for (; captured4 < $AT; captured4 = captured4 + 2) {
						captured1 = (captured1 << 5) - captured1 + (captured396(captured6[captured4]) << 16 | captured396(captured6[captured4 + 1])) | 0;
					}
					if (captured4 < captured3) {
						return true;
					} else {
						if (captured3 !== captured6.length) {
							captured1 = (captured1 << 5) - captured1 + (captured396(captured6[captured3]) << 16) | 0;
						}
						captured0(captured1);
						return false;
					}
				} catch (e) {
					captured5(e);
					return false;
				}
			});
		}
function $THREAD_13_130($AP, $AQ, $AR, $AS) {
			if (typeof $AP === "string") {
				captured389($AP, $AQ, $AR, $AS);
			} else {
				let $AT = captured143;
				if ($AT) {
					$AT = ArrayBuffer.isView($AP);
				}
				if ($AT) {
					captured301($AP, $AQ, $AR, $AS);
				} else {
					$AS(new TypeError());
				}
			}
		}
let captured389=$THREAD_12_120;let captured301=$THREAD_11_110;return $THREAD_13_130;}main();
