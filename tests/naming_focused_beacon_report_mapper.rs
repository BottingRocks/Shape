use sha2::{Digest, Sha256};
use shape::naming::name_threads;

const ROLE: &str = "buildBeaconReports";

// The literal-endpoint record mapper is embedded byte-for-byte so recognition is tested
// against its actual body rather than a normalized paraphrase.
const LITERAL_ENDPOINT_WITNESS: &str = r#"function $THREAD_8_65495($BG) {
				let $KEY_2 = $BG === void 0 ? {} : $BG;
				let $KEY_0 = $KEY_2.includeDebugNames !== void 0 && $KEY_2.includeDebugNames;
				let $KEY_4 = $KEY_2.notifyLastChance === void 0 || $KEY_2.notifyLastChance;
				return $KEY_260.map($BH => {
					let $BI = { beaconEndpoint: $BH.a };
					let $BJ = {
						signalFilter: $BH.b,
						includeDebugNames: $KEY_0,
						notifyLastChance: $KEY_4
					};
					$BI.beaconData = $KEY_1($BJ);
					return $BI;
				});
			}"#;
const LITERAL_ENDPOINT_WITNESS_SHA256: &str =
    "7012ab6ccc84d70b4c8065725e64a83b3aec657c38378303f9067c936b848364";

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn program(body: &str, target: &str) -> String {
    format!(
        "(function(){{\nconst $KEY_260 = [];\nconst $KEY_5 = [];\nconst $KEY_1 = value => value;\nconst $KEY_6 = value => value;\nconst $OUTER = {{a: 1}};\n{body}\nreturn [{target}];\n}})();"
    )
}

fn applied(source: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(ROLE))
}

fn refused(mutated: &str, target: &str) {
    let source = program(mutated, target);
    assert!(!applied(&source));
    assert_eq!(name_threads(&source, true).unwrap().source, source);
}

// The already-applied base form: the callback object literal declares both
// fields and then assigns both, so the legacy `Facts.writes` guard still owns
// the name.
const LEGACY_ASSIGNMENT_BASE: &str = r#"function $THREAD_3_3($CS) {
	let $KEY_2 = $CS === void 0 ? {} : $CS;
	return $KEY_260.map($CT => {
		let $CU = {
			beaconEndpoint: void 0,
			beaconData: void 0
		};
		$CU.beaconEndpoint = $CT.a;
		let $CV = {
			signalFilter: $CT.b,
			includeDebugNames: $KEY_2.includeDebugNames
		};
		$CU.beaconData = $KEY_1($CV);
		return $CU;
	});
}"#;

// Same structure with readable local/parameter spelling.
const ALPHA_WITNESS: &str = r#"function $THREAD_2_2(recordSource) {
	let settings = recordSource === void 0 ? {} : recordSource;
	let debugNames = settings.includeDebugNames !== void 0 && settings.includeDebugNames;
	let notify = settings.notifyLastChance === void 0 || settings.notifyLastChance;
	return $KEY_5.map(entry => {
		let row = { beaconEndpoint: entry.endpoint };
		let payload = {
			signalFilter: entry.b,
			includeDebugNames: debugNames,
			notifyLastChance: notify
		};
		row.beaconData = $KEY_6(payload);
		return row;
	});
}"#;

// A nested helper assembles the record; the owning callback only forwards it,
// so the nested donor evidence must not migrate to the outer `.map` callback.
const NESTED_DONOR: &str = r#"function $THREAD_2_2($BG) {
	let $KEY_2 = $BG === void 0 ? {} : $BG;
	return $KEY_260.map($BH => {
		function makeReport(r) {
			let row = { beaconEndpoint: r.a };
			let payload = { signalFilter: r.b };
			row.beaconData = $KEY_1(payload);
			return row;
		}
		return makeReport($BH);
	});
}"#;

#[test]
fn witness_is_the_exact_493_byte_remaining_outcome() {
    assert_eq!(LITERAL_ENDPOINT_WITNESS.len(), 493);
    assert_eq!(
        sha256_hex(LITERAL_ENDPOINT_WITNESS.as_bytes()),
        LITERAL_ENDPOINT_WITNESS_SHA256
    );
}

#[test]
fn legacy_assignment_base_is_still_named() {
    let source = program(LEGACY_ASSIGNMENT_BASE, "$THREAD_3_3");
    assert!(applied(&source));
    let result = name_threads(&source, true).unwrap();
    assert!(result.source.contains("function buildBeaconReports($CS)"));
    assert_eq!(result.source, source.replace("$THREAD_3_3", ROLE));
}

#[test]
fn real_literal_endpoint_literal_endpoint_record_is_named() {
    let source = program(LITERAL_ENDPOINT_WITNESS, "$THREAD_8_65495");
    assert!(applied(&source));
    let result = name_threads(&source, true).unwrap();
    assert!(result.source.contains("function buildBeaconReports($BG)"));
    assert_eq!(result.source, source.replace("$THREAD_8_65495", ROLE));
}

#[test]
fn readable_alpha_literal_endpoint_record_is_named() {
    let source = program(ALPHA_WITNESS, "$THREAD_2_2");
    assert!(applied(&source));
    let result = name_threads(&source, true).unwrap();
    assert!(result
        .source
        .contains("function buildBeaconReports(recordSource)"));
    assert_eq!(result.source, source.replace("$THREAD_2_2", ROLE));
}

#[test]
fn wrong_returned_object_refuses() {
    let mutated = LITERAL_ENDPOINT_WITNESS.replace("return $BI;", "return $BJ;");
    assert_ne!(mutated, LITERAL_ENDPOINT_WITNESS);
    refused(&mutated, "$THREAD_8_65495");
}

#[test]
fn read_only_endpoint_mention_refuses() {
    // The callback only reads `beaconEndpoint`; it is not the row-literal key.
    let mutated = LITERAL_ENDPOINT_WITNESS.replace(
        "let $BI = { beaconEndpoint: $BH.a };",
        "let $BI = { signalFilter: $BH.beaconEndpoint };",
    );
    assert_ne!(mutated, LITERAL_ENDPOINT_WITNESS);
    refused(&mutated, "$THREAD_8_65495");
}

#[test]
fn missing_beacon_data_refuses() {
    let mutated = LITERAL_ENDPOINT_WITNESS.replace("$BI.beaconData = $KEY_1($BJ);", "");
    assert_ne!(mutated, LITERAL_ENDPOINT_WITNESS);
    refused(&mutated, "$THREAD_8_65495");
}

#[test]
fn foreign_row_endpoint_refuses() {
    // The endpoint value comes from a captured outer object, not this
    // callback's own formal row.
    let mutated = LITERAL_ENDPOINT_WITNESS.replace("beaconEndpoint: $BH.a", "beaconEndpoint: $OUTER.a");
    assert_ne!(mutated, LITERAL_ENDPOINT_WITNESS);
    refused(&mutated, "$THREAD_8_65495");
}

#[test]
fn nested_donor_record_refuses() {
    refused(NESTED_DONOR, "$THREAD_2_2");
}

#[test]
fn duplicate_payload_write_refuses() {
    let mutated = LITERAL_ENDPOINT_WITNESS.replace(
        "$BI.beaconData = $KEY_1($BJ);",
        "$BI.beaconData = $KEY_1($BJ); $BI.beaconData = $KEY_1($BJ);",
    );
    assert_ne!(mutated, LITERAL_ENDPOINT_WITNESS);
    refused(&mutated, "$THREAD_8_65495");
}
