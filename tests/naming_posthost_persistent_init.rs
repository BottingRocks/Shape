use shape::naming::name_threads;

// Posthost family: persistent transaction-token storage initialization.
// The owned zero-formal body guards one captured storage provider, writes the
// transaction token under `config.persistentId.key` (or `.resetKey`) when it
// is absent, then writes a generated timestamp/entropy string under a prefixed
// persistent key when that key is absent. The nested collector callback is a
// different owned entity and is deliberately not entered by this evaluator.
const EVALUATOR: &str = "posthost_persistent_init";
const NAME: &str = "initializePersistentTransactionState";

const PERSISTENT_INIT: &str = r#"function main(){
let config = {persistentId:{key:"k"},transactionToken:"t"};
let storage = null;
let ready = false;
let seedKey = null;
let seedValue = null;
let counter = 0;
let collectors = [];
let result = null;
let applyToken = function(value) { return value; };
let notifyReady = function() {};
let createRecord = function() { return {}; };
let deferQueue = null;
let microTasks = [];
let report = function(record) {};
function $THREAD_0_69045() {
    let key = config.persistentId.key;
    let token = config.transactionToken;
    try {
        ready = false;
        if (storage) {
            if (!storage.getItem(key)) {
                storage.setItem(key, token);
            }
            ready = true;
        }
    } catch (e) {}
    try {
        if (storage) {
            seedKey = "l_" + config.persistentId.key;
            let stamp = Date.now();
            let random = Math.random() * 4294967296 >>> 0;
            let perf = 0;
            try {
                perf = performance.now() >>> 0;
            } catch (e2) {}
            let now = Number(new Date()) >>> 0;
            let cryptoValue = 0;
            try {
                cryptoValue = crypto.getRandomValues(new Uint32Array(1))[0];
            } catch (e3) {}
            random = random ^ 1103515245;
            perf = perf === perf ? random ^ perf : random;
            now = now === now ? perf ^ now : perf;
            ++counter;
            seedValue = "" + stamp + ((now ^ cryptoValue ^ counter) >>> 0);
            if (!storage.getItem(seedKey)) {
                storage.setItem(seedKey, seedValue);
            }
        }
    } catch (e4) {}
    result = applyToken(config.transactionToken);
    notifyReady();
    try {
        collectors.forEach(item => {
            item.init(createRecord());
            if (item.afterReady != null) {
                if (typeof setTimeout !== "function") {
                    item.afterReady.call(void 0);
                } else if (deferQueue) {
                    setTimeout(item.afterReady, 0);
                } else if (microTasks) {
                    microTasks.push(item.afterReady);
                }
            }
        });
    } catch (e5) {
        let record = {};
        record.message = "3.1:" + e5;
        report(record);
    }
}
return $THREAD_0_69045;
}
main();"#;

// The same family with opaque generated spellings and `var`/`const` capture
// declarations. Protocol member names stay readable by design.
const PERSISTENT_INIT_SPELLED: &str = r#"function main(){
const $KEY_13 = {persistentId:{key:"k"},transactionToken:"t"};
var $KEY_376 = null;
let $KEY_146 = false;
let $KEY_12 = null;
let $KEY_99 = null;
let $KEY_400 = 0;
let $KEY_7 = [];
let $KEY_232 = null;
let $KEY_359 = function($BH) { return $BH; };
let $KEY_242 = function() {};
let $KEY_66 = function() { return {}; };
let $KEY_270 = null;
let $KEY_335 = [];
let $KEY_398 = function($BH) {};
function $THREAD_900_777_2() {
    let $BY = $KEY_13.persistentId.key;
    let $BZ = $KEY_13.transactionToken;
    try {
        $KEY_146 = false;
        if ($KEY_376) {
            if (!$KEY_376.getItem($BY)) {
                $KEY_376.setItem($BY, $BZ);
            }
            $KEY_146 = true;
        }
    } catch (e) {}
    try {
        if ($KEY_376) {
            $KEY_12 = "l_" + $KEY_13.persistentId.key;
            let $CA = Date.now();
            let $CB = Math.random() * 4294967296 >>> 0;
            let $CC = 0;
            try {
                $CC = performance.now() >>> 0;
            } catch (e_2) {}
            let $CD = Number(new Date()) >>> 0;
            let $CE = 0;
            try {
                $CE = crypto.getRandomValues(new Uint32Array(1))[0];
            } catch (e_3) {}
            $CB = $CB ^ 1103515245;
            $CC = $CC === $CC ? $CB ^ $CC : $CB;
            $CD = $CD === $CD ? $CC ^ $CD : $CC;
            ++$KEY_400;
            $KEY_99 = "" + $CA + (($CD ^ $CE ^ $KEY_400) >>> 0);
            if (!$KEY_376.getItem($KEY_12)) {
                $KEY_376.setItem($KEY_12, $KEY_99);
            }
        }
    } catch (e_4) {}
    $KEY_232 = $KEY_359($KEY_13.transactionToken);
    $KEY_242();
    try {
        $KEY_7.forEach($CG => {
            $CG.init($KEY_66());
            if ($CG.afterReady != null) {
                if (typeof setTimeout !== "function") {
                    $CG.afterReady.call(void 0);
                } else if ($KEY_270) {
                    setTimeout($CG.afterReady, 0);
                } else if ($KEY_335) {
                    $KEY_335.push($CG.afterReady);
                }
            }
        });
    } catch (e_5) {
        let $CF = {};
        $CF.message = "3.1:" + e_5;
        $KEY_398($CF);
    }
}
return $THREAD_900_777_2;
}
main();"#;

// A complete family body inside a nested function: the outer generated
// function must not borrow it, while the independently owned nested body is
// still claimable on its own.
const NESTED_DECOY: &str = r#"function main(){
let config = {persistentId:{key:"k"},transactionToken:"t"};
let storage = null;
let ready = false;
let seedKey = null;
let seedValue = null;
let counter = 0;
function $THREAD_7_77() {
    function decoy() {
        let key = config.persistentId.key;
        let token = config.transactionToken;
        try {
            ready = false;
            if (storage) {
                if (!storage.getItem(key)) {
                    storage.setItem(key, token);
                }
                ready = true;
            }
        } catch (e) {}
        try {
            if (storage) {
                seedKey = "l_" + config.persistentId.key;
                let stamp = Date.now();
                let random = Math.random() * 4294967296 >>> 0;
                let perf = performance.now() >>> 0;
                let now = Number(new Date()) >>> 0;
                let cryptoValue = crypto.getRandomValues(new Uint32Array(1))[0];
                random = random ^ 1103515245;
                perf = perf === perf ? random ^ perf : random;
                now = now === now ? perf ^ now : perf;
                ++counter;
                seedValue = "" + stamp + ((now ^ cryptoValue ^ counter) >>> 0);
                if (!storage.getItem(seedKey)) {
                    storage.setItem(seedKey, seedValue);
                }
            }
        } catch (e4) {}
    }
}
return $THREAD_7_77;
}
main();"#;

const STORAGE_EVENT_LISTENER: &str = r#"function main(){
let config = {persistentId:{key:"k"},transactionToken:"t"};
let storage = null;
function $THREAD_8_88() {
    storage.addEventListener("storage", event => {
        event.key;
    });
    let key = config.persistentId.key;
    let token = config.transactionToken;
}
return $THREAD_8_88;
}
main();"#;

const COOKIE_PROVIDER: &str = r#"function main(){
let config = {persistentId:{key:"k"},transactionToken:"t"};
function $THREAD_9_99() {
    let key = config.persistentId.key;
    let token = config.transactionToken;
    document.cookie = key + "=" + token;
}
return $THREAD_9_99;
}
main();"#;

const GENERAL_PAYLOAD_COLLECTION: &str = r#"function main(){
let storage = null;
function $THREAD_5_55() {
    if (storage) {
        let key = "payload";
        if (!storage.getItem(key)) {
            storage.setItem(key, loadPayload());
        }
    }
}
return $THREAD_5_55;
}
main();"#;

// The initializer body is retained byte-for-byte, including the legacy `f` migration
// decoder and registry/afterReady tail. Surrounding declarations preserve provider and
// helper identities.
const MIGRATION_INIT: &str = r#"function main(){
let $KEY_0 = {persistentId:{key:"k",resetKey:"r"},transactionToken:"t"};
let $KEY_12 = null;
let $KEY_140 = false;
let $KEY_110 = null;
let $KEY_162 = null;
let $KEY_130 = null;
let $KEY_17 = [];
let $KEY_227 = function() {};
let $KEY_341 = function(value) { return value; };
let $KEY_103 = [];
let $KEY_346 = false;
let $KEY_434 = function() {};
let $KEY_370 = 0;
let $KEY_73 = [];
let $KEY_146 = function(callback) { return callback; };
function $THREAD_5_69705() {
	let $KEY_3 = Math.random() * 4294967296 >>> 0;
	let $KEY_2 = 0;
	try {
		$KEY_2 = performance.now() >>> 0;
	} catch (e) {}
	let $KEY_1 = Number(new Date()) >>> 0;
	let $KEY_0 = 0;
	try {
		$KEY_0 = crypto.getRandomValues(new Uint32Array(1))[0];
	} catch (e_2) {}
	$KEY_3 = $KEY_3 ^ 1103515245;
	$KEY_2 = $KEY_2;
	$KEY_2 = $KEY_2 === $KEY_2 ? $KEY_3 ^ $KEY_2 : $KEY_3;
	$KEY_1 = $KEY_1 === $KEY_1 ? $KEY_2 ^ $KEY_1 : $KEY_2;
	$KEY_0 = $KEY_0;
	++$KEY_370;
	return ($KEY_1 ^ $KEY_0 ^ $KEY_370) >>> 0;
}
let $KEY_177 = $THREAD_5_69705;
function $THREAD_6_133968() {
				let $CJ = $KEY_0.persistentId.key;
				try {
					if ($KEY_12 != null) {
						if ($KEY_12.getItem($CJ) == null) {
							let $CK = $KEY_12.getItem("f");
							if ($CK != null && typeof $CK === "string") {
								let $CL = {};
								let $CM = 0;
								while ($CM < "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=".length) {
									$CL["ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_="[$CM]] = $CM;
									++$CM;
								}
								let $CN = [];
								let $CO = 0;
								while ($CO < $CK.length) {
									if ($CL[$CK[$CO + 2]] === 64 && $CL[$CK[$CO + 3]] !== 64) {
										throw new Error("The string to be decoded is not correctly padded.");
									} else {
										$CN.push($CL[$CK[$CO]] << 2 | $CL[$CK[$CO + 1]] >> 4);
										if ($CL[$CK[$CO + 2]] !== 64 && $CO + 2 < $CK.length) {
											$CN.push(($CL[$CK[$CO + 1]] & 15) << 4 | $CL[$CK[$CO + 2]] >> 2);
										}
										if ($CL[$CK[$CO + 3]] !== 64 && $CO + 3 < $CK.length) {
											$CN.push(($CL[$CK[$CO + 2]] & 3) << 6 | $CL[$CK[$CO + 3]]);
										}
										$CO += 4;
										continue;
									}
								}
								if ($CN[5] >= 90 && $CN[5] <= 190 && $CN[6] === 1 && $CN[7] === 0 && $CN[8] === 0) {
									$KEY_12.removeItem("f");
									try {
										$KEY_140 = false;
										if ($KEY_12) {
											if (!$KEY_12.getItem($CJ)) {
												$KEY_12.setItem($CJ, $CK);
											}
											$KEY_140 = true;
										}
									} catch (e) {}
								}
							}
						}
					}
				} catch (e_2) {}
				let $CP = $KEY_0.persistentId.key;
				let $CQ = $KEY_0.transactionToken;
				try {
					$KEY_140 = false;
					if ($KEY_12) {
						if (typeof $KEY_0.persistentId.resetKey !== "undefined") {
							$KEY_12.removeItem($KEY_0.persistentId.resetKey);
						}
						if (!$KEY_12.getItem($CP)) {
							$KEY_12.setItem($CP, $CQ);
						}
						$KEY_140 = true;
					}
				} catch (e_3) {}
				try {
					if ($KEY_12) {
						$KEY_110 = "l_" + $KEY_0.persistentId.key;
						$KEY_162 = "" + Date.now() + $KEY_177();
						if (!$KEY_12.getItem($KEY_110)) {
							$KEY_12.setItem($KEY_110, $KEY_162);
						}
					}
				} catch (e_4) {}
				$KEY_130 = $KEY_341($KEY_0.transactionToken);
				$KEY_227();
				try {
					$KEY_17.forEach($CS => {
						$CS.init(new $KEY_146($CT => {
							let $KEY_1 = $CT;
							$KEY_73.push($CU => $KEY_1.next($CU));
						}));
						if ($CS.afterReady != null) {
							if (typeof setTimeout !== "function") {
								$CS.afterReady.call(void 0);
							} else if ($KEY_346) {
								setTimeout($CS.afterReady, 0);
							} else if ($KEY_103) {
								$KEY_103.push($CS.afterReady);
							}
						}
					});
				} catch (e_5) {
					let $CR = {};
					$CR.message = "3.1:" + e_5;
					$KEY_434($CR);
				}
			}
return $THREAD_6_133968;
}
main();"#;

fn named(source: &str, expected: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result
            .report
            .outcomes
            .iter()
            .any(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(expected))
    })
}

fn our_claim(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.function_roles.iter().any(|role| {
            role.evaluators
                .iter()
                .any(|evaluator| evaluator == EVALUATOR)
        })
    })
}

fn parses(source: &str) -> bool {
    name_threads(source, true).is_ok()
}

fn denied(source: &str) {
    assert!(parses(source), "fixture did not parse: {source}");
    assert!(!named(source, NAME), "unexpected {NAME} for: {source}");
    assert!(
        !our_claim(source),
        "unexpected persistent-init claim for: {source}"
    );
}

#[test]
fn persistent_init_accepts_migration_witness_and_captured_helper() {
    let body_start = MIGRATION_INIT
        .find("function $THREAD_6_133968() {")
        .expect("persistent migration function");
    let body_end = MIGRATION_INIT[body_start..]
        .find("\nreturn $THREAD_6_133968;")
        .map(|offset| body_start + offset)
        .expect("persistent migration function end");
    assert_eq!(
        MIGRATION_INIT[body_start..body_end].as_bytes().len(),
        2_830,
        "the persistent migration witness body must remain exact"
    );
    assert!(MIGRATION_INIT.contains("getItem(\"f\")"));
    assert!(MIGRATION_INIT.contains("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_="));
    assert!(MIGRATION_INIT.contains("afterReady"));
    assert!(named(MIGRATION_INIT, NAME), "persistent migration witness was not named");
    assert!(our_claim(MIGRATION_INIT), "persistent migration witness was not claimed by this family");
}

#[test]
fn persistent_init_migration_variant_negatives_do_not_borrow_decoys() {
    assert!(
        named(MIGRATION_INIT, NAME),
        "the captured-helper positive must be accepted before its refusals"
    );
    assert!(
        our_claim(MIGRATION_INIT),
        "the captured-helper positive must claim the family before its refusals"
    );
    let wrong_storage = MIGRATION_INIT
        .replace("let $KEY_12 = null;", "let $KEY_12 = null;\nlet $OTHER_STORAGE = null;")
        .replace(
            "if ($KEY_12) {\n\t\t\t\t\t\t$KEY_110 = \"l_\"",
            "if ($OTHER_STORAGE) {\n\t\t\t\t\t\t$KEY_110 = \"l_\"",
        )
        .replace(
            "$KEY_12.getItem($KEY_110)",
            "$OTHER_STORAGE.getItem($KEY_110)",
        )
        .replace(
            "$KEY_12.setItem($KEY_110, $KEY_162)",
            "$OTHER_STORAGE.setItem($KEY_110, $KEY_162)",
        );
    let decoder_without_token = MIGRATION_INIT.replace(
        "$KEY_12.setItem($CP, $CQ)",
        "$KEY_12.setItem($CP, $CK)",
    );
    let wrong_input = MIGRATION_INIT.replace(
        "$KEY_0.transactionToken",
        "$KEY_0.transactionId",
    );
    let wrong_state = MIGRATION_INIT.replace(
        "$KEY_12.setItem($CP, $CQ)",
        "$KEY_12.setItem($CQ, $CP)",
    );
    let wrong_effect = MIGRATION_INIT.replace(
        "$KEY_12.setItem($KEY_110, $KEY_162)",
        "$KEY_12.removeItem($KEY_110)",
    );
    let mutable_helper = MIGRATION_INIT.replace(
        "let $KEY_177 = $THREAD_5_69705;",
        "let $KEY_177 = $THREAD_5_69705;\n$KEY_177 = $THREAD_5_69705;",
    );
    let locally_defined_helper = MIGRATION_INIT.replace(
        "function $THREAD_6_133968() {\n",
        "function $THREAD_6_133968() {\n\t\t\t\tlet $KEY_177 = function() {\n\t\t\t\t\tlet $random = Math.random();\n\t\t\t\t\tlet $perf = performance.now();\n\t\t\t\t\tlet $stamp = Number(new Date());\n\t\t\t\t\t++$KEY_370;\n\t\t\t\t\treturn ($random ^ $perf ^ $stamp ^ $KEY_370) >>> 0;\n\t\t\t\t};\n",
    );
    let nested_donor = MIGRATION_INIT
        .replace(
            "function $THREAD_6_133968() {\n",
            "function $THREAD_6_133968() {\n\t\t\t\tfunction donor() {\n\t\t\t\t\tlet random = Math.random();\n\t\t\t\t\tlet perf = performance.now();\n\t\t\t\t\tlet stamp = Number(new Date());\n\t\t\t\t\t++$KEY_370;\n\t\t\t\t\treturn (random ^ perf ^ stamp ^ $KEY_370) >>> 0;\n\t\t\t\t}\n",
        )
        .replace("$KEY_177()", "donor()");
    let wrong_callee = MIGRATION_INIT.replace("$KEY_177()", "$KEY_177.call(null)");
    let wrong_return = MIGRATION_INIT.replace(
        "return ($KEY_1 ^ $KEY_0 ^ $KEY_370) >>> 0;",
        "return $KEY_1;",
    );
    let wrong_xor = MIGRATION_INIT.replace(
        "return ($KEY_1 ^ $KEY_0 ^ $KEY_370) >>> 0;",
        "return ($KEY_1 + $KEY_0 + $KEY_370) >>> 0;",
    );
    let wrong_time_source =
        MIGRATION_INIT.replace("let $KEY_1 = Number(new Date()) >>> 0;", "let $KEY_1 = 0;");
    for source in [
        wrong_storage,
        decoder_without_token,
        wrong_input,
        wrong_state,
        wrong_effect,
        mutable_helper,
        locally_defined_helper,
        nested_donor,
        wrong_callee,
        wrong_return,
        wrong_xor,
        wrong_time_source,
    ] {
        denied(&source);
    }
}

/// Swap the token-try and seed-try blocks, leaving the collector try in place.
fn reorder_try_blocks(source: &str) -> String {
    let first = source
        .find("    try {\n        ready = false;")
        .expect("first try block");
    let second = source
        .find("    try {\n        if (storage) {\n            seedKey")
        .expect("second try block");
    let third = source
        .find("    try {\n        collectors.forEach")
        .expect("third try block");
    format!(
        "{}{}{}{}",
        &source[..first],
        &source[second..third],
        &source[first..second],
        &source[third..]
    )
}

#[test]
fn persistent_init_accepts_portable_spellings() {
    for source in [
        PERSISTENT_INIT.to_string(),
        PERSISTENT_INIT_SPELLED.to_string(),
    ] {
        assert!(named(&source, NAME), "not named: {source}");
        assert!(
            our_claim(&source),
            "not the persistent-init claim: {source}"
        );
    }
}

#[test]
fn persistent_init_accepts_source_node_shapes() {
    let computed = PERSISTENT_INIT
        .replace(
            "config.persistentId.key",
            "config[\"persistentId\"][\"key\"]",
        )
        .replace("config.transactionToken", "config[\"transactionToken\"]")
        .replace("storage.getItem(key)", "storage[\"getItem\"](key)")
        .replace(
            "storage.setItem(key, token)",
            "storage[\"setItem\"](key, token)",
        )
        .replace("storage.getItem(seedKey)", "storage[\"getItem\"](seedKey)")
        .replace(
            "storage.setItem(seedKey, seedValue)",
            "storage[\"setItem\"](seedKey, seedValue)",
        );
    let unbraced = PERSISTENT_INIT.replace(
        "            if (!storage.getItem(key)) {\n                storage.setItem(key, token);\n            }",
        "            if (!storage.getItem(key)) storage.setItem(key, token);",
    );
    let nullish = PERSISTENT_INIT
        .replace(
            "if (!storage.getItem(key))",
            "if (storage.getItem(key) == null)",
        )
        .replace(
            "if (!storage.getItem(seedKey))",
            "if (storage.getItem(seedKey) == null)",
        );
    let declarations = PERSISTENT_INIT
        .replace("let config =", "const config =")
        .replace("let storage =", "var storage =")
        .replace("let ready =", "var ready =")
        .replace("let seedKey =", "var seedKey =")
        .replace("let seedValue =", "var seedValue =")
        .replace("let counter =", "var counter =")
        .replace("let collectors =", "const collectors =");
    // The generated value reaches setItem through one more single-origin alias.
    let aliased = PERSISTENT_INIT.replace(
        "seedValue = \"\" + stamp + ((now ^ cryptoValue ^ counter) >>> 0);",
        "let generated = \"\" + stamp + ((now ^ cryptoValue ^ counter) >>> 0);\n            seedValue = generated;",
    );
    let reordered = reorder_try_blocks(PERSISTENT_INIT);
    for source in [
        computed,
        unbraced,
        nullish,
        declarations,
        aliased,
        reordered,
    ] {
        assert!(named(&source, NAME), "not named: {source}");
        assert!(
            our_claim(&source),
            "not the persistent-init claim: {source}"
        );
    }
}

#[test]
fn persistent_init_refuses_changed_operations() {
    for (from, to) in [
        // The direct pair stops being a get-or-write pair.
        ("storage.getItem(key)", "storage.removeItem(key)"),
        ("storage.setItem(key, token)", "storage.getItem(key, token)"),
        // The presence guard is no longer a missing-value test.
        ("!storage.getItem(key)", "storage.getItem(key)"),
        ("!storage.getItem(seedKey)", "storage.getItem(seedKey)"),
        // The transaction token must be the value under the direct key.
        ("storage.setItem(key, token)", "storage.setItem(token, key)"),
        // The persistent member identity is exact.
        ("config.persistentId.key", "config.persistentId.token"),
        ("config.transactionToken", "config.transactionId"),
        // The second key must be a prefixed persistent key.
        (
            "\"l_\" + config.persistentId.key",
            "config.persistentId.key",
        ),
        // The seed must keep the string/XOR/unsigned-shift generation.
        (
            "\"\" + stamp + ((now ^ cryptoValue ^ counter) >>> 0)",
            "stamp + 1",
        ),
        // The entropy protocol is not optional.
        ("Math.random() * 4294967296 >>> 0", "0 * 4294967296 >>> 0"),
    ] {
        denied(&PERSISTENT_INIT.replace(from, to));
    }
}

#[test]
fn persistent_init_requires_the_storage_presence_guard() {
    denied(&PERSISTENT_INIT.replace("if (storage) {", "if (ready) {"));
}

#[test]
fn persistent_init_refuses_shadowed_providers() {
    for source in [
        // The storage provider is redeclared inside the owned body.
        PERSISTENT_INIT.replace(
            "function $THREAD_0_69045() {\n",
            "function $THREAD_0_69045() {\n    let storage = null;\n",
        ),
        // The configuration provider is redeclared inside the owned body.
        PERSISTENT_INIT.replace(
            "function $THREAD_0_69045() {\n",
            "function $THREAD_0_69045() {\n    let config = {persistentId:{key:\"x\"},transactionToken:\"y\"};\n",
        ),
    ] {
        denied(&source);
    }
}

#[test]
fn persistent_init_refuses_disconnected_decoy_entropy() {
    for source in [
        // Date.now stays in the guard but no longer reaches the stored value.
        PERSISTENT_INIT.replace("let stamp = Date.now();", "let stamp = 0;\n            Date.now();"),
        // Math.random stays in the guard but is not part of the XOR mix.
        PERSISTENT_INIT.replace(
            "let random = Math.random() * 4294967296 >>> 0;",
            "let random = 0;\n            Math.random() * 4294967296 >>> 0;",
        ),
        // Both second-entropy calls stay in the guard but are disconnected.
        PERSISTENT_INIT
            .replace("perf = performance.now() >>> 0;", "performance.now() >>> 0;")
            .replace(
                "cryptoValue = crypto.getRandomValues(new Uint32Array(1))[0];",
                "crypto.getRandomValues(new Uint32Array(1))[0];",
            ),
        // The counter is read by the mix but never incremented.
        PERSISTENT_INIT.replace("++counter;", "counter;"),
        // Entropy written from a nested callback does not flow to the value.
        PERSISTENT_INIT.replace(
            "let stamp = Date.now();",
            "let stamp = 0;\n            const readStamp = () => { stamp = Date.now(); };\n            readStamp;",
        ),
    ] {
        denied(&source);
    }
}

#[test]
fn persistent_init_refuses_shadowed_entropy_globals() {
    for source in [
        // A local object standing in for Math must not donate randomness.
        PERSISTENT_INIT.replace(
            "function $THREAD_0_69045() {\n",
            "function $THREAD_0_69045() {\n    let Math = { random: function() { return 0; } };\n",
        ),
        // A local object standing in for Date must not donate timestamps.
        PERSISTENT_INIT.replace(
            "function $THREAD_0_69045() {\n",
            "function $THREAD_0_69045() {\n    let Date = { now: function() { return 0; } };\n",
        ),
        // Local stand-ins for the second-entropy globals must not count.
        PERSISTENT_INIT.replace(
            "function $THREAD_0_69045() {\n",
            "function $THREAD_0_69045() {\n    let performance = { now: function() { return 0; } };\n    let crypto = { getRandomValues: function() { return [0]; } };\n",
        ),
    ] {
        denied(&source);
    }
}

#[test]
fn persistent_init_refuses_receiver_mismatch() {
    let source = PERSISTENT_INIT
        .replace(
            "let storage = null;",
            "let storage = null;\nlet otherStorage = null;",
        )
        .replace(
            "    try {\n        if (storage) {\n            seedKey",
            "    try {\n        if (otherStorage) {\n            seedKey",
        )
        .replace("storage.getItem(seedKey)", "otherStorage.getItem(seedKey)")
        .replace(
            "storage.setItem(seedKey, seedValue)",
            "otherStorage.setItem(seedKey, seedValue)",
        );
    denied(&source);
}

#[test]
fn nested_decoy_does_not_supply_outer_evidence() {
    let result = name_threads(NESTED_DECOY, true).unwrap();
    let outer = result
        .report
        .function_roles
        .iter()
        .find(|role| role.current_name.as_deref() == Some("$THREAD_7_77"))
        .expect("the outer generated function must exist");
    assert!(
        !outer
            .evaluators
            .iter()
            .any(|evaluator| evaluator == EVALUATOR),
        "the outer function borrowed nested evidence"
    );
    assert!(!result.report.outcomes.iter().any(|outcome| {
        outcome.binding.name == "$THREAD_7_77"
            && outcome.applied
            && outcome.proposed_name.as_deref() == Some(NAME)
    }));

    let decoy = result
        .report
        .function_roles
        .iter()
        .find(|role| role.current_name.as_deref() == Some("decoy"))
        .expect("the nested decoy must exist");
    assert!(
        decoy
            .evaluators
            .iter()
            .any(|evaluator| evaluator == EVALUATOR),
        "the independently owned decoy body should be claimable"
    );
}

#[test]
fn persistent_init_excludes_adjacent_storage_families() {
    for source in [
        STORAGE_EVENT_LISTENER,
        COOKIE_PROVIDER,
        GENERAL_PAYLOAD_COLLECTION,
    ] {
        denied(source);
    }
}

#[test]
fn persistent_init_preserves_human_names_and_collision_guards() {
    let human = PERSISTENT_INIT.replace("$THREAD_0_69045", NAME);
    let result = name_threads(&human, true).unwrap();
    assert!(result.source.contains(&format!("function {NAME}(")));
    assert!(!result
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.binding.name == NAME && outcome.applied));

    for collision in [
        PERSISTENT_INIT.replace(
            "function main(){",
            "function main(){let initializePersistentTransactionState=17;useValue(initializePersistentTransactionState);",
        ),
        PERSISTENT_INIT.replace(
            "function main(){",
            "function main(){function initializePersistentTransactionState(){}useValue(initializePersistentTransactionState);",
        ),
    ] {
        assert!(parses(&collision), "{collision}");
        assert!(!named(&collision, NAME), "{collision}");
    }
}

#[test]
fn persistent_init_edits_are_identifier_only_replacements() {
    let result = name_threads(PERSISTENT_INIT, true).unwrap();
    let applied: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(NAME))
        .collect();
    assert_eq!(applied.len(), 1, "expected exactly one applied target");
    let mut replay = PERSISTENT_INIT.to_string();
    for edit in result.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}
