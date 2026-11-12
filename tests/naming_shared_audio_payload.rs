use shape::naming::name_threads;

const USE_SITE_NONCE_BODY: &str = r###"function $THREAD_5_67726($DM) {
									let $KEY_5 = $DM.getChannelData(0);
									let $DN = Math.random() * 4294967296;
									let $DO = [];
									let $DP = [($DN | 0) >>> 24, ($DN | 0) >> 16 & 255, ($DN | 0) >> 8 & 255, ($DN | 0) & 255];
									$DN = $KEY_385(($DN | 0) ^ 2960915465);
									$KEY_5 = Array.from($KEY_5);
									let $DQ = $KEY_5.length;
									let $DR = $KEY_351($DN, $DQ, $DP, 3);
									while ($DQ - 1 >= 0) {
										$DR = $KEY_351($DN, $KEY_5[$DQ - 1], $DP, $DR);
										--$DQ;
									}
									$KEY_351($DN, $KEY_13[0], $DP, $DR);
									$KEY_8.next([$DO, $DP]);
									$KEY_8.complete();
								}"###;
fn use_site_nonce_carrier(body: &str) -> String {
    format!("function main(){{ let observer={{next(){{}},complete(){{}}}}, trailer=[0], anchor=1; function hash(v){{return v;}} function mix(h,v,b,c){{return c;}} {} return $THREAD_5_67726;}} main();", use_site_nonce_body(body))
}
fn use_site_nonce_body(body: &str) -> String {
    body.replace("$KEY_385", "hash")
        .replace("$KEY_351", "mix")
        .replace("$KEY_13[0]", "trailer[0]")
        .replace("$KEY_8.", "observer.")
}
fn check_use_site_nonce(body: &str, expected: bool) {
    let source = use_site_nonce_carrier(body);
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, &source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        parsed.errors.is_empty(),
        "invalid fixture: {:?}",
        parsed.errors
    );
    let result = name_threads(&source, true).unwrap();
    let o = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_5_67726")
        .unwrap();
    assert_eq!(
        o.applied && o.proposed_name.as_deref() == Some("emitSeededHashEncoding"),
        expected,
        "{}\nOutcome: {:?}",
        body,
        o
    );
}
const DECLARATION_NONCE_BODY: &str = r###"function $THREAD_2_98829($CU) {
									let $KEY_2 = $CU.getChannelData(0);
									let $KEY_3 = Math.random() * 4294967296 | 0;
									let $CV = [$KEY_3 >>> 24, $KEY_3 >> 16 & 255, $KEY_3 >> 8 & 255, $KEY_3 & 255];
									let $CW = [];
									$KEY_3 = $KEY_192($KEY_3 ^ 2498983620);
									$KEY_2 = Array.from($KEY_2);
									let $CX = $KEY_2.length;
									let $CY = $KEY_47($KEY_3, $CX, $CV, 3);
									while ($CX - 1 >= 0) {
										$CY = $KEY_47($KEY_3, $KEY_2[$CX - 1], $CV, $CY);
										--$CX;
									}
									$KEY_47($KEY_3, $KEY_4[0], $CV, $CY);
									$KEY_18.next([$CW, $CV]);
									$KEY_18.complete();
								}"###;
fn declaration_nonce_carrier(body: &str) -> String {
    format!("function main(){{ let observer={{next(){{}},complete(){{}}}}, trailer=[0], anchor=1; function hash(v){{return v;}} function mix(h,v,b,c){{return c;}} {} return $THREAD_2_98829;}} main();", declaration_nonce_body(body))
}
fn declaration_nonce_body(body: &str) -> String {
    body.replace("$KEY_192", "hash")
        .replace("$KEY_47", "mix")
        .replace("$KEY_4[0]", "trailer[0]")
        .replace("$KEY_18.", "observer.")
}
fn check_declaration_nonce(body: &str, expected: bool) {
    let source = declaration_nonce_carrier(body);
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, &source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        parsed.errors.is_empty(),
        "invalid fixture: {:?}",
        parsed.errors
    );
    let result = name_threads(&source, true).unwrap();
    let o = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_2_98829")
        .unwrap();
    assert_eq!(
        o.applied && o.proposed_name.as_deref() == Some("emitSeededHashEncoding"),
        expected,
        "{}\nOutcome: {:?}",
        body,
        o
    );
}
#[test]
fn audio_reverse_selected_variants_and_alpha_renaming() {
    check_use_site_nonce(USE_SITE_NONCE_BODY, true);
    check_declaration_nonce(DECLARATION_NONCE_BODY, true);
    check_use_site_nonce(
        &USE_SITE_NONCE_BODY
            .replace("$DN", "nonceWord")
            .replace("2960915465", "12345"),
        true,
    );
    check_declaration_nonce(
        &DECLARATION_NONCE_BODY
            .replace("$CV", "headerBytes")
            .replace("2498983620", "67890"),
        true,
    );
}
#[test]
fn audio_reverse_use_site_nonce_refuses_near_misses_and_nested_donors() {
    for (from, to) in [
        ("getChannelData(0)", "getChannelData(1)"),
        ("Array.from($KEY_5)", "Array.from([])"),
        ("$DQ - 1 >= 0", "$DQ >= 0"),
        ("--$DQ", "++$DQ"),
        ("$KEY_5[$DQ - 1]", "$KEY_5[$DQ]"),
        ("$KEY_8.complete()", "other.complete()"),
        ("$KEY_8.next(", "other.next("),
        ("$DM.getChannelData(0)", "other.getChannelData(0)"),
        ("while ($DQ - 1 >= 0)", "if (false) while ($DQ - 1 >= 0)"),
    ] {
        assert!(USE_SITE_NONCE_BODY.contains(from));
        check_use_site_nonce(&USE_SITE_NONCE_BODY.replace(from, to), false);
    }
    let start = USE_SITE_NONCE_BODY.find("while (").unwrap();
    let end = USE_SITE_NONCE_BODY[start..].find("\n\t\t\t\t\t\t\t\t\t}").unwrap() + start + 11;
    let mut nested = USE_SITE_NONCE_BODY.to_string();
    nested.insert_str(end, " });");
    nested.insert_str(start, "(() => { ");
    check_use_site_nonce(&nested, false);
}
#[test]
fn audio_reverse_declaration_nonce_refuses_near_misses_and_nested_donors() {
    for (from, to) in [
        ("getChannelData(0)", "getChannelData(1)"),
        ("Array.from($KEY_2)", "Array.from([])"),
        ("$CX - 1 >= 0", "$CX >= 0"),
        ("--$CX", "++$CX"),
        ("$KEY_2[$CX - 1]", "$KEY_2[$CX]"),
        ("$KEY_18.complete()", "other.complete()"),
        ("$KEY_18.next(", "other.next("),
        ("$CU.getChannelData(0)", "other.getChannelData(0)"),
        ("while ($CX - 1 >= 0)", "if (false) while ($CX - 1 >= 0)"),
    ] {
        assert!(DECLARATION_NONCE_BODY.contains(from));
        check_declaration_nonce(&DECLARATION_NONCE_BODY.replace(from, to), false);
    }
    let start = DECLARATION_NONCE_BODY.find("while (").unwrap();
    let end = DECLARATION_NONCE_BODY[start..].find("\n\t\t\t\t\t\t\t\t\t}").unwrap() + start + 11;
    let mut nested = DECLARATION_NONCE_BODY.to_string();
    nested.insert_str(end, " });");
    nested.insert_str(start, "(() => { ");
    check_declaration_nonce(&nested, false);
}
