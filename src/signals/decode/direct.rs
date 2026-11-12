//! Shared compiler for the opt-in explicit source-format adapter.
pub fn parse_salt(s: &str) -> Option<i64> {
    let s = s.rsplit('^').next()?.trim().trim_end_matches(')').trim();
    s.parse::<i64>()
        .ok()
        .or_else(|| {
            // JavaScript may spell the salt in decimal exponent form, e.g.
            // 25312e3 for 25312000.
            s.parse::<f64>()
                .ok()
                .filter(|f| f.is_finite() && f.fract() == 0.0)
                .map(|f| f as i64)
        })
        .or_else(|| {
            let hex = s.strip_prefix("0x").unwrap_or(s);
            i64::from_str_radix(hex, 16).ok()
        })
}
use crate::signals::{
    extract::contracts::{EmissionContract, GrammarNode as G},
    schema::SourceSpan,
    wire::{explicit_initializer as dc, *},
};
use serde_json::{json, Value};
fn err(s: impl Into<String>) -> WireError {
    WireError::Unsupported(s.into())
}
fn direct_mask(form: crate::signals::extract::contracts::MaskForm) -> Result<WireMask, WireError> {
    match form {
        crate::signals::extract::contracts::MaskForm::Stream => Ok(WireMask::Inherit),
        crate::signals::extract::contracts::MaskForm::Unmasked
        | crate::signals::extract::contracts::MaskForm::StaticXor => Ok(WireMask::None),
        _ => Err(err("direct mask unresolved")),
    }
}
fn empty(o: &Op) -> bool {
    match o {
        Op::Sequence { items } => items.iter().all(empty),
        Op::DomainChoice { arms, .. } | Op::Choice { arms, .. } => arms.iter().any(empty),
        _ => false,
    }
}
fn domain(o: &Op) -> Option<(Vec<u8>, WireMask)> {
    match o {
        Op::Byte {
            mask,
            xor,
            allowed: Some(a),
        } => Some((a.iter().map(|x| x ^ xor).collect(), mask.clone())),
        Op::Number { mask, .. } => Some(((0..=128).collect(), mask.clone())),
        Op::String { mask, .. } => Some((
            (0..=127).chain([129, 192]).chain(194..=244).collect(),
            mask.clone(),
        )),
        Op::Sequence { items } => {
            let mut out = vec![];
            let mut mask = None;
            for i in items {
                if let Some((d, m)) = domain(i) {
                    if mask.as_ref().is_some_and(|x| *x != m) {
                        return None;
                    }
                    mask = Some(m);
                    out.extend(d)
                }
                if !empty(i) {
                    break;
                }
            }
            mask.map(|m| (out, m))
        }
        Op::DomainChoice { arms, mask, .. } | Op::Choice { arms, mask, .. } => Some((
            arms.iter().filter_map(domain).flat_map(|x| x.0).collect(),
            mask.clone(),
        )),
        _ => None,
    }
}
pub fn compile(
    n: &G,
    e: &EmissionContract,
    a: &AlgorithmReport,
    p: &SourcePlan,
    source: &str,
    path: Vec<usize>,
    last: Option<(Vec<usize>, SourceSpan)>,
    facts: &mut Vec<Value>,
    context: &CompileContext,
) -> Result<Op, WireError> {
    match n {
        G::Sequence { items } => {
            let mut out = vec![];
            let mut prev = last;
            for (i, n) in items.iter().enumerate() {
                if matches!(n, G::RawPrefix(_)) {
                    continue;
                }
                let mut q = path.clone();
                q.push(i);
                let op = compile(n, e, a, p, source, q.clone(), prev.clone(), facts, context)?;
                out.push(op);
                if let G::Number(w) = n {
                    prev = Some((q, w.span.into()))
                }
            }
            Ok(Op::Sequence { items: out })
        }
        G::Repeat {
            body,
            count,
            direction,
            span,
            ..
        } => {
            let mut q = path.clone();
            q.push(0);
            // The legacy fallback compiles the body before checking the count.
            // Preserve its refusal reason (part of saved schema JSON) by keeping
            // that order when direct mode is disabled.
            let legacy_body = if !context.direct {
                Some(compile(
                    body,
                    e,
                    a,
                    p,
                    source,
                    q.clone(),
                    None,
                    facts,
                    context,
                )?)
            } else {
                None
            };
            let rp = control_path(e, n);
            let mut reference = None;
            if count.is_none() {
                let (countpath, write) = last.ok_or_else(|| err("repeat lacks preceding count"))?;
                let proven = context.counts.iter().any(|c| {
                    c["loop"] == json!([span.start, span.end])
                        && c["countWrite"] == json!([write.start, write.end])
                        && c["proven"] == true
                });
                if !context.direct && !proven {
                    return Err(err("repeat has no source-connected count write/test"));
                }
                let r = p
                    .counts
                    .iter()
                    .find(|r| r.loop_span.start == span.start)
                    .cloned()
                    .unwrap_or(CountRef {
                        path: countpath.clone(),
                        repeat_path: rp,
                        write,
                        collection_binding: write,
                        counter_binding: (*span).into(),
                        update: (*span).into(),
                        update_in_body: true,
                        predecrement_exit: None,
                        length_evaluation: write,
                        same_cached_evaluation: false,
                        loop_span: (*span).into(),
                        direction: *direction,
                        maximum: u32::MAX as usize,
                        reject_negative_zero: true,
                    });
                facts.push(json!({
                    "loop":[span.start,span.end],
                    "countWrite":write,
                    "countPath":countpath,
                    "kind": if proven { "source_ordered_prior_count" } else { "runtime_prior_count" }
                }));
                context.loops.borrow_mut().push(r.clone());
                reference = Some(r);
            }
            let op = match legacy_body {
                Some(op) => op,
                None => compile(body, e, a, p, source, q, None, facts, context)?,
            };
            if let Some(c) = count {
                return Ok(Op::Repeat {
                    count: *c as usize,
                    direction: *direction,
                    body: Box::new(op),
                });
            }
            let reference = reference.expect("count reference");
            context.loops.borrow_mut().pop();
            context.last_count.replace(Some(reference.clone()));
            Ok(Op::CountedRepeat {
                count: CountExpr::PriorNumber { reference },
                direction: *direction,
                body: Box::new(op),
            })
        }
        G::Choice {
            arms,
            span,
            condition,
            ..
        } => {
            let has_counter_proof = p
                .counter_choices
                .iter()
                .any(|proof| proof.choice.start == span.start && proof.choice.end == span.end);
            if context.direct && !has_counter_proof {
                if let Some(op) = packed_counter_choice(
                    n,
                    arms,
                    (*span).into(),
                    condition,
                    e,
                    a,
                    p,
                    source,
                    &path,
                    last.clone(),
                    facts,
                    context,
                )? {
                    return Ok(op);
                }
            }
            // Prefer the strict node when it succeeds. Only fall back to the
            // direct arm compiler when the strict helper refuses (for example
            // a root choice whose string writer is uncertified).
            if let Ok(op) = dc::node(n, e, a, p, source) {
                return Ok(op);
            }
            let mut ops = vec![];
            for (i, arm) in arms.iter().enumerate() {
                let mut q = path.clone();
                q.push(i);
                ops.push(compile(
                    &arm.body,
                    e,
                    a,
                    p,
                    source,
                    q,
                    last.clone(),
                    facts,
                    context,
                )?)
            }
            let mut heads: Vec<Option<(Vec<u8>, WireMask)>> = ops.iter().map(domain).collect();
            let empties: Vec<_> = ops
                .iter()
                .enumerate()
                .filter(|(_, o)| empty(o))
                .map(|(i, _)| i)
                .collect();
            // A dynamic value byte plus an empty arm has no tag domain: the
            // byte is the payload, not a discriminant. Select the non-empty
            // arm for any first byte and leave the empty arm with no domain.
            let mut dynamic_optional = false;
            if context.direct && heads.iter().flatten().next().is_none() && empties.len() == 1 {
                let nonempty: Vec<usize> = (0..ops.len()).filter(|i| !empty(&ops[*i])).collect();
                if nonempty.len() == 1 {
                    let mask = WireMask::Inherit;
                    heads[nonempty[0]] = Some(((0..=255).collect(), mask.clone()));
                    heads[empties[0]] = Some((Vec::new(), mask));
                    facts.push(json!({"optionalWrite":[span.start,span.end],"emptyArm":empties[0],"dynamicValueArm":nonempty[0],"kind":"collector_null_tag_or_no_write"}));
                    dynamic_optional = true;
                }
            }
            if !dynamic_optional && empties.len() == 1 {
                let used: Vec<u8> = heads
                    .iter()
                    .flatten()
                    .flat_map(|(d, _)| d.clone())
                    .collect();
                let mask = heads
                    .iter()
                    .flatten()
                    .next()
                    .ok_or_else(|| {
                        err(if context.direct {
                            format!("empty choice at {}", span.start)
                        } else {
                            "empty choice".into()
                        })
                    })?
                    .1
                    .clone();
                let mut d = heads[empties[0]]
                    .as_ref()
                    .map(|x| x.0.clone())
                    .unwrap_or_default();
                d.extend((0..=255).filter(|b| !used.contains(b)));
                heads[empties[0]] = Some((d, mask));
                facts.push(json!({"optionalWrite":[span.start,span.end],"emptyArm":empties[0],"kind":"collector_null_tag_or_no_write"}));
            }
            if !dynamic_optional && empties.len() > 1 {
                let used: Vec<u8> = heads
                    .iter()
                    .flatten()
                    .flat_map(|(d, _)| d.clone())
                    .collect();
                let mask = heads
                    .iter()
                    .flatten()
                    .next()
                    .ok_or_else(|| {
                        err(if context.direct {
                            format!("all-empty choice at {}", span.start)
                        } else {
                            "all-empty choice".into()
                        })
                    })?
                    .1
                    .clone();
                let fallback = empties[0];
                for i in &empties {
                    if heads[*i].is_none() {
                        heads[*i] = Some((vec![], mask.clone()))
                    }
                }
                heads[fallback]
                    .as_mut()
                    .unwrap()
                    .0
                    .extend((0..=255).filter(|b| !used.contains(b)));
                facts.push(json!({"noWriteEquivalentSourceArms":empties,"choice":[span.start,span.end],"canonicalEmptyArm":fallback}));
            }
            let heads = heads
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| err(format!("choice head unresolved at {}", span.start)))?;
            let mask = heads[0].1.clone();
            let mut seen = vec![];
            for (d, m) in &heads {
                if *m != mask || d.iter().any(|b| seen.contains(b)) {
                    return Err(err(format!("choice domain overlap at {}", span.start)));
                }
                seen.extend(d)
            }
            Ok(Op::DomainChoice {
                source: (*span).into(),
                path: control_path(e, n),
                consumption: TagConsumption::ArmOwnsTag,
                semantics: None,
                domains: heads.into_iter().map(|x| x.0).collect(),
                mask,
                arms: ops,
            })
        }
        G::Byte(w) | G::Number(w) | G::String(w) if context.worker => {
            let mask = match w.mask {
                crate::signals::extract::contracts::MaskForm::Stream => WireMask::Inherit,
                crate::signals::extract::contracts::MaskForm::Unmasked
                | crate::signals::extract::contracts::MaskForm::StaticXor => WireMask::None,
                _ => return Err(err("worker unresolved mask")),
            };
            facts.push(json!({"workerPrimitive":[w.span.start,w.span.end],"writerFactory":w.writer_factory,"streamFactory":w.stream_factory,"input":"posted transactionToken; worker payload.a; paired h31"}));
            Ok(match n {
                G::Number(_) => Op::Number {
                    envelope: None,
                    mask,
                    literal: w.value.parse::<f64>().ok().map(f64::to_bits),
                },
                G::String(_) => Op::String {
                    mask,
                    literal: None,
                },
                _ => Op::Byte {
                    mask,
                    xor: w.xor_literal.unwrap_or(0.) as u8,
                    allowed: w.value.parse::<u8>().ok().map(|b| vec![b]),
                },
            })
        }
        G::String(w) if context.direct => {
            // Direct adapter: the wire string is modified-UTF-8 units under the
            // grammar's mask. The strict compiler's certificer is bypassed here
            // because this example is explicitly the non-lifecycle path.
            let mask = direct_mask(w.mask)?;
            Ok(Op::String {
                mask,
                literal: None,
            })
        }
        G::Number(w) if context.direct => Ok(Op::Number {
            envelope: None,
            mask: direct_mask(w.mask)?,
            literal: w.value.parse::<f64>().ok().map(f64::to_bits),
        }),
        G::Byte(w) if context.direct => Ok(Op::Byte {
            mask: direct_mask(w.mask)?,
            xor: w.xor_literal.unwrap_or(0.) as u8,
            allowed: w.value.parse::<u8>().ok().map(|b| vec![b]),
        }),
        _ => dc::node(n, e, a, p, source),
    }
}
#[allow(clippy::too_many_arguments)]
fn packed_counter_choice(
    n: &G,
    arms: &[crate::signals::extract::contracts::ChoiceArm],
    span: SourceSpan,
    condition: &Option<String>,
    e: &EmissionContract,
    a: &AlgorithmReport,
    p: &SourcePlan,
    source: &str,
    path: &[usize],
    last: Option<(Vec<usize>, SourceSpan)>,
    facts: &mut Vec<Value>,
    context: &CompileContext,
) -> Result<Option<Op>, WireError> {
    let Some(condition) = condition.as_deref() else {
        return Ok(None);
    };
    let (equals, inverted, phase) = if condition.contains("=== 8") {
        (8u32, false, PackedPhase::BeforeFlush)
    } else if condition.contains("!== 0") {
        (0u32, true, PackedPhase::AfterLoop)
    } else {
        return Ok(None);
    };
    let count = if matches!(phase, PackedPhase::BeforeFlush) {
        context.loops.borrow().last().cloned()
    } else {
        context.last_count.borrow().clone()
    };
    let Some(count) = count else {
        return Ok(None);
    };
    let mut ops = Vec::new();
    for (i, arm) in arms.iter().enumerate() {
        let mut q = path.to_vec();
        q.push(i);
        ops.push(compile(
            &arm.body,
            e,
            a,
            p,
            source,
            q,
            last.clone(),
            facts,
            context,
        )?);
    }
    facts.push(json!({
        "packedCounterChoice": [span.start, span.end],
        "countPath": count.path,
        "phase": format!("{phase:?}"),
        "kind": "runtime_packed_count"
    }));
    Ok(Some(Op::CounterChoice {
        proof: CounterChoiceProof {
            choice: span,
            read: span,
            path: control_path(e, n),
            counter: CounterRef {
                loop_span: count.loop_span,
                repeat_path: count.repeat_path.clone(),
                binding: count.counter_binding,
            },
            predicate: BitPredicate {
                mask: u32::MAX,
                equals,
                inverted,
            },
            input: CounterInput::Packed {
                count,
                increment: span,
                reset: span,
                modulus: 8,
                phase,
            },
        },
        arms: ops,
    }))
}
fn control_path(e: &EmissionContract, n: &G) -> ControlPath {
    fn walk(r: &G, n: &G, p: ControlPath) -> Option<ControlPath> {
        if std::ptr::eq(r, n) {
            return Some(p);
        }
        match r {
            G::Sequence { items } => {
                for (i, c) in items.iter().enumerate() {
                    let mut q = p.clone();
                    q.push(ControlStep::Child(i));
                    if let Some(x) = walk(c, n, q) {
                        return Some(x);
                    }
                }
            }
            G::Repeat { body, .. } => {
                let mut q = p;
                q.push(ControlStep::Body);
                return walk(body, n, q);
            }
            G::Choice { arms, .. } => {
                for (i, a) in arms.iter().enumerate() {
                    let mut q = p.clone();
                    q.push(ControlStep::Arm(i));
                    if let Some(x) = walk(&a.body, n, q) {
                        return Some(x);
                    }
                }
            }
            _ => {}
        }
        None
    }
    walk(&e.grammar, n, vec![]).unwrap()
}

pub struct CompileContext<'a> {
    /// Allow uncertified leaves and runtime packed-count synthesis.
    pub direct: bool,
    pub worker: bool,
    pub counts: &'a [Value],
    /// Enclosing packed-loop counts, innermost last.
    pub loops: std::cell::RefCell<Vec<CountRef>>,
    /// The most recently compiled loop count, for the trailing flush choice.
    pub last_count: std::cell::RefCell<Option<CountRef>>,
}

pub fn manual_local_storage(value: &WireValue, slot: &WireSlot) -> Option<NamedSignal> {
    if slot.naming.as_ref()?.schema_key != "LocalStoragePersistentId" {
        return None;
    }
    let WireValue::Choice { arm, value } = value else {
        return None;
    };
    let WireValue::Sequence(items) = value.as_ref() else {
        return None;
    };
    let (stored, status) = match (arm, items.as_slice()) {
        (0, [WireValue::Byte(byte), WireValue::NumberBits(status)]) => (
            if *byte == 130 {
                serde_json::Value::Null
            } else {
                json!(byte)
            },
            f64::from_bits(*status),
        ),
        (1, [WireValue::StringUnits(units), WireValue::NumberBits(status)]) => (
            json!(String::from_utf16(units).ok()?),
            f64::from_bits(*status),
        ),
        _ => return None,
    };
    let mut fields = serde_json::Map::new();
    fields.insert("storedId".into(), stored);
    fields.insert("storageStatus".into(), json!(status));
    Some(NamedSignal {
        fields: Some(fields),
        status: NamedStatus::Decoded,
        metadata: json!({
            "address": slot.address,
            "canonicalName": "LocalStoragePersistentId",
            "position": slot.routing,
        }),
    })
}

pub fn is_local_storage(slot: &WireSlot) -> bool {
    slot.naming
        .as_ref()
        .is_some_and(|name| name.schema_key == "LocalStoragePersistentId")
}

/// Invert the direct root-arm projection. Retained bytes supply the seed and
/// unprojected state; unsupported edits fail rather than replaying stale bytes.
pub(crate) fn encode_local_storage(
    slot: &WireSlot,
    fields: &serde_json::Map<String, Value>,
    original: Option<&[u8]>,
    token: i32,
    order: FloatByteOrder,
) -> Result<Vec<u8>, WireError> {
    let raw =
        original.ok_or_else(|| err("direct storage encoding requires retained original bytes"))?;
    let seed = u32::from_be_bytes(
        raw.get(..4)
            .ok_or(WireError::Truncated)?
            .try_into()
            .map_err(|_| WireError::Truncated)?,
    );
    let expected = crate::signals::wire::named::canonical_fields(fields);
    let status = fields
        .get("storageStatus")
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or(WireError::Type)?;
    let stored = fields.get("storedId").ok_or(WireError::Type)?;
    let (arm, stored) = match stored {
        Value::Null => (0, WireValue::Byte(130)),
        Value::String(text) => (1, WireValue::StringUnits(text.encode_utf16().collect())),
        value => (
            0,
            WireValue::Byte(
                value
                    .as_u64()
                    .filter(|byte| *byte < 256 && *byte != 130)
                    .ok_or(WireError::Type)? as u8,
            ),
        ),
    };
    let edited = WireValue::Choice {
        arm,
        value: Box::new(WireValue::Sequence(vec![
            stored,
            WireValue::NumberBits(status.to_bits()),
        ])),
    };
    for support in &slot.programs {
        let program = match support {
            Support::Supported { program } | Support::RequiresInitializer { program, .. } => {
                program
            }
            Support::Unsupported { .. } => continue,
        };
        let Ok(base) = dc::decode(program, raw, token, order) else {
            continue;
        };
        if dc::encode(program, &base, seed, token, order)
            .ok()
            .as_deref()
            != Some(raw)
        {
            continue;
        }
        // Preserve numeric bit patterns (including negative zero) on an unchanged roundtrip.
        let value = if manual_local_storage(&base, slot)
            .and_then(|named| named.fields)
            .is_some_and(|fields| {
                crate::signals::wire::named::canonical_fields(&fields) == expected
            }) {
            &base
        } else {
            &edited
        };
        let Ok(bytes) = dc::encode(program, value, seed, token, order) else {
            continue;
        };
        let checked = dc::decode(program, &bytes, token, order)?;
        if manual_local_storage(&checked, slot)
            .and_then(|named| named.fields)
            .is_some_and(|fields| {
                crate::signals::wire::named::canonical_fields(&fields) == expected
            })
        {
            return Ok(bytes);
        }
    }
    Err(err(
        "direct storage fields cannot be encoded by the selected root arms",
    ))
}
