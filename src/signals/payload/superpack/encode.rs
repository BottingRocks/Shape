use super::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
enum Token {
    Byte(u8),
    String(String),
}
struct Encoder<'a> {
    options: &'a SerializerOptions,
    keysets: Vec<Vec<String>>,
    key_indices: HashMap<Vec<String>, usize>,
    hist: Vec<(String, usize)>,
    hist_indices: HashMap<String, usize>,
    nodes: usize,
    storage: usize,
}

pub(super) fn pack(value: &Value, options: &SerializerOptions) -> Result<Vec<u8>, Error> {
    let mut e = Encoder {
        options,
        keysets: options.keysets_to_omit.clone(),
        key_indices: HashMap::new(),
        hist: vec![],
        hist_indices: HashMap::new(),
        nodes: 0,
        storage: 0,
    };
    for (i, keys) in e.keysets.iter().enumerate() {
        e.key_indices.entry(keys.clone()).or_insert(i);
    }
    let mut body = vec![];
    e.value(value, &mut body, 0, options.depth_bound)?;
    // Source removes preloaded keysets before encoding the keyset table itself.
    let new_keys: Vec<Value> = e.keysets[options.keysets_to_omit.len()..]
        .iter()
        .map(|k| Value::Array(k.iter().cloned().map(Value::String).collect()))
        .collect();
    let has_new_keys = !new_keys.is_empty();
    let mut metadata = vec![];
    e.value(
        &Value::Array(new_keys),
        &mut metadata,
        0,
        options.depth_bound,
    )?;
    let mut candidates: Vec<_> = e
        .hist
        .iter()
        .enumerate()
        .filter(|(_, (s, n))| *n >= 2 && s.encode_utf16().count().saturating_mul(*n) >= 8)
        .collect();
    // Object.keys histogram enumeration puts array-index properties first.
    candidates.sort_by(
        |(ia, (a, _)), (ib, (b, _))| match (index_key(a), index_key(b)) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            _ => ia.cmp(ib),
        },
    );
    candidates.sort_by_key(|(_, (s, n))| {
        let l = s.encode_utf16().count() as i128;
        -((l + 1) * *n as i128 - (l + 1 + *n as i128 * 2))
    });
    let lut: Vec<String> = candidates
        .into_iter()
        .take(255)
        .map(|(_, (s, _))| s.clone())
        .collect();
    let lookup: HashMap<&str, u8> = lut
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i as u8))
        .collect();
    let mut out = vec![];
    if !lut.is_empty() || has_new_keys {
        out.extend([254, lut.len() as u8]);
        for s in &lut {
            write_string(s, &mut out);
        }
        resolve(&metadata, &lookup, &mut out, options.limits.max_bytes)?;
    }
    resolve(&body, &lookup, &mut out, options.limits.max_bytes)?;
    if out.len() > options.limits.max_bytes {
        return Err(err("output exceeds byte limit"));
    }
    Ok(out)
}
fn resolve(
    tokens: &[Token],
    lut: &HashMap<&str, u8>,
    out: &mut Vec<u8>,
    limit: usize,
) -> Result<(), Error> {
    for t in tokens {
        match t {
            Token::Byte(b) => out.push(*b),
            Token::String(s) => {
                if let Some(i) = lut.get(s.as_str()) {
                    out.extend([243, *i]);
                } else {
                    write_string(s, out);
                }
            }
        }
        if out.len() > limit {
            return Err(err("output exceeds byte limit"));
        }
    }
    Ok(())
}
fn write_string(s: &str, out: &mut Vec<u8>) {
    let n = s.len();
    if n < 32 {
        out.push(192 | n as u8);
    } else if !s.as_bytes().contains(&0) {
        out.push(240);
    } else if n <= 255 {
        out.extend([241, n as u8]);
    } else {
        out.push(242);
        uint(n as u64, out);
    }
    out.extend_from_slice(s.as_bytes());
    if n >= 32 && !s.as_bytes().contains(&0) {
        out.push(0);
    }
}
pub(super) fn uint(n: u64, out: &mut Vec<u8>) {
    match n {
        0..=63 => out.push(n as u8),
        64..=16383 => out.extend([64 | (n >> 8) as u8, n as u8]),
        16384..=65535 => {
            out.push(228);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
        65536..=16777215 => {
            out.push(229);
            out.extend_from_slice(&n.to_be_bytes()[5..]);
        }
        16777216..=4294967295 => {
            out.push(230);
            out.extend_from_slice(&(n as u32).to_be_bytes());
        }
        _ => {
            out.push(231);
            out.extend_from_slice(&n.to_be_bytes());
        }
    }
}
fn number(v: f64, out: &mut Vec<u8>) {
    let a = v.abs();
    if v.is_finite()
        && a.floor() == a
        && a < 18446744073709549568.0
        && (a != 0.0 || !v.is_sign_negative())
    {
        if v >= 0.0 {
            uint(v as u64, out);
        } else {
            let n = a as u64;
            match n {
                0..=15 => out.push(128 | n as u8),
                16..=255 => out.extend([232, n as u8]),
                256..=65535 => {
                    out.push(233);
                    out.extend_from_slice(&(n as u16).to_be_bytes());
                }
                65536..=4294967295 => {
                    out.push(234);
                    out.extend_from_slice(&(n as u32).to_be_bytes());
                }
                _ => {
                    out.push(235);
                    out.extend_from_slice(&n.to_be_bytes());
                }
            }
        }
    } else if v.is_nan() {
        out.extend([236, 127, 192, 0, 0]);
    } else if (v as f32) as f64 == v {
        out.push(236);
        out.extend_from_slice(&(v as f32).to_be_bytes());
    } else {
        out.push(237);
        out.extend_from_slice(&v.to_be_bytes());
    }
}
fn bits(values: impl Iterator<Item = bool>, out: &mut Vec<u8>) {
    let mut byte = 0;
    let mut n = 0;
    for b in values {
        byte |= u8::from(b) << (7 - n);
        n += 1;
        if n == 8 {
            out.push(byte);
            byte = 0;
            n = 0;
        }
    }
    if n > 0 {
        out.push(byte);
    }
}
impl Encoder<'_> {
    fn value(
        &mut self,
        value: &Value,
        out: &mut Vec<Token>,
        depth: usize,
        remaining: Option<usize>,
    ) -> Result<(), Error> {
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > self.options.limits.max_entries || depth > self.options.limits.max_depth {
            return Err(err("entry/depth limit exceeded"));
        }
        let mut b = vec![];
        match value {
            Value::Undefined => b.push(227),
            Value::Null => b.push(226),
            Value::Bool(v) => b.push(if *v { 225 } else { 224 }),
            Value::Number(v) => number(v.as_f64(), &mut b),
            Value::String(s) => {
                self.storage = self.storage.saturating_add(s.len());
                if self.storage > self.options.limits.max_bytes {
                    return Err(err("string storage exceeds byte limit"));
                }
                if !inherited_hist_key(s) {
                    if let Some(i) = self.hist_indices.get(s) {
                        self.hist[*i].1 += 1;
                    } else {
                        self.hist_indices.insert(s.clone(), self.hist.len());
                        self.hist.push((s.clone(), 1));
                    }
                }
                out.push(Token::String(s.clone()));
            }
            Value::Date(ms) => {
                if *ms >= 1u64 << 48 {
                    return Err(err(
                        "unsupported Date: require nonnegative integer milliseconds below 2^48",
                    ));
                }
                b.push(238);
                b.extend_from_slice(&ms.to_be_bytes()[2..]);
            }
            Value::Binary(bytes) => {
                if bytes.len() > self.options.limits.max_entries
                    || bytes.len() > self.options.limits.max_bytes
                {
                    return Err(err("binary count exceeds limit"));
                }
                // Source dispatches both length and bytes through encodeValue.
                out.push(Token::Byte(239));
                self.value(&Value::from(bytes.len() as f64), out, depth + 1, remaining)?;
                for byte in bytes {
                    self.value(&Value::from(*byte as f64), out, depth + 1, remaining)?;
                }
            }
            Value::MissingSentinel => {
                if !self.options.missing_sentinel_extension {
                    return Err(err("missing sentinel extension is disabled"));
                }
                out.extend([Token::Byte(255), Token::Byte(0)]);
                return self.value(&Value::Undefined, out, depth + 1, remaining);
            }
            Value::Extension { id, payload } => {
                match (*id, payload.as_ref()) {
                    (0, Value::Undefined) if self.options.missing_sentinel_extension => (),
                    (DEPTH_EXTENSION, Value::Null) if self.options.depth_bound.is_some() => (),
                    (0, _) | (DEPTH_EXTENSION, _) => {
                        return Err(err("known extension disabled or invalid payload"))
                    }
                    _ if self.options.opaque_extensions => (),
                    _ => {
                        return Err(err(
                            "opaque extensions disabled; callback registry unsupported",
                        ))
                    }
                }
                b.push(255);
                uint(*id as u64, &mut b);
                out.extend(b.into_iter().map(Token::Byte));
                return self.value(payload, out, depth + 1, remaining);
            }
            Value::Array(_) | Value::Object(_) => {
                if remaining == Some(0) {
                    b.push(255);
                    uint(DEPTH_EXTENSION as u64, &mut b);
                    out.extend(b.into_iter().map(Token::Byte));
                    return self.value(&Value::Null, out, depth + 1, remaining);
                } else {
                    let next = remaining.map(|r| r - 1);
                    match value {
                        Value::Array(a) => {
                            if a.len() > self.options.limits.max_entries {
                                return Err(err("array count exceeds limit"));
                            }
                            let boolean =
                                !a.is_empty() && a.iter().all(|v| matches!(v, Value::Bool(_)));
                            if boolean {
                                match a.len() {
                                    1..=15 => b.push(144 | a.len() as u8),
                                    16..=255 => b.extend([246, a.len() as u8]),
                                    _ => {
                                        b.push(247);
                                        uint(a.len() as u64, &mut b);
                                    }
                                }
                                bits(a.iter().map(|v| matches!(v, Value::Bool(true))), &mut b);
                                self.nodes = self.nodes.saturating_add(a.len());
                            } else {
                                match a.len() {
                                    0..=31 => b.push(160 | a.len() as u8),
                                    32..=255 => b.extend([244, a.len() as u8]),
                                    _ => {
                                        b.push(245);
                                        uint(a.len() as u64, &mut b);
                                    }
                                }
                                out.extend(b.drain(..).map(Token::Byte));
                                for v in a {
                                    self.value(v, out, depth + 1, next)?;
                                }
                            }
                        }
                        Value::Object(entries) => {
                            if entries.len() > self.options.limits.max_entries {
                                return Err(err("map count exceeds limit"));
                            }
                            for (key, _) in entries {
                                self.storage = self.storage.saturating_add(key.len());
                                if self.storage > self.options.limits.max_bytes {
                                    return Err(err("object key storage exceeds byte limit"));
                                }
                            }
                            let mut sorted: Vec<_> = entries.iter().collect();
                            sorted.sort_by(|a, b| utf16_cmp(&a.0, &b.0));
                            let mut seen = HashSet::new();
                            for (k, _) in &sorted {
                                if !seen.insert(k) {
                                    return Err(err("duplicate object key"));
                                }
                            }
                            let keys: Vec<_> = sorted.iter().map(|(k, _)| k.clone()).collect();
                            let i = if let Some(i) = self.key_indices.get(&keys) {
                                *i
                            } else {
                                let i = self.keysets.len();
                                self.key_indices.insert(keys.clone(), i);
                                self.keysets.push(keys);
                                i
                            };
                            let boolean = sorted.iter().all(|(_, v)| matches!(v, Value::Bool(_)));
                            b.push(if boolean { 249 } else { 248 });
                            uint(i as u64, &mut b);
                            if boolean {
                                bits(
                                    sorted.iter().map(|(_, v)| matches!(v, Value::Bool(true))),
                                    &mut b,
                                );
                                self.nodes = self.nodes.saturating_add(sorted.len());
                            } else {
                                out.extend(b.drain(..).map(Token::Byte));
                                for (_, v) in sorted {
                                    self.value(v, out, depth + 1, next)?;
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        if self.nodes > self.options.limits.max_entries {
            return Err(err("entry limit exceeded"));
        }
        self.storage = self.storage.saturating_add(b.len());
        out.extend(b.into_iter().map(Token::Byte));
        if self.storage > self.options.limits.max_bytes || out.len() > self.options.limits.max_bytes
        {
            return Err(err("encoding storage exceeds byte limit"));
        }
        Ok(())
    }
}
