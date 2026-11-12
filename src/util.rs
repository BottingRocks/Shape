//! Small crate-internal encoding primitives shared by report serializers.

/// Return the lowercase, fixed-width hexadecimal FNV-1a-64 digest.
pub fn fnv1a64_hex(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

/// Encode a string as a JSON string literal using the crate's compact format.
pub(crate) fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if character.is_control() => {
                out.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => out.push(character),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::{fnv1a64_hex, json_string};

    #[test]
    fn fnv1a64_hex_matches_known_vectors() {
        assert_eq!(fnv1a64_hex(b""), "cbf29ce484222325");
        assert_eq!(fnv1a64_hex(b"a"), "af63dc4c8601ec8c");
    }

    #[test]
    fn json_string_escapes_unicode_controls() {
        assert_eq!(
            json_string("quote\" slash\\\n\r\t\0\u{007f}\u{0085} snowman ☃"),
            "\"quote\\\" slash\\\\\\n\\r\\t\\u0000\\u007f\\u0085 snowman ☃\""
        );
    }
}
