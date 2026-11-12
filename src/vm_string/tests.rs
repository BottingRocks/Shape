use super::*;

#[test]
fn manual_decoder_rejects_padding_and_url_safe_variants() {
    let limits = VmStringLimits::default();
    assert!(decode_manual_base64("YQ=", &limits).is_err());
    assert!(decode_manual_base64("-_8", &limits).is_err());
    assert_eq!(decode_manual_base64("SGVsbG8", &limits).unwrap(), b"Hello");
}

#[test]
fn proven_decoder_reads_only_its_own_alphabet() {
    let limits = VmStringLimits::default();
    assert_eq!(
        decode_proven_base64("-_8", URL_SAFE_ALPHABET, &limits).unwrap(),
        decode_proven_base64("+/8", STANDARD_ALPHABET, &limits).unwrap()
    );
    assert!(decode_proven_base64("+/8", URL_SAFE_ALPHABET, &limits).is_err());
    assert!(decode_proven_base64("YQ=", URL_SAFE_ALPHABET, &limits).is_err());
}

#[test]
fn raw_xor_preserves_non_ascii_code_units() {
    let value = evaluate_raw_xor(&[0, 0xE9], &[0, 0], &VmStringLimits::default()).unwrap();
    assert_eq!(value.units(), &[0xE9]);
    assert_eq!(value.to_string().unwrap(), "é");
}
