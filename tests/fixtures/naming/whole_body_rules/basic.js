function $THREAD_0_900000() {
let $KEY_0, $KEY_2, $KEY_5, $KEY_8, $KEY_9, $KEY_12, $KEY_14, $KEY_19, $KEY_21, $KEY_32, $KEY_77, $KEY_115, $KEY_125, $KEY_208, $KEY_241, $KEY_1250, $KEY_1260;

function $THREAD_9_900001() {
	$KEY_32();
}

function $THREAD_9_900002($HC) {
	return $KEY_0($HC, $KEY_5);
}

function $THREAD_9_900003($GW) {
	$KEY_8.push($GW);
}

function $THREAD_9_900004($GW) {
	try {
		$GW();
	} catch (e) {
		let $KEY_0 = e;
		$KEY_8.forEach($GX => $GX.error($KEY_0));
	}
}

function $THREAD_9_900005() {
	$KEY_12 = $KEY_208;
	$KEY_14 = $KEY_208;
	if ($KEY_2 != null) {
		$KEY_2();
	}
}

function $THREAD_9_900006() {
	throw new TypeError("Cannot read properties of undefined (reading 'value')");
}

function $THREAD_9_900007($GP) {
	if (typeof $GP === "function" && $GP.toString === Function.prototype.toString) {
		return !!$KEY_125.exec($GP.toString());
	} else {
		return false;
	}
}

function $THREAD_9_900008() {
	if ($KEY_115 != null) {
		$KEY_115.terminate();
	}
	$KEY_77.complete();
}

function $THREAD_9_900009($CA) {
	try {
		++$KEY_2;
		let $CB = $KEY_19;
		let $CC = $KEY_19.put;
		$CC.call($CB, {
			visible: $CA.target.visibilityState === "visible",
			timestamp: $KEY_9()
		});
	} catch (e) {
		$KEY_21.error(e);
	}
}

function $THREAD_9_900010() {
	let $KEY_0;
	function $THREAD_8_70754() {
		"use strict";
		if (this instanceof $KEY_0) {
			this.extensions = Object.create(null);
			return void 0;
		} else {
			throw new TypeError("Cannot call a class as a function");
		}
	}
	$KEY_0 = $THREAD_8_70754;
	let $AW = { key: "extend" };
	function $THREAD_9_14073($AX, $AY, $AZ, $BA) {
		this.extensions[$AX] = {
			detector: $AY,
			serialiser: $AZ,
			deserialiser: $BA
		};
	}
	$AW.value = $THREAD_9_14073;
	$AW = [$AW];
	$KEY_241($THREAD_8_70754, $AW);
	return $KEY_0;
}

function $THREAD_9_900011($GP) {
	"use strict";
	this.targetBatchTimeMs = $GP;
	this._internalNow = $KEY_1260(typeof global !== "undefined" && global != null && global.global === global ? global : void 0);
}
}

$THREAD_0_900000();
