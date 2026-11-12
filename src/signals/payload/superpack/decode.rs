use super::*;
use std::collections::HashSet;

struct Decoder<'a> {
    bytes: &'a [u8],
    pos: usize,
    options: &'a SerializerOptions,
    strings: Vec<String>,
    keysets: Vec<Vec<String>>,
    entries: usize,
    storage: usize,
}
pub(super) fn unpack(bytes: &[u8], options: &SerializerOptions) -> Result<Value, Error> {
    if bytes.len() > options.limits.max_bytes {
        return Err(err("input exceeds byte limit"));
    }
    let mut d = Decoder {
        bytes,
        pos: 0,
        options,
        strings: vec![],
        keysets: options.keysets_to_omit.clone(),
        entries: 0,
        storage: 0,
    };
    if bytes.first() == Some(&254) {
        d.pos += 1;
        let n = d.byte()? as usize;
        // LUT entries are encoded with stringPlaceholders=false: no STRREFs.
        let mut strings = vec![];
        for _ in 0..n {
            match d.value(0)? {
                Value::String(s) => strings.push(s),
                _ => return Err(err("LUT entry must be a literal string")),
            }
        }
        d.strings = strings;
        let metadata = d.value(0)?;
        let Value::Array(sets) = metadata else {
            return Err(err(
                "unsupported/truncated keyset metadata (source depth bound may have truncated it)",
            ));
        };
        for set in sets {
            let Value::Array(keys) = set else {
                return Err(err("unsupported/truncated keyset entry"));
            };
            let mut result = vec![];
            let mut seen = HashSet::new();
            for key in keys {
                let Value::String(key) = key else {
                    return Err(err("keyset key must be a string"));
                };
                if !seen.insert(key.clone()) {
                    return Err(err("duplicate keyset key"));
                }
                result.push(key);
            }
            d.keysets.push(result);
            if d.keysets.len() > options.limits.max_entries {
                return Err(err("keyset count limit exceeded"));
            }
        }
    }
    let value = d.value(0)?;
    if d.pos != bytes.len() {
        return Err(err("trailing bytes"));
    }
    Ok(value)
}
impl Decoder<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], Error> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| err("length overflow"))?;
        let result = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| err("truncated input"))?;
        self.pos = end;
        Ok(result)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn be(&mut self, n: usize) -> Result<u64, Error> {
        Ok(self.take(n)?.iter().fold(0, |a, b| a << 8 | *b as u64))
    }
    fn unsigned_tag(&mut self, t: u8) -> Result<u64, Error> {
        match t {
            0..=63 => Ok(t as u64),
            64..=127 => Ok(((t as u64 & 63) << 8) | self.byte()? as u64),
            228 => self.be(2),
            229 => self.be(3),
            230 => self.be(4),
            231 => self.be(8),
            _ => Err(err("expected type-tagged unsigned integer")),
        }
    }
    fn unsigned(&mut self) -> Result<u64, Error> {
        let t = self.byte()?;
        self.unsigned_tag(t)
    }
    fn count(&mut self) -> Result<usize, Error> {
        let n = self.unsigned()?;
        if n > self.options.limits.max_entries as u64 {
            return Err(err("count exceeds entry limit"));
        }
        Ok(n as usize)
    }
    fn charge(&mut self, n: usize) -> Result<(), Error> {
        self.entries = self.entries.saturating_add(n);
        if self.entries > self.options.limits.max_entries {
            return Err(err("entry limit exceeded"));
        }
        Ok(())
    }
    fn storage(&mut self, n: usize) -> Result<(), Error> {
        self.storage = self.storage.saturating_add(n);
        if self.storage > self.options.limits.max_bytes {
            return Err(err("decoded storage exceeds byte limit"));
        }
        Ok(())
    }
    fn text(&mut self, n: usize) -> Result<Value, Error> {
        if n > self.options.limits.max_bytes {
            return Err(err("string length exceeds byte limit"));
        }
        self.storage(n)?;
        let s = std::str::from_utf8(self.take(n)?)
            .map_err(|_| err("invalid UTF-8"))?
            .to_owned();
        Ok(Value::String(s))
    }
    fn booleans(&mut self, n: usize) -> Result<Vec<Value>, Error> {
        self.charge(n)?;
        let bytes = self.take(n.div_ceil(8))?;
        Ok((0..n)
            .map(|i| Value::Bool(bytes[i / 8] & (128 >> (i % 8)) != 0))
            .collect())
    }
    fn array(&mut self, n: usize, depth: usize) -> Result<Value, Error> {
        if n > self.options.limits.max_entries || n > self.bytes.len().saturating_sub(self.pos) {
            return Err(err("array count exceeds limit or remaining input"));
        }
        let mut values = vec![];
        for _ in 0..n {
            values.push(self.value(depth + 1)?);
        }
        Ok(Value::Array(values))
    }
    fn integer(n: u64, negative: bool) -> Result<Value, Error> {
        // JS cannot preserve arbitrary 64-bit integers; reject lossy values.
        if n >= 18446744073709549568 || (n as f64) as u64 != n {
            return Err(err(
                "integer is not exactly representable in source number domain",
            ));
        }
        Ok(Value::from(if negative { -(n as f64) } else { n as f64 }))
    }
    fn value(&mut self, depth: usize) -> Result<Value, Error> {
        if depth > self.options.limits.max_depth {
            return Err(err("decode depth limit exceeded"));
        }
        self.charge(1)?;
        let t = self.byte()?;
        match t {
            0..=127 | 228..=231 => {
                let n = self.unsigned_tag(t)?;
                Self::integer(n, false)
            }
            128..=143 => Self::integer((t & 15) as u64, true),
            144..=159 => Ok(Value::Array(self.booleans((t & 15) as usize)?)),
            160..=191 => self.array((t & 31) as usize, depth),
            192..=223 => self.text((t & 31) as usize),
            224 => Ok(Value::Bool(false)),
            225 => Ok(Value::Bool(true)),
            226 => Ok(Value::Null),
            227 => Ok(Value::Undefined),
            232..=235 => {
                let n = self.be(match t {
                    232 => 1,
                    233 => 2,
                    234 => 4,
                    _ => 8,
                })?;
                Self::integer(n, true)
            }
            236 => {
                let bits = self.be(4)? as u32;
                Ok(Value::from(f32::from_bits(bits) as f64))
            }
            237 => {
                let bits = self.be(8)?;
                Ok(Value::Number(Number(bits)))
            }
            238 => Ok(Value::Date(self.be(6)?)),
            239 => {
                let n = self.count()?;
                if n > self.bytes.len().saturating_sub(self.pos) {
                    return Err(err("truncated binary"));
                }
                self.storage(n)?;
                let mut bytes = vec![];
                for _ in 0..n {
                    // Every byte uses generic integer encoding in the source.
                    let v = self.unsigned()?;
                    if v > 255 {
                        return Err(err("binary element is not an unsigned byte"));
                    }
                    self.charge(1)?;
                    bytes.push(v as u8);
                }
                Ok(Value::Binary(bytes))
            }
            240 => {
                let n = self.bytes[self.pos..]
                    .iter()
                    .position(|b| *b == 0)
                    .ok_or_else(|| err("unterminated CSTRING"))?;
                let s = self.text(n)?;
                self.pos += 1;
                Ok(s)
            }
            241 => {
                let n = self.byte()? as usize;
                self.text(n)
            }
            242 => {
                let n = self.unsigned()?;
                if n > self.options.limits.max_bytes as u64 {
                    return Err(err("string length exceeds limit"));
                }
                self.text(n as usize)
            }
            243 => {
                let i = self.byte()? as usize;
                let s = self
                    .strings
                    .get(i)
                    .ok_or_else(|| err("invalid string reference"))?;
                let n = s.len();
                self.storage(n)?;
                Ok(Value::String(self.strings[i].clone()))
            }
            244 => {
                let n = self.byte()? as usize;
                self.array(n, depth)
            }
            245 => {
                let n = self.count()?;
                self.array(n, depth)
            }
            246 => {
                let n = self.byte()? as usize;
                Ok(Value::Array(self.booleans(n)?))
            }
            247 => {
                let n = self.count()?;
                Ok(Value::Array(self.booleans(n)?))
            }
            248 | 249 => {
                let i = self.count()?;
                let keys = self.keysets.get(i).ok_or_else(|| {
                    err("invalid keyset reference (supply keysets_to_omit if omitted)")
                })?;
                let n = keys.len();
                let size = keys.iter().map(String::len).sum();
                self.storage(size)?;
                let keys = self.keysets[i].clone();
                let values = if t == 249 {
                    self.booleans(n)?
                } else {
                    let mut a = vec![];
                    for _ in 0..n {
                        a.push(self.value(depth + 1)?);
                    }
                    a
                };
                Ok(Value::Object(keys.into_iter().zip(values).collect()))
            }
            255 => {
                let id = self.unsigned()?;
                if id > u32::MAX as u64 {
                    return Err(err("extension id overflow"));
                }
                let payload = self.value(depth + 1)?;
                match (id as u32, &payload) {
                    (0, Value::Undefined) if self.options.missing_sentinel_extension => {
                        Ok(Value::MissingSentinel)
                    }
                    (DEPTH_EXTENSION, Value::Null) if self.options.depth_bound.is_some() => {
                        Ok(Value::Extension {
                            id: id as u32,
                            payload: Box::new(payload),
                        })
                    }
                    (0, _) | (DEPTH_EXTENSION, _) => {
                        Err(err("known extension disabled or invalid payload"))
                    }
                    _ if self.options.opaque_extensions => Ok(Value::Extension {
                        id: id as u32,
                        payload: Box::new(payload),
                    }),
                    _ => Err(err("opaque extension disabled")),
                }
            }
            250 | 251 => Err(err(
                "unsupported MAPL/BMAPL: declared but never emitted in supplied sources",
            )),
            _ => Err(err("unsupported tag (including nested STRLUT)")),
        }
    }
}
