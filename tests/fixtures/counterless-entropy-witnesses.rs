//                               Static counterless entropy protocol declarations.
pub const WITNESSES: &[(&str, &str)] = &[
    (
        "if-clock-random-key0-clock-key2-date-key5-crypto-key4",
        r#"function $THREAD_8_113969() {
					let $KEY_0 = Math.random() * 4294967296 >>> 0;
					let $KEY_2 = 0;
					try {
						$KEY_2 = performance.now() >>> 0;
					} catch (e) {}
					let $KEY_5 = Number(new Date()) >>> 0;
					let $KEY_4 = 0;
					try {
						$KEY_4 = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_2) {}
					$KEY_0 = $KEY_0 ^ 1103515245;
					$KEY_2 = $KEY_2;
					if ($KEY_2 === $KEY_2) {
						$KEY_0 = $KEY_0 ^ $KEY_2;
					}
					$KEY_5 = $KEY_5 === $KEY_5 ? $KEY_0 ^ $KEY_5 : $KEY_0;
					$KEY_4 = $KEY_4;
					return ($KEY_5 ^ $KEY_4) >>> 0;
				}"#,
    ),
    (
        "ternary-clock-random-key3-clock-key5-date-key4-crypto-key1",
        r#"function $THREAD_1_80340() {
					let $KEY_3 = Math.random() * 4294967296 >>> 0;
					let $KEY_5 = 0;
					try {
						$KEY_5 = performance.now() >>> 0;
					} catch (e) {}
					let $KEY_4 = Number(new Date()) >>> 0;
					let $KEY_1 = 0;
					try {
						$KEY_1 = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_2) {}
					$KEY_3 = $KEY_3 ^ 1103515245;
					$KEY_5 = $KEY_5;
					$KEY_5 = $KEY_5 === $KEY_5 ? $KEY_3 ^ $KEY_5 : $KEY_3;
					$KEY_4 = $KEY_4 === $KEY_4 ? $KEY_5 ^ $KEY_4 : $KEY_5;
					$KEY_1 = $KEY_1;
					return ($KEY_4 ^ $KEY_1) >>> 0;
				}"#,
    ),
    (
        "ternary-clock-random-key4-clock-key3-date-key0-crypto-key1",
        r#"function $THREAD_8_15067() {
					let $KEY_4 = Math.random() * 4294967296 >>> 0;
					let $KEY_3 = 0;
					try {
						$KEY_3 = performance.now() >>> 0;
					} catch (e) {}
					let $KEY_0 = Number(new Date()) >>> 0;
					let $KEY_1 = 0;
					try {
						$KEY_1 = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_2) {}
					$KEY_4 = $KEY_4 ^ 1103515245;
					$KEY_3 = $KEY_3;
					$KEY_3 = $KEY_3 === $KEY_3 ? $KEY_4 ^ $KEY_3 : $KEY_4;
					$KEY_0 = $KEY_0 === $KEY_0 ? $KEY_3 ^ $KEY_0 : $KEY_3;
					$KEY_1 = $KEY_1;
					return ($KEY_0 ^ $KEY_1) >>> 0;
				}"#,
    ),
    (
        "ternary-clock-random-key0-clock-key2-date-key1-crypto-key4",
        r#"function $THREAD_0_82887() {
					let $KEY_0 = Math.random() * 4294967296 >>> 0;
					let $KEY_2 = 0;
					try {
						$KEY_2 = performance.now() >>> 0;
					} catch (e) {}
					let $KEY_1 = Number(new Date()) >>> 0;
					let $KEY_4 = 0;
					try {
						$KEY_4 = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_2) {}
					$KEY_0 = $KEY_0 ^ 1103515245;
					$KEY_2 = $KEY_2;
					$KEY_2 = $KEY_2 === $KEY_2 ? $KEY_0 ^ $KEY_2 : $KEY_0;
					$KEY_1 = $KEY_1 === $KEY_1 ? $KEY_2 ^ $KEY_1 : $KEY_2;
					$KEY_4 = $KEY_4;
					return ($KEY_1 ^ $KEY_4) >>> 0;
				}"#,
    ),
    (
        "null-guarded-clock-reassignment",
        r#"function $THREAD_6_19852() {
					let $KEY_3 = Math.random() * 4294967296 >>> 0;
					let $KEY_1 = 0;
					try {
						$KEY_1 = performance.now() >>> 0;
					} catch (e) {}
					let $KEY_5 = Number(new Date()) >>> 0;
					let $KEY_0 = 0;
					try {
						$KEY_0 = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_2) {}
					$KEY_3 = $KEY_3 ^ 1103515245;
					$KEY_1 = $KEY_1;
					let $AK;
					if ($KEY_1 !== null) {
						$AK = $KEY_1 === $KEY_1;
					} else {
						$AK = $KEY_1;
						$KEY_1 = $KEY_5;
					}
					$AK = $AK ? $KEY_3 ^ $KEY_1 : $KEY_3;
					$KEY_5 = $KEY_5 === $KEY_5 ? $AK ^ $KEY_5 : $AK;
					$KEY_0 = $KEY_0;
					return ($KEY_5 ^ $KEY_0) >>> 0;
				}"#,
    ),
    (
        "ternary-clock-random-key5-clock-key4-date-key1-crypto-key0",
        r#"function $THREAD_6_133714() {
					let $KEY_5 = Math.random() * 4294967296 >>> 0;
					let $KEY_4 = 0;
					try {
						$KEY_4 = performance.now() >>> 0;
					} catch (e) {}
					let $KEY_1 = Number(new Date()) >>> 0;
					let $KEY_0 = 0;
					try {
						$KEY_0 = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_2) {}
					$KEY_5 = $KEY_5 ^ 1103515245;
					$KEY_4 = $KEY_4;
					$KEY_4 = $KEY_4 === $KEY_4 ? $KEY_5 ^ $KEY_4 : $KEY_5;
					$KEY_1 = $KEY_1 === $KEY_1 ? $KEY_4 ^ $KEY_1 : $KEY_4;
					$KEY_0 = $KEY_0;
					return ($KEY_1 ^ $KEY_0) >>> 0;
				}"#,
    ),
    (
        "ternary-clock-random-key0-clock-key2-date-key4-crypto-key1",
        r#"function $THREAD_6_101799() {
					let $KEY_0 = Math.random() * 4294967296 >>> 0;
					let $KEY_2 = 0;
					try {
						$KEY_2 = performance.now() >>> 0;
					} catch (e) {}
					let $KEY_4 = Number(new Date()) >>> 0;
					let $KEY_1 = 0;
					try {
						$KEY_1 = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_2) {}
					$KEY_0 = $KEY_0 ^ 1103515245;
					$KEY_2 = $KEY_2;
					$KEY_2 = $KEY_2 === $KEY_2 ? $KEY_0 ^ $KEY_2 : $KEY_0;
					$KEY_4 = $KEY_4 === $KEY_4 ? $KEY_2 ^ $KEY_4 : $KEY_2;
					$KEY_1 = $KEY_1;
					return ($KEY_4 ^ $KEY_1) >>> 0;
				}"#,
    ),
    (
        "ternary-clock-random-key4-clock-key1-date-key3-crypto-key5",
        r#"function $THREAD_0_71161() {
					let $KEY_4 = Math.random() * 4294967296 >>> 0;
					let $KEY_1 = 0;
					try {
						$KEY_1 = performance.now() >>> 0;
					} catch (e) {}
					let $KEY_3 = Number(new Date()) >>> 0;
					let $KEY_5 = 0;
					try {
						$KEY_5 = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_2) {}
					$KEY_4 = $KEY_4 ^ 1103515245;
					$KEY_1 = $KEY_1;
					$KEY_1 = $KEY_1 === $KEY_1 ? $KEY_4 ^ $KEY_1 : $KEY_4;
					$KEY_3 = $KEY_3 === $KEY_3 ? $KEY_1 ^ $KEY_3 : $KEY_1;
					$KEY_5 = $KEY_5;
					return ($KEY_3 ^ $KEY_5) >>> 0;
				}"#,
    ),
    (
        "ternary-clock-random-key1-clock-key4-date-key3-crypto-key0",
        r#"function $THREAD_3_50856() {
					let $KEY_1 = Math.random() * 4294967296 >>> 0;
					let $KEY_4 = 0;
					try {
						$KEY_4 = performance.now() >>> 0;
					} catch (e) {}
					let $KEY_3 = Number(new Date()) >>> 0;
					let $KEY_0 = 0;
					try {
						$KEY_0 = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_2) {}
					$KEY_1 = $KEY_1 ^ 1103515245;
					$KEY_4 = $KEY_4;
					$KEY_4 = $KEY_4 === $KEY_4 ? $KEY_1 ^ $KEY_4 : $KEY_1;
					$KEY_3 = $KEY_3 === $KEY_3 ? $KEY_4 ^ $KEY_3 : $KEY_4;
					$KEY_0 = $KEY_0;
					return ($KEY_3 ^ $KEY_0) >>> 0;
				}"#,
    ),
    (
        "ternary-clock-random-key4-clock-key1-date-key0-crypto-key2",
        r#"function $THREAD_7_27119() {
					let $KEY_4 = Math.random() * 4294967296 >>> 0;
					let $KEY_1 = 0;
					try {
						$KEY_1 = performance.now() >>> 0;
					} catch (e) {}
					let $KEY_0 = Number(new Date()) >>> 0;
					let $KEY_2 = 0;
					try {
						$KEY_2 = crypto.getRandomValues(new Uint32Array(1))[0];
					} catch (e_2) {}
					$KEY_4 = $KEY_4 ^ 1103515245;
					$KEY_1 = $KEY_1;
					$KEY_1 = $KEY_1 === $KEY_1 ? $KEY_4 ^ $KEY_1 : $KEY_4;
					$KEY_0 = $KEY_0 === $KEY_0 ? $KEY_1 ^ $KEY_0 : $KEY_1;
					$KEY_2 = $KEY_2;
					return ($KEY_0 ^ $KEY_2) >>> 0;
				}"#,
    ),
];
