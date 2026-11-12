use sha2::{Digest, Sha256};
use shape::naming::name_threads;

const ROLE: &str = "buildBeaconDescriptorPayloads";
const CANONICAL_REPORT_ROLE: &str = "buildBeaconReports";

// The descriptor-payload mapper body is embedded byte-for-byte. Its wrapper supplies the
// captured record list, encoder helpers, dynamic-key global installation, and
// initialization/teardown frame.
const DESCRIPTOR_PAYLOAD_BODY: &str = r#"function $THREAD_8_12645($AT) {
						let $KEY_2 = $AT;
						let $AU;
						if ($AT == null) {
							$AU = $KEY_260;
						} else {
							$AU = $KEY_260.filter($AV => $AV.a === $KEY_2);
						}
						return $AU.map($AV => {
							let $KEY_5 = $KEY_413($KEY_0, $AV.c.map($AY => $KEY_332[$AY]), $KEY_4, $KEY_8);
							let $AW = {
								beaconEndpoint: $AV.a,
								beaconDescriptor: $AV.d
							};
							let $AX = {
								signalFilter: $AV.b,
								notifyLastChance: true
							};
							$AX.encode = $KEY_5;
							$AX.descriptor = $AV.d;
							$AW.payload = $KEY_19($AX);
							return $AW;
						});
					}"#;
const DESCRIPTOR_PAYLOAD_BODY_SHA256: &str =
    "0293c34eebf1f0f0f60d322982a6ed0ed3970c8bb5939c534ad19872bf8011d1";

// A readable-spelling form of the same structural sibling: filtered record list,
// descriptor row, captured-encoder capture object and one payload write.
const ALPHA_BODY: &str = r#"function $THREAD_2_2(recordId) {
	let id = recordId;
	let filtered;
	if (recordId == null) {
		filtered = $KEY_260;
	} else {
		filtered = $KEY_260.filter(record => record.a === id);
	}
	return filtered.map(record => {
		let encoded = $KEY_413($KEY_0, record.c.map(code => $KEY_332[code]), $KEY_4, $KEY_8);
		let row = {
			beaconEndpoint: record.a,
			beaconDescriptor: record.d
		};
		let capture = {
			signalFilter: record.b,
			notifyLastChance: true
		};
		capture.encode = encoded;
		capture.descriptor = record.d;
		row.payload = $KEY_19(capture);
		return row;
	});
}"#;

// Donors already accepted for the beaconData report role must keep buildBeaconReports and
// must not be claimed by the descriptor-payload sibling.
const TARGET_DONOR_BODY: &str = r#"function $THREAD_3_38797($BH) {
				let $KEY_4 = $BH === void 0 ? {} : $BH;
				let $KEY_2 = $KEY_4.includeDebugNames !== void 0 && $KEY_4.includeDebugNames;
				let $KEY_5 = $KEY_4.notifyLastChance === void 0 || $KEY_4.notifyLastChance;
				return $KEY_33.map($BI => {
					let $BJ = {
						beaconEndpoint: void 0,
						beaconData: void 0
					};
					$BJ.beaconEndpoint = $BI.a;
					let $BK = {
						signalFilter: $BI.b,
						includeDebugNames: $KEY_2,
						notifyLastChance: $KEY_5
					};
					$BJ.beaconData = $KEY_21($BK);
					return $BJ;
				});
			}"#;
const TARGET_DONOR_SHA256: &str =
    "c417acf1d62f158fac17d9d30744e5053e4fa687e766444800a34970d018a13a";
const FIXED_REPORT_DONOR_BODY: &str = r#"function $THREAD_7_128660($CS) {
				let $KEY_1 = $CS === void 0 ? {} : $CS;
				let $KEY_4 = $KEY_1.includeDebugNames !== void 0 && $KEY_1.includeDebugNames;
				let $KEY_3 = $KEY_1.notifyLastChance === void 0 || $KEY_1.notifyLastChance;
				return $KEY_298.map($CT => {
					let $CU = {
						beaconEndpoint: void 0,
						beaconData: void 0
					};
					$CU.beaconEndpoint = $CT.a;
					let $CV = {
						signalFilter: $CT.b,
						includeDebugNames: $KEY_4,
						notifyLastChance: $KEY_3
					};
					$CU.beaconData = $KEY_18($CV);
					return $CU;
				});
			}"#;
const FIXED_REPORT_DONOR_SHA256: &str =
    "3a47b05c617a6f868bd7ccd351b197168eaf1b71197d0f20235df2fefbc14b1d";

const CONTEXT_PREFIX: &str = r#"(function(){
	const $KEY_260 = [];
	const $KEY_33 = [];
	const $KEY_298 = [];
	const $KEY_0 = {};
	const $KEY_4 = {};
	const $KEY_8 = {};
	const $KEY_332 = [];
	const $KEY_413 = (...values) => values;
	const $KEY_19 = value => value;
	const $KEY_21 = value => value;
	const $KEY_18 = value => value;
	const $OUTER = {a: "outer", b: "outer", c: [], d: "outer"};
	const $KEY_261 = "collector";
	const $AO = { global: {} };
	let $KEY_3 = $AO.global;
	function $THREAD_0_129852() {}
	function $THREAD_2_115229() {
		function $THREAD_2_52154(values) {
			return [values];
		}
		$KEY_3.ns_collect = $THREAD_2_52154;
"#;

const CONTEXT_SUFFIX: &str = r#"
	let $AS = { init: $THREAD_2_115229 };
	$AS.teardown = $THREAD_0_129852;
	return $AS;
})();"#;

fn program(body: &str, target: &str) -> String {
    let mut source = String::from(CONTEXT_PREFIX);
    source.push_str(body);
    source.push_str("\n\t\tif ($KEY_261) {\n\t\t\t$KEY_3[$KEY_261] = ");
    source.push_str(target);
    source.push_str(";\n\t\t\treturn void 0;\n\t\t} else {\n\t\t\treturn void 0;\n\t\t}\n\t}");
    source.push_str(CONTEXT_SUFFIX);
    source
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn named(source: &str, binding: &str, role: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|outcome| {
            outcome.binding.name == binding
                && outcome.applied
                && outcome.proposed_name.as_deref() == Some(role)
        })
}

fn refused(source: &str, target: &str) {
    let result = name_threads(source, true).unwrap();
    assert!(
        !result
            .report
            .outcomes
            .iter()
            .any(|outcome| outcome.proposed_name.as_deref() == Some(ROLE)),
        "descriptor/payload sibling was wrongly proposed"
    );
    assert!(
        result
            .report
            .outcomes
            .iter()
            .any(|outcome| outcome.binding.name == target),
        "missing target outcome; fixture did not exercise the target"
    );
}

fn mutate(original: &str, from: &str, to: &str) -> String {
    let mutated = original.replace(from, to);
    assert_ne!(mutated, original, "negative mutation was a NOOP");
    mutated
}

#[test]
fn witness_is_the_exact_624_byte_remaining_outcome() {
    assert_eq!(DESCRIPTOR_PAYLOAD_BODY.len(), 624);
    assert_eq!(
        sha256_hex(DESCRIPTOR_PAYLOAD_BODY.as_bytes()),
        DESCRIPTOR_PAYLOAD_BODY_SHA256
    );
    assert_eq!(sha256_hex(TARGET_DONOR_BODY.as_bytes()), TARGET_DONOR_SHA256);
    assert_eq!(sha256_hex(FIXED_REPORT_DONOR_BODY.as_bytes()), FIXED_REPORT_DONOR_SHA256);
}

#[test]
fn real_descriptor_payload_descriptor_payload_is_named() {
    let source = program(DESCRIPTOR_PAYLOAD_BODY, "$THREAD_8_12645");
    // The exact witness is embedded whole, next to the real init/teardown
    // installer on the dynamic global property alias.
    assert!(source.contains(DESCRIPTOR_PAYLOAD_BODY));
    assert!(source.contains("$KEY_3[$KEY_261] = $THREAD_8_12645;"));
    assert!(source.contains("$AS.teardown = $THREAD_0_129852;"));
    assert!(named(&source, "$THREAD_8_12645", ROLE));
    assert!(name_threads(&source, true)
        .unwrap()
        .source
        .contains("function buildBeaconDescriptorPayloads($AT)"));
}

#[test]
fn readable_alpha_descriptor_payload_is_named() {
    let source = program(ALPHA_BODY, "$THREAD_2_2");
    assert!(named(&source, "$THREAD_2_2", ROLE));
}

#[test]
fn report_donors_keep_canonical_role() {
    for (body, span, binding) in [
        (TARGET_DONOR_BODY, "$THREAD_3_38797", "$THREAD_3_38797"),
        (FIXED_REPORT_DONOR_BODY, "$THREAD_7_128660", "$THREAD_7_128660"),
    ] {
        let source = program(body, span);
        assert!(named(&source, binding, CANONICAL_REPORT_ROLE));
        assert!(!named(&source, binding, ROLE));
    }
}

#[test]
fn wrong_filtered_record_refuses() {
    let body = mutate(
        DESCRIPTOR_PAYLOAD_BODY,
        "return $AU.map($AV => {",
        "return $KEY_260.map($AV => {",
    );
    refused(&program(&body, "$THREAD_8_12645"), "$THREAD_8_12645");
}

#[test]
fn wrong_filter_call_refuses() {
    let body = mutate(
        DESCRIPTOR_PAYLOAD_BODY,
        "$AU = $KEY_260.filter($AV => $AV.a === $KEY_2);",
        "$AU = $KEY_260;",
    );
    refused(&program(&body, "$THREAD_8_12645"), "$THREAD_8_12645");
}

// The filter still runs exactly once, but its result is discarded by a later
// raw-list write before the return; the mapped receiver is not the filter
// result, so the sibling must refuse.
#[test]
fn filter_result_overwritten_refuses() {
    let body = mutate(
        DESCRIPTOR_PAYLOAD_BODY,
        "$AU = $KEY_260.filter($AV => $AV.a === $KEY_2);",
        "$AU = $KEY_260.filter($AV => $AV.a === $KEY_2); $AU = $KEY_260;",
    );
    refused(&program(&body, "$THREAD_8_12645"), "$THREAD_8_12645");
}

#[test]
fn foreign_filter_alias_refuses() {
    let body = mutate(DESCRIPTOR_PAYLOAD_BODY, "$AV.a === $KEY_2", "$AV.a === $OUTER");
    refused(&program(&body, "$THREAD_8_12645"), "$THREAD_8_12645");
}

#[test]
fn foreign_endpoint_source_refuses() {
    let body = mutate(DESCRIPTOR_PAYLOAD_BODY, "beaconEndpoint: $AV.a", "beaconEndpoint: $OUTER.a");
    refused(&program(&body, "$THREAD_8_12645"), "$THREAD_8_12645");
}

#[test]
fn foreign_descriptor_source_refuses() {
    let body = mutate(DESCRIPTOR_PAYLOAD_BODY, "beaconDescriptor: $AV.d", "beaconDescriptor: $OUTER.d");
    refused(&program(&body, "$THREAD_8_12645"), "$THREAD_8_12645");
}

#[test]
fn missing_payload_write_refuses() {
    let body = mutate(DESCRIPTOR_PAYLOAD_BODY, "\t\t\t\t\t\t\t$AW.payload = $KEY_19($AX);\n", "");
    refused(&program(&body, "$THREAD_8_12645"), "$THREAD_8_12645");
}

#[test]
fn foreign_encoder_source_refuses() {
    let body = mutate(DESCRIPTOR_PAYLOAD_BODY, "$AW.payload = $KEY_19($AX);", "$AW.payload = $KEY_19($OUTER);");
    refused(&program(&body, "$THREAD_8_12645"), "$THREAD_8_12645");
}

#[test]
fn missing_encoder_invocation_refuses() {
    let body = mutate(DESCRIPTOR_PAYLOAD_BODY, "$AX.encode = $KEY_5;", "$AX.encode = 1;");
    refused(&program(&body, "$THREAD_8_12645"), "$THREAD_8_12645");
}

#[test]
fn extra_row_effect_refuses() {
    let body = mutate(
        DESCRIPTOR_PAYLOAD_BODY,
        "$AW.payload = $KEY_19($AX);",
        "$AW.payload = $KEY_19($AX); $AW.extra = 1;",
    );
    refused(&program(&body, "$THREAD_8_12645"), "$THREAD_8_12645");
}

#[test]
fn extra_capture_effect_refuses() {
    let body = mutate(
        DESCRIPTOR_PAYLOAD_BODY,
        "$AX.descriptor = $AV.d;",
        "$AX.descriptor = $AV.d; $AX.extra = 1;",
    );
    refused(&program(&body, "$THREAD_8_12645"), "$THREAD_8_12645");
}

// Global-collection API coverage retains the complete outer carrier, its initializer, the
// encoded-header collector, and the independently accepted descriptor-payload mapper. The
// owned teardown keeps its existing anchor label and must never be claimed by this family.

const CREATE_GLOBAL_COLLECTION_API: &str = "createGlobalCollectionApi";
const INITIALISE_GLOBAL_COLLECTION_API: &str = "initialiseGlobalCollectionApi";
const COLLECT_ENCODED_HEADERS: &str = "collectEncodedHeaders";
const TEARDOWN_ANCHOR_ROLE: &str = "syncCapturedFieldsIfChanged";

const DESCRIPTOR_PAYLOAD_OUTER_BODY: &str = r#"function $THREAD_7_141559($AO, $AP, $AQ, $AR) {
			let $KEY_0 = $AP;
			let $KEY_1, $KEY_3;
			let $KEY_4 = $AQ;
			let $KEY_6;
			let $KEY_8 = $AR;
			function $THREAD_0_129852() {
				if ($KEY_3.ns_collect !== $KEY_6) {
					$KEY_3.ns_collect = $KEY_6;
				}
				if ($KEY_3[$KEY_261] !== $KEY_1) {
					$KEY_3[$KEY_261] = $KEY_1;
				}
			}
			function $THREAD_2_115229() {
				$KEY_6 = $KEY_3.ns_collect;
				function $THREAD_2_52154($AT) {
					if ($AT == null || $AT.url != null && $AT.method != null && $KEY_280($AT)) {
						let $AU = $KEY_19(void 0);
						let $AV = Object.create(null);
						if ($AU !== null && $AU !== void 0) {
							let $AW;
							for (const i in $AU) {
								if (Object.prototype.hasOwnProperty.call($AU, i)) {
									let $AX = $KEY_249("X-y8S6k3DB-" + i, $AU[i], 7900);
									if ($AX !== null && $AX !== void 0) {
										for (const j in $AX) {
											$AW = j;
											if (!!Object.prototype.hasOwnProperty.call($AX, $AW)) {
												$AV[$AW] = $AX[$AW];
											}
										}
									}
								}
							}
						}
						return $AV;
					} else {
						return null;
					}
				}
				$KEY_3.ns_collect = $THREAD_2_52154;
				if ($KEY_261) {
					$KEY_1 = $KEY_3[$KEY_261];
					function $THREAD_8_12645($AT) {
						let $KEY_2 = $AT;
						let $AU;
						if ($AT == null) {
							$AU = $KEY_260;
						} else {
							$AU = $KEY_260.filter($AV => $AV.a === $KEY_2);
						}
						return $AU.map($AV => {
							let $KEY_5 = $KEY_413($KEY_0, $AV.c.map($AY => $KEY_332[$AY]), $KEY_4, $KEY_8);
							let $AW = {
								beaconEndpoint: $AV.a,
								beaconDescriptor: $AV.d
							};
							let $AX = {
								signalFilter: $AV.b,
								notifyLastChance: true
							};
							$AX.encode = $KEY_5;
							$AX.descriptor = $AV.d;
							$AW.payload = $KEY_19($AX);
							return $AW;
						});
					}
					$KEY_3[$KEY_261] = $THREAD_8_12645;
					return void 0;
				} else {
					return void 0;
				}
			}
			$KEY_3 = $AO.global;
			let $AS = { init: $THREAD_2_115229 };
			$AS.teardown = $THREAD_0_129852;
			return $AS;
		}"#;
const DESCRIPTOR_PAYLOAD_OUTER_SHA256: &str =
    "06167a52b835945b04bf0236635d9794f46f02c51309d52515650fd88f37dcf6";
const DESCRIPTOR_PAYLOAD_OUTER_LEN: usize = 2082;
const DESCRIPTOR_PAYLOAD_INIT_OFFSET: usize = 346;
const DESCRIPTOR_PAYLOAD_INIT_LEN: usize = 1616;
const DESCRIPTOR_PAYLOAD_INIT_SHA256: &str =
    "c696a72a8255f51a3c4100c96b5e38845753b728aa56fbaf5d36c42fc6cac600";
const DESCRIPTOR_PAYLOAD_COLLECTOR_OFFSET: usize = 412;
const DESCRIPTOR_PAYLOAD_COLLECTOR_LEN: usize = 722;
const DESCRIPTOR_PAYLOAD_COLLECTOR_SHA256: &str =
    "465aecbe84569fcdc36aeb92fa991b1806d10bbc3bf7808a13a25387395d0f42";
const DESCRIPTOR_PAYLOAD_MAPPER_OFFSET: usize = 1233;
const DESCRIPTOR_PAYLOAD_MAPPER_LEN: usize = 624;

// The captured helpers the exact witness reads. `$KEY_261` is the captured
// dynamic property key; its value is opaque and never evidence.
const WRAPPER_PREFIX: &str = r#"(function(){
	const $KEY_260 = [];
	const $KEY_332 = [];
	const $KEY_413 = (...values) => values;
	const $KEY_19 = value => value;
	const $KEY_249 = (name, value, limit) => ({ [name]: value });
	const $KEY_280 = request => request != null;
	const $KEY_261 = "descriptorSlot";
"#;
const WRAPPER_SUFFIX: &str = "\n})();";

// A readable-spelling rewrite of the same three owned bodies: readable local
// names, the same exact structure, the same captured helper symbols and the
// same accepted descriptor-payload mapper role.
const ALPHA_OUTER_BODY: &str = r#"function $THREAD_7_7(ctx, alphabet, shuffle, encryption) {
			let encoderSeed = alphabet;
			let savedDescriptorSlot, globalTarget;
			let encoderShuffle = shuffle;
			let savedCollectorSlot;
			let encoderEncryption = encryption;
			function $THREAD_0_0() {
				if (globalTarget.ns_collect !== savedCollectorSlot) {
					globalTarget.ns_collect = savedCollectorSlot;
				}
				if (globalTarget[$KEY_261] !== savedDescriptorSlot) {
					globalTarget[$KEY_261] = savedDescriptorSlot;
				}
			}
			function $THREAD_2_2() {
				savedCollectorSlot = globalTarget.ns_collect;
				function $THREAD_2_3(request) {
					if (request == null || request.url != null && request.method != null && $KEY_280(request)) {
						let generated = $KEY_19(void 0);
						let headers = Object.create(null);
						if (generated !== null && generated !== void 0) {
							let lastHeader;
							for (const key in generated) {
								if (Object.prototype.hasOwnProperty.call(generated, key)) {
									let formatted = $KEY_249("prefix-" + key, generated[key], 4096);
									if (formatted !== null && formatted !== void 0) {
										for (const header in formatted) {
											lastHeader = header;
											if (!!Object.prototype.hasOwnProperty.call(formatted, lastHeader)) {
												headers[lastHeader] = formatted[lastHeader];
											}
										}
									}
								}
							}
						}
						return headers;
					} else {
						return null;
					}
				}
				globalTarget.ns_collect = $THREAD_2_3;
				if ($KEY_261) {
					savedDescriptorSlot = globalTarget[$KEY_261];
					function $THREAD_2_4(recordId) {
						let id = recordId;
						let filtered;
						if (recordId == null) {
							filtered = $KEY_260;
						} else {
							filtered = $KEY_260.filter(record => record.a === id);
						}
						return filtered.map(record => {
							let encoded = $KEY_413(encoderSeed, record.c.map(code => $KEY_332[code]), encoderShuffle, encoderEncryption);
							let row = {
								beaconEndpoint: record.a,
								beaconDescriptor: record.d
							};
							let capture = {
								signalFilter: record.b,
								notifyLastChance: true
							};
							capture.encode = encoded;
							capture.descriptor = record.d;
							row.payload = $KEY_19(capture);
							return row;
						});
					}
					globalTarget[$KEY_261] = $THREAD_2_4;
					return void 0;
				} else {
					return void 0;
				}
			}
			globalTarget = ctx.global;
			let api = { init: $THREAD_2_2 };
			api.teardown = $THREAD_0_0;
			return api;
		}"#;

fn wrapper_program(outer: &str) -> String {
    let mut source = String::from(WRAPPER_PREFIX);
    source.push_str(outer);
    source.push_str(WRAPPER_SUFFIX);
    source
}

fn refused_role(source: &str, binding: &str, role: &str) {
    let result = name_threads(source, true).unwrap();
    assert!(
        result
            .report
            .outcomes
            .iter()
            .any(|outcome| outcome.binding.name == binding),
        "missing target outcome; fixture did not exercise {binding}"
    );
    assert!(
        !result.report.outcomes.iter().any(|outcome| {
            outcome.binding.name == binding
                && outcome.applied
                && outcome.proposed_name.as_deref() == Some(role)
        }),
        "unexpectedly named {binding} as {role}"
    );
}

fn wrapper_negative(body: &str, from: &str, to: &str, binding: &str, role: &str) {
    let mutated = mutate(body, from, to);
    refused_role(&wrapper_program(&mutated), binding, role);
}

#[test]
fn wrapper_witness_pins_are_exact() {
    let outer = DESCRIPTOR_PAYLOAD_OUTER_BODY;
    assert_eq!(outer.len(), DESCRIPTOR_PAYLOAD_OUTER_LEN);
    assert_eq!(sha256_hex(outer.as_bytes()), DESCRIPTOR_PAYLOAD_OUTER_SHA256);
    let bytes = outer.as_bytes();
    let init = &bytes[DESCRIPTOR_PAYLOAD_INIT_OFFSET..DESCRIPTOR_PAYLOAD_INIT_OFFSET + DESCRIPTOR_PAYLOAD_INIT_LEN];
    let collector =
        &bytes[DESCRIPTOR_PAYLOAD_COLLECTOR_OFFSET..DESCRIPTOR_PAYLOAD_COLLECTOR_OFFSET + DESCRIPTOR_PAYLOAD_COLLECTOR_LEN];
    let mapper = &bytes[DESCRIPTOR_PAYLOAD_MAPPER_OFFSET..DESCRIPTOR_PAYLOAD_MAPPER_OFFSET + DESCRIPTOR_PAYLOAD_MAPPER_LEN];
    assert_eq!(sha256_hex(init), DESCRIPTOR_PAYLOAD_INIT_SHA256);
    assert_eq!(sha256_hex(collector), DESCRIPTOR_PAYLOAD_COLLECTOR_SHA256);
    assert_eq!(sha256_hex(mapper), DESCRIPTOR_PAYLOAD_BODY_SHA256);
    assert_eq!(mapper, DESCRIPTOR_PAYLOAD_BODY.as_bytes());
}

#[test]
fn real_descriptor_payload_global_collection_api_is_named() {
    let source = wrapper_program(DESCRIPTOR_PAYLOAD_OUTER_BODY);
    assert!(source.contains(DESCRIPTOR_PAYLOAD_OUTER_BODY));
    assert!(named(&source, "$THREAD_7_141559", CREATE_GLOBAL_COLLECTION_API));
    assert!(named(&source, "$THREAD_2_115229", INITIALISE_GLOBAL_COLLECTION_API));
    assert!(named(&source, "$THREAD_2_52154", COLLECT_ENCODED_HEADERS));
    // The embedded mapper keeps the accepted 002 sibling role.
    assert!(named(&source, "$THREAD_8_12645", ROLE));
    // The restore function keeps its pre-existing anchor label and is never
    // claimed by this family.
    assert!(named(&source, "$THREAD_0_129852", TEARDOWN_ANCHOR_ROLE));
    assert!(!named(&source, "$THREAD_0_129852", CREATE_GLOBAL_COLLECTION_API));
    assert!(!named(&source, "$THREAD_0_129852", INITIALISE_GLOBAL_COLLECTION_API));
    assert!(!named(&source, "$THREAD_0_129852", COLLECT_ENCODED_HEADERS));
    let transformed = name_threads(&source, true).unwrap().source;
    assert!(transformed.contains("function createGlobalCollectionApi("));
    assert!(transformed.contains("function initialiseGlobalCollectionApi("));
    assert!(transformed.contains("function collectEncodedHeaders("));
}

#[test]
fn readable_alpha_global_collection_api_is_named() {
    let source = wrapper_program(ALPHA_OUTER_BODY);
    assert!(named(&source, "$THREAD_7_7", CREATE_GLOBAL_COLLECTION_API));
    assert!(named(&source, "$THREAD_2_2", INITIALISE_GLOBAL_COLLECTION_API));
    assert!(named(&source, "$THREAD_2_3", COLLECT_ENCODED_HEADERS));
    assert!(named(&source, "$THREAD_2_4", ROLE));
    assert!(named(&source, "$THREAD_0_0", TEARDOWN_ANCHOR_ROLE));
}

#[test]
fn wrong_global_refuses() {
    wrapper_negative(
        DESCRIPTOR_PAYLOAD_OUTER_BODY,
        "$KEY_3 = $AO.global;",
        "$KEY_3 = $AO.realm;",
        "$THREAD_7_141559",
        CREATE_GLOBAL_COLLECTION_API,
    );
}

#[test]
fn wrong_installer_refuses() {
    wrapper_negative(
        DESCRIPTOR_PAYLOAD_OUTER_BODY,
        "$KEY_3.ns_collect = $THREAD_2_52154;",
        "$KEY_3.ns_collect = $KEY_19;",
        "$THREAD_2_115229",
        INITIALISE_GLOBAL_COLLECTION_API,
    );
}

#[test]
fn wrong_restore_slot_refuses() {
    wrapper_negative(
        DESCRIPTOR_PAYLOAD_OUTER_BODY,
        "$KEY_3.ns_collect !== $KEY_6",
        "$KEY_3.ns_collect !== $KEY_1",
        "$THREAD_7_141559",
        CREATE_GLOBAL_COLLECTION_API,
    );
}

#[test]
fn wrong_payload_refuses() {
    wrapper_negative(
        DESCRIPTOR_PAYLOAD_OUTER_BODY,
        "let $AU = $KEY_19(void 0);",
        "let $AU = $KEY_19(0);",
        "$THREAD_2_52154",
        COLLECT_ENCODED_HEADERS,
    );
}

#[test]
fn wrong_header_factory_refuses() {
    wrapper_negative(
        DESCRIPTOR_PAYLOAD_OUTER_BODY,
        "$KEY_249(\"X-y8S6k3DB-\" + i, $AU[i], 7900)",
        "$KEY_19(\"X-y8S6k3DB-\" + i, $AU[i], 7900)",
        "$THREAD_2_52154",
        COLLECT_ENCODED_HEADERS,
    );
}

#[test]
fn wrong_key_copy_refuses() {
    wrapper_negative(
        DESCRIPTOR_PAYLOAD_OUTER_BODY,
        "$AV[$AW] = $AX[$AW];",
        "$AV[$AW] = $AX[i];",
        "$THREAD_2_52154",
        COLLECT_ENCODED_HEADERS,
    );
}

#[test]
fn wrong_request_guard_refuses() {
    wrapper_negative(
        DESCRIPTOR_PAYLOAD_OUTER_BODY,
        "$AT == null || $AT.url != null && $AT.method != null && $KEY_280($AT)",
        "$AT == null || $AT.url != null && $KEY_280($AT)",
        "$THREAD_2_52154",
        COLLECT_ENCODED_HEADERS,
    );
}

#[test]
fn wrong_return_frame_refuses() {
    wrapper_negative(
        DESCRIPTOR_PAYLOAD_OUTER_BODY,
        "return $AV;",
        "return null;",
        "$THREAD_2_52154",
        COLLECT_ENCODED_HEADERS,
    );
}

#[test]
fn extra_collector_effect_refuses() {
    wrapper_negative(
        DESCRIPTOR_PAYLOAD_OUTER_BODY,
        "return $AV;",
        "$AV.extra = 1; return $AV;",
        "$THREAD_2_52154",
        COLLECT_ENCODED_HEADERS,
    );
}
