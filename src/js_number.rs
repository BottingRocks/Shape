//! Shared ECMA-262 numeric primitives.
//!
//! The concrete executor, the symbolic tracer, and the static pipeline all
//! need the same JavaScript conversions. Keeping them here gives those
//! consumers one implementation while leaving each consumer's *policy* alone:
//! for example, the symbolic tracer still declines to fold string-to-number.
//!
//! These helpers are crate-private. They are implementation details of the
//! interpreters, not part of the public library API.

use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, Statement};
use oxc_ecmascript::StringToNumber;
use oxc_parser::Parser;
use oxc_span::SourceType;
use oxc_syntax::{
    identifier::is_white_space, line_terminator::is_line_terminator, number::ToJsString,
};

/// ECMA-262 `ToInt32` (§7.1.6).
///
/// The value is truncated toward zero, reduced modulo 2^32, and interpreted as
/// a signed 32-bit integer. Non-finite values and both zeros map to zero.
pub(crate) fn to_int32(value: f64) -> i32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    let truncated = value.trunc();
    let modulo = truncated.rem_euclid(4_294_967_296.0);
    if modulo >= 2_147_483_648.0 {
        (modulo - 4_294_967_296.0) as i32
    } else {
        modulo as i32
    }
}

/// ECMA-262 `ToUint32` (§7.1.7).
///
/// Reducing to a signed 32-bit integer and reinterpreting those bits is
/// equivalent to reducing modulo 2^32 into the unsigned range.
pub(crate) fn to_uint32(value: f64) -> u32 {
    to_int32(value) as u32
}

/// ECMA-262 `ToNumber` for strings (§7.1.4), backed by Oxc's `StringToNumber`.
///
/// Both ends are trimmed here because Oxc intentionally trims only the start
/// before its primitive parser; the grammar permits JavaScript whitespace on
/// both ends, including BOM and NBSP. Oxc's radix slow path accumulates
/// directly into binary64, so values wider than `u64` are not truncated or
/// rejected.
pub(crate) fn string_to_number(value: &str) -> f64 {
    let trimmed =
        value.trim_matches(|character| is_white_space(character) || is_line_terminator(character));
    trimmed.string_to_number()
}

/// ECMA-262 `Number::toString` (§6.1.6.1.20) for a numeric value.
///
/// Oxc's `ToJsString` implements the ECMA-262 algorithm (including the
/// `[1e-6, 1e21)` fixed-notation window and canonical exponent signs). The
/// special values are explicit so this primitive's signed-zero and NaN
/// contract is visible at the call site.
pub(crate) fn number_to_js_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value == f64::INFINITY {
        return "Infinity".into();
    }
    if value == f64::NEG_INFINITY {
        return "-Infinity".into();
    }
    if value == 0.0 {
        return "0".into();
    }
    value.to_js_string()
}

/// Normalize a manually supplied numeric literal to the exact binary64 value
/// Oxc would decode.
///
/// Extraction does not call this: it carries
/// [`crate::operations::templates::LiteralValue::DecodedNumber`] bits. This
/// bounded helper exists only for manually constructed
/// `LiteralValue::Number(String)` recipes and other lexical consumers, so that
/// raw legacy octal (`010`), separators, and prefixed literals wider than 64
/// bits match Oxc instead of a Rust `str::parse`. Canonical `NaN`/`Infinity`
/// spellings are handled explicitly; everything else is validated by parsing a
/// single expression with Oxc and requiring it to be exactly one numeric
/// literal. No source is executed and no arbitrary expression is evaluated.
pub(crate) fn parse_numeric_literal(raw: &str) -> Option<f64> {
    let text = raw.trim();
    let (negative, magnitude) = match text.strip_prefix('-') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, text.strip_prefix('+').unwrap_or(text).trim_start()),
    };
    match magnitude {
        "NaN" => return Some(if negative { -f64::NAN } else { f64::NAN }),
        "Infinity" => {
            return Some(if negative {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            })
        }
        _ => {}
    }
    if magnitude.is_empty() {
        return None;
    }
    let value = oxc_numeric_literal(magnitude)?;
    Some(if negative { -value } else { value })
}

/// Parse `text` with Oxc and return the value only when the whole program is a
/// single numeric-literal expression statement. Anything else fails closed.
fn oxc_numeric_literal(text: &str) -> Option<f64> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, text, SourceType::script()).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let mut body = parsed.program.body.iter();
    let statement = body.next()?;
    if body.next().is_some() {
        return None;
    }
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    match &statement.expression {
        Expression::NumericLiteral(literal) => Some(literal.value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Frozen copies of the two pre-refactor implementations. They keep the
    // characterization reproducible: any future edit to the shared helpers is
    // checked against both algorithms that this refactor replaced.
    fn legacy_executor_to_int32(value: f64) -> i32 {
        if !value.is_finite() || value == 0.0 {
            return 0;
        }
        let truncated = value.trunc();
        let modulo = truncated.rem_euclid(4_294_967_296.0);
        if modulo >= 2_147_483_648.0 {
            (modulo - 4_294_967_296.0) as i32
        } else {
            modulo as i32
        }
    }

    fn legacy_symbolic_to_uint32(value: f64) -> u32 {
        if !value.is_finite() || value == 0.0 {
            return 0;
        }
        let truncated = value.trunc();
        let modulo = truncated % 4_294_967_296.0;
        if modulo < 0.0 {
            (modulo + 4_294_967_296.0) as u32
        } else {
            modulo as u32
        }
    }

    fn edge_values() -> Vec<(&'static str, f64)> {
        vec![
            ("+0", 0.0),
            ("-0", -0.0),
            ("NaN", f64::NAN),
            ("-NaN", f64::from_bits(0xfff8_0000_0000_0000)),
            ("+Infinity", f64::INFINITY),
            ("-Infinity", f64::NEG_INFINITY),
            ("2^31-1", 2_147_483_647.0),
            ("2^31", 2_147_483_648.0),
            ("-2^31", -2_147_483_648.0),
            ("-2^31-1", -2_147_483_649.0),
            ("2^32", 4_294_967_296.0),
            ("2^32+1", 4_294_967_297.0),
            ("-2^32", -4_294_967_296.0),
            ("2^53", 9_007_199_254_740_992.0),
            ("2^53+1 (rounds to 2^53)", 9_007_199_254_740_993.0),
            ("2^53+2", f64::from_bits(0x4340_0000_0000_0001)),
            ("1e21", 1e21),
            ("-1e21", -1e21),
            ("4294967295.5", 4_294_967_295.5),
            ("-0.5", -0.5),
            ("-4294967295.5", -4_294_967_295.5),
            ("f64::MAX", f64::MAX),
            ("-f64::MAX", -f64::MAX),
            ("f64::MIN_POSITIVE", f64::MIN_POSITIVE),
            ("min subnormal", f64::from_bits(1)),
            ("max subnormal", f64::from_bits(0x000f_ffff_ffff_ffff)),
        ]
    }

    #[test]
    fn conversions_agree_with_both_legacy_implementations() {
        for (label, value) in edge_values() {
            let expected_i32 = legacy_executor_to_int32(value);
            let expected_u32 = legacy_symbolic_to_uint32(value);
            assert_eq!(expected_i32, to_int32(value), "ToInt32 for {label}");
            assert_eq!(expected_u32, to_uint32(value), "ToUint32 for {label}");
            assert_eq!(
                expected_i32 as u32, expected_u32,
                "legacy ToInt32/ToUint32 disagreement for {label}"
            );
            if value.is_nan() {
                assert_eq!(to_int32(value), 0, "NaN ToInt32 for {label}");
                assert_eq!(to_uint32(value), 0, "NaN ToUint32 for {label}");
            }
            if value == 0.0 {
                assert_eq!(to_int32(value), 0);
                assert_eq!(to_uint32(value), 0);
            }
        }

        let mut state = 0x243f_6a88_85a3_08d3u64;
        let mut nan_count = 0usize;
        let mut zero_count = 0usize;
        for _ in 0..250_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let value = f64::from_bits(state);
            if value.is_nan() {
                nan_count += 1;
            }
            if value == 0.0 {
                zero_count += 1;
            }
            let expected_i32 = legacy_executor_to_int32(value);
            let expected_u32 = legacy_symbolic_to_uint32(value);
            assert_eq!(
                expected_i32,
                to_int32(value),
                "ToInt32 mismatch for bits {state:016x}"
            );
            assert_eq!(
                expected_u32,
                to_uint32(value),
                "ToUint32 mismatch for bits {state:016x}"
            );
            assert_eq!(
                expected_i32 as u32, expected_u32,
                "legacy disagreement for bits {state:016x}"
            );
        }
        // The deterministic seed happens to hit no zeros; NaNs are still
        // classified rather than compared by numeric equality.
        assert!(nan_count > 0);
        assert_eq!(zero_count, 0);
    }

    #[test]
    fn to_int32_and_to_uint32_match_ecma_262_boundaries() {
        for (value, int32, uint32) in [
            (0.0, 0, 0),
            (-0.0, 0, 0),
            (f64::NAN, 0, 0),
            (f64::INFINITY, 0, 0),
            (f64::NEG_INFINITY, 0, 0),
            (2_147_483_647.0, 2_147_483_647, 2_147_483_647),
            (2_147_483_648.0, -2_147_483_648, 2_147_483_648),
            (-2_147_483_648.0, -2_147_483_648, 2_147_483_648),
            (-2_147_483_649.0, 2_147_483_647, 2_147_483_647),
            (4_294_967_295.5, -1, 4_294_967_295),
            (-0.5, 0, 0),
            (4_294_967_296.0, 0, 0),
            (-4_294_967_296.0, 0, 0),
        ] {
            assert_eq!(to_int32(value), int32, "ToInt32({value:?})");
            assert_eq!(to_uint32(value), uint32, "ToUint32({value:?})");
        }
    }

    #[test]
    fn literal_parser_handles_js_bases_and_wide_prefixed_values() {
        assert_eq!(parse_numeric_literal("0x10"), Some(16.0));
        assert_eq!(parse_numeric_literal("1_000"), Some(1000.0));
        // Legacy sloppy octal decodes to its JavaScript value, not base-10.
        assert_eq!(parse_numeric_literal("010"), Some(8.0));
        assert_eq!(parse_numeric_literal("0o10"), Some(8.0));
        assert_eq!(parse_numeric_literal("0b101"), Some(5.0));
        // Prefixed literals wider than u64 keep the exact binary64 value.
        let power_of_two = 18_446_744_073_709_551_616.0f64; // 2^64
        assert_eq!(
            parse_numeric_literal("0x10000000000000000"),
            Some(power_of_two)
        );
        let binary = format!("0b1{}", "0".repeat(64));
        assert_eq!(parse_numeric_literal(&binary), Some(power_of_two));
        assert_eq!(parse_numeric_literal("1e3"), Some(1000.0));
        // Signed zero keeps its bit pattern, and signs may be separated by
        // whitespace just as in the pre-refactor helper.
        assert_eq!(
            parse_numeric_literal("-0").map(f64::to_bits),
            Some((-0.0f64).to_bits())
        );
        assert_eq!(parse_numeric_literal(" - 0x10 "), Some(-16.0));
        // Canonical special spellings are explicit, never arbitrary source.
        assert!(parse_numeric_literal("NaN").is_some_and(f64::is_nan));
        assert!(parse_numeric_literal("-NaN").is_some_and(f64::is_nan));
        assert_eq!(parse_numeric_literal("Infinity"), Some(f64::INFINITY));
        assert_eq!(parse_numeric_literal("+Infinity"), Some(f64::INFINITY));
        assert_eq!(parse_numeric_literal("-Infinity"), Some(f64::NEG_INFINITY));
    }

    #[test]
    fn literal_parser_rejects_non_literal_forms() {
        // Invalid manual forms fail closed instead of parsing partially or
        // evaluating an expression.
        for text in [
            "",
            " ",
            "0x",
            "12abc",
            "1+1",
            "--1",
            "+-1",
            "1 2",
            "Infinityx",
            "nan",
            "1_",
            "_1",
            "0b2",
            "0o8",
            "1e",
            "0x1p2",
            "1n",
            "true",
            "'1'",
            "(1)",
            "1;2",
        ] {
            assert_eq!(
                parse_numeric_literal(text),
                None,
                "must reject non-literal {text:?}"
            );
        }
    }

    #[test]
    fn string_to_number_uses_js_whitespace_and_radix_rules() {
        for (text, expected) in [
            ("0", 0.0),
            (" 42 ", 42.0),
            ("\u{feff}\u{a0}42\u{a0}\n", 42.0),
            ("0x10", 16.0),
            ("010", 10.0),
            ("1e3", 1000.0),
            (".5", 0.5),
            ("5.", 5.0),
            ("Infinity", f64::INFINITY),
            ("+Infinity", f64::INFINITY),
            ("-Infinity", f64::NEG_INFINITY),
        ] {
            assert_eq!(string_to_number(text), expected, "StringToNumber({text:?})");
        }
        assert_eq!(string_to_number("\u{feff}\u{a0}\t\n"), 0.0);
        for text in [
            "inf", "infinity", "INF", "NaN", "0x", "12abc", "1+1", "1_000",
        ] {
            assert!(string_to_number(text).is_nan(), "StringToNumber({text:?})");
        }
        assert_eq!(
            string_to_number("0x10000000000000000"),
            18_446_744_073_709_551_616.0
        );
    }

    #[test]
    fn number_to_js_string_matches_ecma_262_number_to_string() {
        for (value, expected) in [
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
            (0.0, "0"),
            (-0.0, "0"),
            (1.5, "1.5"),
            (0.1, "0.1"),
            (1e21, "1e+21"),
            (-1e21, "-1e+21"),
            (1e-7, "1e-7"),
            (1e-6, "0.000001"),
        ] {
            assert_eq!(
                number_to_js_string(value),
                expected,
                "Number::toString({value:?})"
            );
        }
    }
}
