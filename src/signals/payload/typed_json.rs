//! Bounded typed JSON parsing for generator inputs (pack requests, reports).

use std::io;

// A typed object layer costs three JSON containers (tag, entries, key/value
// tuple). 1024 covers the core's depth-256 ceiling, including leaf wrappers.
// Only gen pack and report input opt out of serde_json's default recursion limit. The binary
// runs on run_with_large_stack; preflight bounds parsing, error cleanup and drop
// before serde allocates recursive values. All other parsers retain defaults.
const MAX_TYPED_JSON_DEPTH: usize = 1024;

pub fn parse_typed_json<T: serde::de::DeserializeOwned>(
    input: &[u8],
) -> Result<T, Box<dyn std::error::Error>> {
    check_typed_json_depth(input)?;
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    deserializer.disable_recursion_limit();
    let request = serde::Deserialize::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(request)
}

fn check_typed_json_depth(input: &[u8]) -> io::Result<()> {
    let mut stack = [0u8; MAX_TYPED_JSON_DEPTH];
    let mut depth = 0;
    let mut quoted = false;
    let mut escaped = false;
    for &byte in input {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match byte {
            b'"' => quoted = true,
            b'{' | b'[' => {
                if depth == MAX_TYPED_JSON_DEPTH {
                    return Err(invalid(format!(
                        "JSON nesting exceeds {MAX_TYPED_JSON_DEPTH} structural levels"
                    )));
                }
                stack[depth] = byte;
                depth += 1;
            }
            b'}' | b']' => {
                let opening = if byte == b'}' { b'{' } else { b'[' };
                if depth == 0 || stack[depth - 1] != opening {
                    return Err(invalid("mismatched JSON container"));
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    if quoted || depth != 0 {
        return Err(invalid("unterminated JSON string or container"));
    }
    // Syntax, string escapes, duplicate/unknown fields, and exact consumption
    // remain the typed deserializer's responsibility.
    Ok(())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
