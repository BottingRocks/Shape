//! Packed flag bytes: the `type: flags` rendering of a byte built from up to
//! eight booleans.
//!
//! After a slot's fields are bound, [`apply_flags`] looks up every bound
//! card field that is a packed flags byte ([`PACKED`]), derives the bit
//! position of every boolean from the write site and names each bit from the
//! signal's card, then records `{flag name: bit}` on the binding so the
//! projection renders the field as
//! `{"type": "flags", "mask": null, "bits": {"<flag>": <bit>}}`.
//!
//! * Bit positions are per build and come from the written value only: the
//!   `|` / `+` terms of the byte, each `term << n` (or `term * 2^n`,
//!   `test ? 2^n : 0`) shift, through the nearest definitions of the local
//!   bindings the value reads ([`FinderCtx::packed_terms`]).
//! * A bit is named by structure: the probed string, host member or global
//!   its boolean reads ([`FinderCtx::slice_of`] evidence, nearest wins), or
//!   for a whole-list probe the list item it tests. Names are the card's
//!   `flags` list (the signal's `CARD` in its catalog file); a
//!   field whose bits cannot all be named makes the slot refused.
//! * Like the catalog files, nothing here reads salts, slot addresses,
//!   descriptor indices, generated spellings or build constants
//!   (`catalog_sources_stay_structural` also checks this file).

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::{BinaryOperator, Expression};
use oxc_ast::AstKind;
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::{GetSpan, Span};

mod flow;
mod loops;
mod rules;

use crate::signals::schema::{find_key, flag_leaf_offsets};
use crate::signals::analysis::{FieldBinding, FinderCtx, SignalLayout, SlotView};

// ===========================================================================
// Packed fields
// ===========================================================================

/// How the bits of a packed field are named.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Namer {
    /// Each card flag is named by a probe it reads: the flag text itself
    /// (or, for `prefixed`, the part after the first `.`: `window.File` reads
    /// `File`, the receiver telling `window.x` from `document.x` apart).
    Card { prefixed: bool },
    /// `(card flag, probes)`: the flag all of whose probes (member, string
    /// argument, string literal or global names) the boolean reads, nearest
    /// first (a flag's depth is its deepest probe); on equal depth the flag
    /// with more probes (the more specific) wins.
    Probes(&'static [(&'static str, &'static [&'static str])]),
    /// A packing loop over a probed list (`acc = acc | X[j] << c`, a flush
    /// byte at `c === 8`, a tail byte after the loop): bit `p` of the byte
    /// stream is the `p`-th list item in the loop's iteration order, named
    /// by the item string ([`FinderCtx::loop_stream`]). In an `array` of
    /// flush bytes, `p` counts across the bytes (byte `p / 8`, bit `p % 8`);
    /// the tail byte holds positions from the last full byte on.
    Loop,
}

/// One packed flags field of a card.
pub(crate) struct PackedField {
    pub signal: &'static str,
    /// Card field names (all fields sharing one flag list).
    pub fields: &'static [&'static str],
    /// The card fields whose `flags` lists (joined) name the bits (empty:
    /// the [`Namer::Probes`] table's flags).
    pub flags_from: &'static [&'static str],
    pub namer: Namer,
}

/// Every packed flags field handled by the shared derivation.
pub(crate) const PACKED: &[PackedField] = &[
    PackedField {
        signal: "BrowserApiOwnPropertyFlags",
        fields: &["ownPropertyFlags"],
        flags_from: &["ownPropertyFlags"],
        namer: Namer::Card { prefixed: false },
    },
    PackedField {
        signal: "GlobalPropertyChecks",
        fields: &[
            "presenceBits1",
            "presenceBits2",
            "presenceBits3",
            "presenceBits4",
            "presenceBits5",
            "presenceBits6",
            "presenceBits7",
            "presenceBits8",
            "presenceBits9",
        ],
        flags_from: &["presenceBits1"],
        namer: Namer::Card { prefixed: true },
    },
    PackedField {
        signal: "BrowserApiProxyFlags",
        fields: &["proxyFlags1", "proxyFlags2", "proxyFlags3"],
        flags_from: &["proxyFlags1"],
        namer: Namer::Probes(&[
            ("HTMLMediaElement.prototype.canPlayType", &["HTMLMediaElement", "canPlayType"]),
            ("Permissions.prototype.canPlayType", &["Permissions", "canPlayType"]),
            ("speechSynthesis", &["speechSynthesis"]),
            ("Navigator.prototype.languages", &["getOwnPropertyDescriptor", "languages"]),
            ("Navigator.prototype.hardwareConcurrency", &["getOwnPropertyDescriptor", "hardwareConcurrency"]),
            ("Navigator.prototype.plugins", &["getOwnPropertyDescriptor", "plugins"]),
            ("HTMLCanvasElement.prototype.toDataURL", &["HTMLCanvasElement", "toDataURL"]),
            ("navigator.mimeTypes", &["navigator", "mimeTypes"]),
            ("navigator.plugins", &["navigator", "plugins"]),
            ("navigator.getBattery", &["navigator", "getBattery"]),
            ("navigator", &["navigator"]),
            ("Function.prototype.toString", &["Function", "toString"]),
            ("Intl", &["Intl"]),
            ("Intl.supportedValuesOf", &["Intl", "supportedValuesOf"]),
            ("document.createElement", &["createElement"]),
            ("WebGLRenderingContext.prototype.getParameter", &["WebGLRenderingContext", "getParameter"]),
            ("CanvasRenderingContext2D.prototype.getImageData", &["CanvasRenderingContext2D", "getImageData"]),
            ("Proxy", &["Proxy"]),
        ]),
    },
    PackedField {
        signal: "FunctionToStringComparison",
        fields: &["probeFlags"],
        flags_from: &["probeFlags"],
        // `threw` is set only in the comparison's `catch`: by elimination.
        namer: Namer::Probes(&[("equal", &["toString"]), ("threw", &[])]),
    },
    PackedField {
        signal: "ProxyTamperFlags",
        fields: &["proxyFlags"],
        flags_from: &["proxyFlags"],
        // The navigator probe compares the brand string `#<Navigator>`; the
        // global probe has no host token: by elimination.
        namer: Namer::Probes(&[("navigatorLooksLikeProxy", &["#<Navigator>"]), ("globalLooksLikeProxy", &[])]),
    },
    PackedField {
        signal: "PersistentStorageState",
        fields: &["flags"],
        flags_from: &["flags"],
        // The access failure is set in `catch`/`else` arms only: by
        // elimination.
        namer: Namer::Probes(&[
            ("idRemovalSeen", &["newValue"]),
            ("noInheritedId", &["transactionToken"]),
            ("storageAccessFailed", &[]),
        ]),
    },
    PackedField {
        signal: "OuterArgsGlobalProcess",
        fields: &["flags"],
        flags_from: &["flags"],
        // `stringifyThrew` is set in the stringify `catch`: by elimination.
        namer: Namer::Probes(&[
            ("outerArgsDefined", &["outerArgs"]),
            ("processDefined", &["process"]),
            ("globalIsSelf", &["global"]),
            ("stringifyThrew", &[]),
        ]),
    },
    PackedField {
        signal: "SeleniumIdeArtifactFlags",
        fields: &["automationFlags"],
        flags_from: &["automationFlags"],
        namer: Namer::Probes(&[("sideexPlaying", &["MutationObserver"]), ("promptScript", &["test"])]),
    },
    PackedField {
        signal: "BraveDuckDuckGoNavigatorFlags",
        fields: &["browserFlags"],
        flags_from: &["browserFlags"],
        namer: Namer::Probes(&[("navigator.brave", &["brave"]), ("navigator.duckduckgo", &["duckduckgo"])]),
    },
    PackedField {
        signal: "CrossOriginIsolationFlags",
        fields: &["isolationFlags"],
        flags_from: &["isolationFlags"],
        namer: Namer::Probes(&[
            ("sharedArrayBufferAvailable", &["SharedArrayBuffer"]),
            ("crossOriginIsolated", &["crossOriginIsolated"]),
        ]),
    },
    PackedField {
        signal: "StorageAccessApiPresence",
        fields: &["apiFlags"],
        flags_from: &["apiFlags"],
        namer: Namer::Card { prefixed: false },
    },
    PackedField {
        signal: "KeyboardEventSamples",
        // `ownFlag` is the one boolean written on its own byte (bit 0).
        fields: &["flags", "ownFlag"],
        flags_from: &["flags"],
        // Record labels resolve to what the key listener stored: the
        // modifier members, the `instanceOfUIEvent` / `markedAsTrusted`
        // event record, and the phase pair `[type === keydown, type ===
        // keyup]` by element (`@0`, `@1`).
        namer: Namer::Probes(&[
            ("isKeydown", &["@0"]),
            ("isKeyup", &["@1"]),
            ("ctrlKey", &["ctrlKey"]),
            ("altKey", &["altKey"]),
            ("metaKey", &["metaKey"]),
            ("shiftKey", &["shiftKey"]),
            ("instanceOfUIEvent", &["instanceOfUIEvent"]),
            ("isTrusted", &["markedAsTrusted"]),
        ]),
    },
    PackedField {
        signal: "GlobalOwnPropertyBits",
        fields: &["ownPropertyBits", "ownPropertyBitsTail"],
        flags_from: &["ownPropertyBits"],
        namer: Namer::Loop,
    },
    PackedField {
        signal: "MatchMedia",
        // Each group's in-loop flush (`<group>Bits`) and trailing flush
        // (`<group>Flush`) of the match-bit loop over its candidate list;
        // each field names its bits from its own card list.
        fields: &["gridBits", "gridFlush", "overflowInlineBits", "overflowInlineFlush", "anyPointerBits", "anyPointerFlush", "prefersContrastBits", "prefersContrastFlush", "scriptingBits", "scriptingFlush", "lightLevelBits", "lightLevelFlush", "displayModeBits", "displayModeFlush", "forcedColorsBits", "forcedColorsFlush", "hoverBits", "hoverFlush", "prefersColorSchemeBits", "prefersColorSchemeFlush", "colorGamutBits", "colorGamutFlush", "invertedColorsBits", "invertedColorsFlush", "orientationBits", "orientationFlush", "scanBits", "scanFlush", "prefersReducedTransparencyBits", "prefersReducedTransparencyFlush", "updateBits", "updateFlush", "pointerBits", "pointerFlush", "anyHoverBits", "anyHoverFlush", "prefersReducedMotionBits", "prefersReducedMotionFlush", "overflowBlockBits", "overflowBlockFlush"],
        flags_from: &[],
        namer: Namer::Loop,
    },
    PackedField {
        signal: "FontAvailability",
        // Family B: one bit per probed font, flushed every 8 fonts.
        fields: &["fontPresenceBits", "fontPresenceTail"],
        flags_from: &["fontPresenceBits"],
        namer: Namer::Loop,
    },
    PackedField {
        signal: "AnchorIdnaUnicodeVersion",
        // One bit per tested code point: the flush byte and the tail byte.
        fields: &["resultBits1", "resultBits2"],
        flags_from: &["resultBits1"],
        namer: Namer::Loop,
    },
    PackedField {
        signal: "WebAuthnInterfacePresence",
        fields: &["flags"],
        flags_from: &["flags"],
        namer: Namer::Card { prefixed: false },
    },
];

fn packed_field(signal: &str, field: &str) -> Option<&'static PackedField> {
    PACKED
        .iter()
        .find(|packed| packed.signal == signal && packed.fields.contains(&field))
}

// ===========================================================================
// Card flag lists
// ===========================================================================

/// The `flags` list of card field `field` of `signal` (any message, any
/// nesting; the first field of that name with a list).
pub(crate) fn card_flags(signal: &str, field: &str) -> Option<Vec<String>> {
    let flags = crate::signals::catalog::card(signal)?.flags_of(field)?;
    Some(flags.iter().map(|flag| flag.to_string()).collect())
}

// ===========================================================================
// Bit terms
// ===========================================================================

/// One boolean of a packed byte: its bit and the expression it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PackedTerm {
    pub bit: u32,
    /// `None`: a constant 1 bit.
    pub base: Option<NodeId>,
}

/// In-place updates of a binding between its definition and a read.
enum Updates {
    Or(Vec<NodeId>),
    Other,
}

/// Definitions followed while splitting a packed value.
const TERM_HOPS: usize = 12;
/// Definitions followed while naming a boolean.
const NAME_HOPS: usize = 3;

fn integer(expression: &Expression<'_>) -> Option<u32> {
    match expression.without_parentheses() {
        Expression::NumericLiteral(literal)
            if literal.value.fract() == 0.0 && literal.value >= 0.0 && literal.value < 256.0 =>
        {
            Some(literal.value as u32)
        }
        _ => None,
    }
}

/// A mask draw at a byte write: `k()`, or the tracer's `k.call(void 0)`
/// (`t[2].call(void 0)` with the draw in an argument tuple).
fn mask_call(expression: &Expression<'_>) -> bool {
    let Expression::CallExpression(call) = expression.without_parentheses() else { return false };
    if call.arguments.is_empty() {
        return true;
    }
    let Expression::StaticMemberExpression(callee) = call.callee.without_parentheses() else { return false };
    callee.property.name == "call"
        && call.arguments.len() == 1
        && matches!(call.arguments[0].as_expression().map(Expression::without_parentheses),
            Some(Expression::UnaryExpression(unary)) if unary.operator == oxc_ast::ast::UnaryOperator::Void)
}

fn power_of_two(value: u32) -> Option<u32> {
    (value.is_power_of_two()).then(|| value.trailing_zeros())
}

impl FinderCtx<'_, '_, '_> {
    fn expression_node(&self, expression: &Expression<'_>) -> Option<NodeId> {
        self.node_with_span(expression.without_parentheses().span())
    }

    /// The nearest definition value of the identifier node `node`, and
    /// whether the binding is also updated in place (`x |= …`, `++x`).
    ///
    /// `Updates::Or` lists the right sides of `x |= …` updates between the
    /// definition and the read (more terms of the same byte); any other
    /// update in between (`++x`, `x += …`) is `Updates::Other`.
    /// The reaching definition value of a binding read.
    pub(crate) fn reaching_value(&self, node: NodeId) -> Option<NodeId> {
        self.identifier_value(node).map(|(value, _)| value)
    }

    fn identifier_value(&self, node: NodeId) -> Option<(NodeId, Updates)> {
        let slice = self.slice_of(node, 1);
        let symbol = self.reference_symbol(node)?;
        let (_, definition, _) = slice.definitions.iter().find(|(defined, _, _)| *defined == symbol)?;
        let definition = &self.past_zero_arm(symbol, *definition, node).unwrap_or(*definition);
        let definition = &self.reaching_definition(symbol, *definition, node).unwrap_or(*definition);
        let defined_at = self.parse.nodes.kind(*definition).span().end;
        let read_at = self.parse.nodes.kind(node).span().start;
        let mut ors = Vec::new();
        for (updated, update) in &slice.updates {
            if *updated != symbol {
                continue;
            }
            let span = self.parse.nodes.kind(*update).span();
            if span.start < defined_at || span.end > read_at || self.exits_before(*update, node) {
                continue;
            }
            match self.parse.nodes.kind(*update) {
                AstKind::AssignmentExpression(assignment)
                    if assignment.operator == oxc_ast::ast::AssignmentOperator::BitwiseOR =>
                {
                    ors.push(self.expression_node(&assignment.right)?);
                }
                _ => return Some((self.definition_value(*definition)?, Updates::Other)),
            }
        }
        Some((self.definition_value(*definition)?, Updates::Or(ors)))
    }

    /// `if (…) { x = a | b << 1 } else { x = 0 }` then a read of `x`: the
    /// textually nearest definition is the all-zero arm, which does not
    /// contain the read. The packed definition is the nearest one before
    /// that arm.
    fn past_zero_arm(&self, symbol: SymbolId, definition: NodeId, read: NodeId) -> Option<NodeId> {
        let nodes = self.parse.nodes;
        let value = self.definition_value(definition)?;
        let AstKind::NumericLiteral(literal) = nodes.kind(value) else { return None };
        if literal.value != 0.0 {
            return None;
        }
        let read_at = nodes.kind(read).span().start;
        let mut child = definition;
        loop {
            let parent = nodes.parent_id(child);
            if parent == child {
                return None;
            }
            if let AstKind::IfStatement(statement) = nodes.kind(parent) {
                let arm = nodes.kind(child).span();
                let in_arm = statement.consequent.span() == arm
                    || statement.alternate.as_ref().is_some_and(|alternate| alternate.span() == arm);
                if in_arm && !(arm.start <= read_at && read_at < arm.end) {
                    // The latest plain assignment in the other arm.
                    let other = if statement.consequent.span() == arm {
                        statement.alternate.as_ref()?.span()
                    } else {
                        statement.consequent.span()
                    };
                    return self
                        .parse
                        .scoping
                        .get_resolved_references(symbol)
                        .filter(|reference| reference.is_write())
                        .filter_map(|reference| {
                            let parent = nodes.parent_id(reference.node_id());
                            match nodes.kind(parent) {
                                AstKind::AssignmentExpression(assignment) if assignment.operator.is_assign() => {
                                    Some((assignment.span, parent))
                                }
                                _ => None,
                            }
                        })
                        .filter(|(span, _)| other.start <= span.start && span.end <= other.end)
                        .max_by_key(|(span, _)| span.end)
                        .map(|(_, node)| node);
                }
            }
            if matches!(nodes.kind(parent), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)) {
                return None;
            }
            child = parent;
        }
    }

    /// The node is built like a packed value (worth splitting).
    fn packing_shaped(&self, node: NodeId) -> bool {
        match self.parse.nodes.kind(node) {
            AstKind::BinaryExpression(binary) => match binary.operator {
                BinaryOperator::BitwiseOR | BinaryOperator::Addition => true,
                BinaryOperator::ShiftLeft => integer(&binary.right).is_some(),
                BinaryOperator::Multiplication => {
                    integer(&binary.right).and_then(power_of_two).is_some()
                        || integer(&binary.left).and_then(power_of_two).is_some()
                }
                _ => false,
            },
            AstKind::ConditionalExpression(conditional) => match self.decided_arm(conditional) {
                Some(arm) => self.expression_node(arm).is_some_and(|arm| self.packing_shaped(arm)),
                None => integer(&conditional.consequent).is_some() && integer(&conditional.alternate) == Some(0),
            },
            AstKind::NumericLiteral(_) => true,
            AstKind::ParenthesizedExpression(inner) => self
                .expression_node(&inner.expression)
                .is_some_and(|inner| self.packing_shaped(inner)),
            AstKind::IdentifierReference(_) => true,
            _ => false,
        }
    }

    /// The booleans of a packed byte value `value`: `a | b << 1 | …`,
    /// `+`-joined terms, `t * 2^n`, `test ? 2^n : 0`, through the nearest
    /// definitions of the bindings it reads. Fails on a constant non-zero
    /// bit, a variable shift, a binding updated in place, or two terms on one
    /// bit.
    pub(crate) fn packed_terms(&self, value: NodeId) -> Result<Vec<PackedTerm>, String> {
        let mut out = Vec::new();
        self.split_terms(value, 0, TERM_HOPS, &mut out)?;
        let mut seen = BTreeSet::new();
        for term in &out {
            if term.bit > 7 {
                return Err(format!("bit {} is outside the byte", term.bit));
            }
            if !seen.insert(term.bit) {
                return Err(format!("two terms on bit {}", term.bit));
            }
        }
        out.sort_by_key(|term| term.bit);
        Ok(out)
    }

    fn split_terms(&self, node: NodeId, shift: u32, hops: usize, out: &mut Vec<PackedTerm>) -> Result<(), String> {
        let child = |expression: &Expression<'_>| {
            self.expression_node(expression)
                .ok_or_else(|| "unresolved sub-expression".to_string())
        };
        match self.parse.nodes.kind(node) {
            AstKind::ParenthesizedExpression(inner) => self.split_terms(child(&inner.expression)?, shift, hops, out),
            AstKind::BinaryExpression(binary) => match binary.operator {
                BinaryOperator::BitwiseOR | BinaryOperator::Addition => {
                    self.split_terms(child(&binary.left)?, shift, hops, out)?;
                    self.split_terms(child(&binary.right)?, shift, hops, out)
                }
                BinaryOperator::ShiftLeft => match integer(&binary.right) {
                    Some(amount) => self.split_terms(child(&binary.left)?, shift + amount, hops, out),
                    None => match self.guarded_shift(binary) {
                        Some(amount) => self.split_terms(child(&binary.left)?, shift + amount, hops, out),
                        None => Err("variable shift".into()),
                    },
                },
                BinaryOperator::Multiplication => {
                    match (
                        integer(&binary.right).and_then(power_of_two),
                        integer(&binary.left).and_then(power_of_two),
                    ) {
                        (Some(amount), _) => self.split_terms(child(&binary.left)?, shift + amount, hops, out),
                        (None, Some(amount)) => self.split_terms(child(&binary.right)?, shift + amount, hops, out),
                        _ => {
                            out.push(PackedTerm { bit: shift, base: Some(node) });
                            Ok(())
                        }
                    }
                }
                _ => {
                    out.push(PackedTerm { bit: shift, base: Some(node) });
                    Ok(())
                }
            },
            // `t[k]` of the tracer's argument tuple: the element packed.
            AstKind::ComputedMemberExpression(member) if hops > 0 && self.tuple_member_slot(member).is_some() => {
                let slot = self.tuple_member_slot(member).ok_or("tuple slot")?;
                self.split_terms(slot, shift, hops - 1, out)
            }
            // A test decided by its values alone (`b !== null` for a
            // boolean `b`): the arm it takes.
            AstKind::ConditionalExpression(conditional) if self.decided_arm(conditional).is_some() => {
                let arm = self.decided_arm(conditional).ok_or("decided arm")?;
                self.split_terms(child(arm)?, shift, hops, out)
            }
            AstKind::NumericLiteral(literal) => {
                // A constant bit (a test the deobfuscator folded): named only
                // by elimination (see `apply_flags`).
                let value = literal.value;
                if value.fract() != 0.0 || !(0.0..256.0).contains(&value) {
                    return Err(format!("constant {value}"));
                }
                let value = value as u32;
                for bit in 0..8 {
                    if value & (1 << bit) != 0 {
                        out.push(PackedTerm { bit: shift + bit, base: None });
                    }
                }
                Ok(())
            }
            AstKind::ConditionalExpression(conditional)
                if integer(&conditional.alternate) == Some(0) && integer(&conditional.consequent).is_some() =>
            {
                let consequent = integer(&conditional.consequent).unwrap_or(0);
                match power_of_two(consequent) {
                    Some(amount) => {
                        out.push(PackedTerm {
                            bit: shift + amount,
                            base: Some(child(&conditional.test)?),
                        });
                        Ok(())
                    }
                    None => Err(format!("conditional constant {consequent}")),
                }
            }
            AstKind::IdentifierReference(_) if hops > 0 => match self.identifier_value(node) {
                Some((_, Updates::Other)) => Err("binding updated in place".into()),
                Some((value, Updates::Or(ors))) if self.packing_shaped(value) || !ors.is_empty() => {
                    if self.packing_shaped(value) {
                        self.split_terms(value, shift, hops - 1, out)?;
                    } else {
                        out.push(PackedTerm { bit: shift, base: Some(value) });
                    }
                    for or in ors {
                        self.split_terms(or, shift, hops - 1, out)?;
                    }
                    Ok(())
                }
                _ => {
                    out.push(PackedTerm { bit: shift, base: Some(node) });
                    Ok(())
                }
            },
            _ => {
                out.push(PackedTerm { bit: shift, base: Some(node) });
                Ok(())
            }
        }
    }

    /// The arm of `test ? a : b` whose test holds or fails on its values
    /// alone ([`flow::decided`]).
    fn decided_arm<'e>(&self, conditional: &'e oxc_ast::ast::ConditionalExpression<'e>) -> Option<&'e Expression<'e>> {
        let test = self.expression_node(&conditional.test)?;
        match flow::decided(&self.sym(test))? {
            true => Some(&conditional.consequent),
            false => Some(&conditional.alternate),
        }
    }

    /// The receiver binding of a `recv.m` / `recv["m"]` boolean and the
    /// member name, with the receiver's nearest definition (registers are
    /// reused: the same binding holds different objects over time).
    fn member_read(&self, node: NodeId) -> Option<((SymbolId, Option<NodeId>), String)> {
        let (object, name) = match self.parse.nodes.kind(node) {
            AstKind::StaticMemberExpression(member) => (&member.object, member.property.name.to_string()),
            AstKind::ComputedMemberExpression(member) => match &member.expression {
                Expression::StringLiteral(literal) => (&member.object, literal.value.to_string()),
                _ => return None,
            },
            AstKind::ParenthesizedExpression(inner) => return self.member_read(self.expression_node(&inner.expression)?),
            _ => return None,
        };
        // `((a, b, c, d) => { a[b] = c; return d; })(buf, i, v, recv).m`:
        // an inline store helper returning one of its formals unchanged
        // reads `m` on that argument.
        let object = match object.without_parentheses() {
            Expression::CallExpression(call) => self.returned_argument(call)?,
            object => object,
        };
        let Expression::Identifier(_) = object.without_parentheses() else {
            return None;
        };
        let receiver = self.expression_node(object)?;
        let symbol = self.reference_symbol(receiver)?;
        let definition = self
            .slice_of(receiver, 1)
            .definitions
            .iter()
            .find(|(defined, _, _)| *defined == symbol)
            .map(|(_, definition, _)| *definition);
        Some(((symbol, definition), name))
    }

    /// For `helper(…).m` / `helper(…)["m"]` with an inline store helper
    /// callee, the node of the argument the helper returns.
    fn helper_receiver(&self, node: NodeId) -> Option<NodeId> {
        let object = match self.parse.nodes.kind(node) {
            AstKind::StaticMemberExpression(member) => &member.object,
            AstKind::ComputedMemberExpression(member) => &member.object,
            _ => return None,
        };
        let Expression::CallExpression(call) = object.without_parentheses() else {
            return None;
        };
        self.expression_node(self.returned_argument(call)?)
    }

    /// The argument an inline store helper call returns: the callee is an
    /// arrow over plain formals whose body is one `a[b] = v;` store and
    /// `return p;` of a formal `p` that is not the stored-into array and is
    /// never written in the body.
    fn returned_argument<'e>(&self, call: &'e oxc_ast::ast::CallExpression<'e>) -> Option<&'e Expression<'e>> {
        let Expression::ArrowFunctionExpression(arrow) = call.callee.without_parentheses() else {
            return None;
        };
        if arrow.params.rest.is_some() || arrow.r#async {
            return None;
        }
        let mut formals = Vec::new();
        for parameter in &arrow.params.items {
            let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
                return None;
            };
            if parameter.initializer.is_some() {
                return None;
            }
            formals.push(identifier.symbol_id.get()?);
        }
        let [oxc_ast::ast::Statement::ExpressionStatement(store), oxc_ast::ast::Statement::ReturnStatement(returned)] =
            arrow.body.statements.as_slice()
        else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = store.expression.without_parentheses() else {
            return None;
        };
        let oxc_ast::ast::AssignmentTarget::ComputedMemberExpression(target) = &assignment.left else {
            return None;
        };
        let formal = |expression: &Expression<'_>| match expression.without_parentheses() {
            Expression::Identifier(identifier) => {
                let symbol = self.identifier_symbol(identifier)?;
                formals.iter().position(|formal| *formal == symbol)
            }
            _ => None,
        };
        let position = formal(returned.argument.as_ref()?)?;
        if formal(&target.object) == Some(position) {
            return None;
        }
        let scoping = self.parse.scoping;
        if scoping.get_resolved_references(formals[position]).any(|reference| reference.is_write()) {
            return None;
        }
        match call.arguments.get(position)? {
            oxc_ast::ast::Argument::SpreadElement(_) => None,
            argument => argument.as_expression(),
        }
    }

    /// [`Self::direct_member`], or for `t[k].m` with `t` a register tuple
    /// (`t = [x, obj]`, read only by literal index where the tuple reaches:
    /// [`Self::tuple_member_slot`]) whose element `k` is a plain binding,
    /// that binding as the receiver, with its definition where the tuple
    /// was built. Only the receiver of a prefixed card list
    /// (`window.x` / `document.x`) reads this.
    fn receiver_member(&self, node: NodeId) -> Option<((SymbolId, Option<NodeId>), String)> {
        if let Some(read) = self.direct_member(node) {
            return Some(read);
        }
        let (object, name) = match self.parse.nodes.kind(node) {
            AstKind::StaticMemberExpression(member) => (&member.object, member.property.name.to_string()),
            _ => return None,
        };
        let Expression::ComputedMemberExpression(element) = object.without_parentheses() else {
            return None;
        };
        let receiver = self.tuple_member_slot(element)?;
        let AstKind::IdentifierReference(_) = self.parse.nodes.kind(receiver) else {
            return None;
        };
        let symbol = self.reference_symbol(receiver)?;
        let definition = self
            .slice_of(receiver, 1)
            .definitions
            .iter()
            .find(|(defined, _, _)| *defined == symbol)
            .map(|(_, definition, _)| *definition);
        Some(((symbol, definition), name))
    }

    /// The boolean read directly by `node`, following plain copies.
    fn direct_member(&self, node: NodeId) -> Option<((SymbolId, Option<NodeId>), String)> {
        let mut node = node;
        for _ in 0..TERM_HOPS {
            if let Some(read) = self.member_read(node) {
                return Some(read);
            }
            match self.parse.nodes.kind(node) {
                AstKind::IdentifierReference(_) => match self.identifier_value(node)? {
                    (value, Updates::Or(ors)) if ors.is_empty() => node = value,
                    _ => return None,
                },
                _ => return None,
            }
        }
        None
    }
}

// ===========================================================================
// Packing loops
// ===========================================================================

/// The bits of one packing loop.
pub(crate) struct LoopStream {
    /// Item strings in stream order (bit position).
    pub items: Vec<String>,
    /// Bits per flush byte.
    pub byte: u32,
    /// The loop statement.
    pub looped: Span,
}

fn is_loop(kind: &AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::ForStatement(_) | AstKind::WhileStatement(_) | AstKind::DoWhileStatement(_) | AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
    )
}

impl FinderCtx<'_, '_, '_> {
    fn enclosing_loop(&self, node: NodeId) -> Option<NodeId> {
        let nodes = self.parse.nodes;
        let mut child = node;
        loop {
            let parent = nodes.parent_id(child);
            if parent == child || matches!(nodes.kind(parent), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)) {
                return None;
            }
            if is_loop(&nodes.kind(parent)) {
                return Some(parent);
            }
            child = parent;
        }
    }

    fn identifier_name_symbol(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        match expression.without_parentheses() {
            Expression::Identifier(identifier) => self.identifier_symbol(identifier),
            _ => None,
        }
    }

    /// `t[k]` of a register holding the tracer's argument tuple
    /// (`t = [a, X, j]` reaching the read): the tuple's element `k`.
    pub(crate) fn tuple_slot(&self, expression: &Expression<'_>) -> Option<NodeId> {
        let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else { return None };
        self.tuple_member_slot(member)
    }

    pub(crate) fn tuple_member_slot(&self, member: &oxc_ast::ast::ComputedMemberExpression<'_>) -> Option<NodeId> {
        let index = integer(&member.expression)?;
        let Expression::Identifier(_) = member.object.without_parentheses() else { return None };
        let tuple = self.expression_node(&member.object)?;
        let (value, _) = self.identifier_value(tuple)?;
        let AstKind::ArrayExpression(array) = self.parse.nodes.kind(value) else { return None };
        let element = array.elements.get(index as usize)?.as_expression()?;
        // A tuple is only read by literal index: a binding whose elements
        // are stored or which is used as a list (`s[1] = true`, `s.push`) is
        // a state array, not a tuple.
        let symbol = self.reference_symbol(tuple)?;
        let nodes = self.parse.nodes;
        // The store of the tuple: a register reused for other values
        // before or after is judged only where the tuple can reach.
        let store = nodes.parent_id(value);
        if !matches!(nodes.kind(store), AstKind::AssignmentExpression(_) | AstKind::VariableDeclarator(_)) {
            return None;
        }
        for reference in self.parse.scoping.get_resolved_references(symbol) {
            let read = reference.node_id();
            if !reference.is_write() {
                if let Some((_, stores)) = self.reaching(read) {
                    let sees = stores.iter().any(|reaching| match reaching {
                        flow::Store::Entry => true,
                        flow::Store::Assign(node) | flow::Store::Declare(node) | flow::Store::Other(node) => *node == store,
                    });
                    if !sees {
                        continue;
                    }
                }
            }
            let parent = nodes.parent_id(read);
            match nodes.kind(parent) {
                AstKind::ComputedMemberExpression(element) if element.object.span() == nodes.kind(read).span() => {
                    if integer(&element.expression).is_none() {
                        return None;
                    }
                    let target = nodes.parent_id(parent);
                    match nodes.kind(target) {
                        AstKind::AssignmentExpression(assignment) if assignment.left.span() == element.span => return None,
                        AstKind::UpdateExpression(_) => return None,
                        _ => {}
                    }
                }
                AstKind::StaticMemberExpression(member) if member.object.span() == nodes.kind(read).span() => return None,
                _ => {}
            }
        }
        self.expression_node(element)
    }

    /// The register a read-only copy (`let t = acc`, never written again)
    /// copies, through such copies; else `symbol` itself.
    pub(crate) fn copy_root(&self, symbol: SymbolId) -> SymbolId {
        let mut symbol = symbol;
        for _ in 0..4 {
            let declaration = self.parse.scoping.symbol_declaration(symbol);
            let AstKind::VariableDeclarator(declarator) = self.parse.nodes.kind(declaration) else { break };
            let Some(source) = declarator.init.as_ref().and_then(|init| self.identifier_name_symbol(init)) else { break };
            if source == symbol || self.parse.scoping.get_resolved_references(symbol).any(|reference| reference.is_write()) {
                break;
            }
            symbol = source;
        }
        symbol
    }

    /// The binding an operand names, directly or through an argument
    /// tuple slot ([`Self::tuple_slot`]).
    fn operand_symbol(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        self.identifier_name_symbol(expression)
            .or_else(|| self.tuple_slot(expression).and_then(|node| self.reference_symbol(node)))
    }

    /// `++x` / `x++` (`Some(true)`) or `--x` / `x--` (`Some(false)`) of
    /// `symbol` among the nodes of `span`; also `x += 1` / `x -= 1`, and a
    /// step through a loop-carried copy: `for (;; j = Number(q)) { q = j - 1;
    /// … X[q] … }` (`q` is `j` stepped, `j` a copy of `q`).
    fn step_of(&self, symbol: SymbolId, span: Span) -> Option<bool> {
        use oxc_ast::ast::{AssignmentOperator, AssignmentTarget};
        // `t` through parentheses and `Number(t)`: the binding copied.
        let copied = |expression: &Expression<'_>| -> Option<SymbolId> {
            match expression.without_parentheses() {
                Expression::CallExpression(call) if call.arguments.len() == 1 => match call.callee.without_parentheses() {
                    Expression::Identifier(callee) if callee.name == "Number" && self.identifier_symbol(callee).is_none() => {
                        self.identifier_name_symbol(call.arguments[0].as_expression()?)
                    }
                    _ => None,
                },
                other => self.identifier_name_symbol(other),
            }
        };
        // `t + 1` / `t - 1`: the binding stepped and the direction.
        // Also `Number(t - 1)` (the tracer's numeric copy of the step).
        let stepped = |expression: &Expression<'_>| -> Option<(SymbolId, bool)> {
            let expression = match expression.without_parentheses() {
                Expression::CallExpression(call) if call.arguments.len() == 1 => match call.callee.without_parentheses() {
                    Expression::Identifier(callee) if callee.name == "Number" && self.identifier_symbol(callee).is_none() => {
                        call.arguments[0].as_expression()?
                    }
                    _ => return None,
                },
                other => other,
            };
            let Expression::BinaryExpression(binary) = expression.without_parentheses() else { return None };
            let up = match binary.operator {
                BinaryOperator::Addition => true,
                BinaryOperator::Subtraction => false,
                _ => return None,
            };
            (self.constant_integer(&binary.right) == Some(1)).then_some(())?;
            Some((copied(&binary.left)?, up))
        };
        let mut steps: Vec<(SymbolId, bool)> = Vec::new();
        let mut assigned: Vec<(SymbolId, Option<(SymbolId, bool)>, Option<SymbolId>)> = Vec::new();
        for id in self.nodes_within(span) {
            match self.parse.nodes.kind(id) {
                AstKind::UpdateExpression(update) => {
                    let oxc_ast::ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(target) = &update.argument else {
                        continue;
                    };
                    if let Some(target) = self.identifier_symbol(target) {
                        steps.push((target, update.operator == oxc_ast::ast::UpdateOperator::Increment));
                    }
                }
                AstKind::VariableDeclarator(declarator) => {
                    let (oxc_ast::ast::BindingPattern::BindingIdentifier(identifier), Some(init)) = (&declarator.id, &declarator.init) else {
                        continue;
                    };
                    if let Some(target) = identifier.symbol_id.get() {
                        assigned.push((target, stepped(init), copied(init)));
                    }
                }
                AstKind::AssignmentExpression(assignment) => {
                    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else { continue };
                    let Some(target) = self.identifier_symbol(target) else { continue };
                    match assignment.operator {
                        AssignmentOperator::Addition | AssignmentOperator::Subtraction if integer(&assignment.right) == Some(1) => {
                            steps.push((target, assignment.operator == AssignmentOperator::Addition));
                        }
                        AssignmentOperator::Assign => {
                            assigned.push((target, stepped(&assignment.right), copied(&assignment.right)));
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        // `x = c ± 1` where `c` is `x` or a binding only ever assigned `x`
        // in the loop; `x = c` where `c` is only ever assigned `x ± 1`.
        let only_copy_of = |copy: SymbolId, of: SymbolId| {
            // Initialisers to a constant (`let i = 0`) are not loop stores.
            let mut stores = assigned.iter().filter(|(target, step, source)| *target == copy && (step.is_some() || source.is_some())).peekable();
            stores.peek().is_some() && stores.all(|(_, step, source)| step.is_none() && *source == Some(of))
        };
        let only_step_of = |stepper: SymbolId, of: SymbolId| -> Option<bool> {
            let mut found = None;
            for (target, step, _) in assigned.iter().filter(|(target, step, source)| *target == stepper && (step.is_some() || source.is_some())) {
                let _ = target;
                let (source, up) = (*step)?;
                if source != of || found.is_some_and(|known| known != up) {
                    return None;
                }
                found = Some(up);
            }
            found
        };
        for (target, step, source) in &assigned {
            if *target != symbol {
                continue;
            }
            match (step, source) {
                (Some((from, up)), _) if *from == symbol || only_copy_of(*from, symbol) => steps.push((symbol, *up)),
                (None, Some(from)) if *from != symbol => {
                    if let Some(up) = only_step_of(*from, symbol) {
                        steps.push((symbol, up));
                    }
                }
                _ => {}
            }
        }
        let mut found = None;
        for (target, up) in steps {
            if target != symbol {
                continue;
            }
            match found {
                None => found = Some(up),
                Some(previous) if previous == up => {}
                Some(_) => return None,
            }
        }
        found
    }

    /// The tracer's descending loop without an update expression of `j`:
    /// `for (;;) { let j = n - 1; if (j >= 0) { …X[j]…; n = Number(j); continue; } … }`
    /// or `for (;; n = Number(j)) { j = n - 1; … }` — every definition of `j`
    /// is `n - 1` in the loop and `n` only takes `j` (or `Number(j)`) back
    /// in the loop: `Some(false)`.
    fn counted_down(&self, index: SymbolId, span: Span) -> Option<bool> {
        let nodes = self.parse.nodes;
        let inside = |at: Span| span.start <= at.start && at.end <= span.end;
        // Every definition of `j` is `n - 1` inside the loop (one `n`); a
        // declaration without initializer outside it is allowed.
        let minus_one_of = |value: &Expression<'_>| match value.without_parentheses() {
            Expression::BinaryExpression(minus) if minus.operator == BinaryOperator::Subtraction && integer(&minus.right) == Some(1) => {
                self.identifier_name_symbol(&minus.left)
            }
            _ => None,
        };
        // `n - 1`, or a copy of a binding declared in the loop as `n - 1`
        // and never written again (`let t = n - 1; j = t;`).
        let minus_one = |value: &Expression<'_>| {
            minus_one_of(value).or_else(|| {
                let copied = self.identifier_name_symbol(value)?;
                let declaration = nodes.kind(self.parse.scoping.symbol_declaration(copied)).span();
                if !inside(declaration) {
                    return None;
                }
                minus_one_of(self.single_init(copied)?)
            })
        };
        let mut bound: Option<SymbolId> = None;
        let mut agree = |symbol: Option<SymbolId>| -> bool {
            match (symbol, bound) {
                (Some(symbol), None) => {
                    bound = Some(symbol);
                    true
                }
                (Some(symbol), Some(known)) => symbol == known,
                (None, _) => false,
            }
        };
        if let AstKind::VariableDeclarator(declarator) = nodes.kind(self.parse.scoping.symbol_declaration(index)) {
            if let Some(init) = &declarator.init {
                if !inside(declarator.span) || !agree(minus_one(init)) {
                    return None;
                }
            }
        }
        for reference in self.parse.scoping.get_resolved_references(index) {
            if !reference.is_write() {
                continue;
            }
            let parent = nodes.parent_id(reference.node_id());
            let AstKind::AssignmentExpression(assignment) = nodes.kind(parent) else { return None };
            if assignment.operator != oxc_ast::ast::AssignmentOperator::Assign
                || !inside(assignment.span)
                || !agree(minus_one(&assignment.right))
            {
                return None;
            }
        }
        let bound = bound?;
        let takes_index = |value: &Expression<'_>| match value.without_parentheses() {
            Expression::CallExpression(call) => {
                matches!(call.callee.without_parentheses(), Expression::Identifier(callee) if callee.name == "Number")
                    && call.arguments.len() == 1
                    && call.arguments[0].as_expression().is_some_and(|argument| self.identifier_name_symbol(argument) == Some(index))
            }
            other => self.identifier_name_symbol(other) == Some(index),
        };
        // `n` only ever takes `j` back inside the loop.
        let mut steps = 0;
        for reference in self.parse.scoping.get_resolved_references(bound) {
            if !reference.is_write() {
                continue;
            }
            let parent = nodes.parent_id(reference.node_id());
            if !inside(nodes.kind(parent).span()) {
                continue;
            }
            match nodes.kind(parent) {
                AstKind::AssignmentExpression(assignment)
                    if assignment.operator == oxc_ast::ast::AssignmentOperator::Assign && takes_index(&assignment.right) =>
                {
                    steps += 1
                }
                _ => return None,
            }
        }
        (steps > 0).then_some(false)
    }

    /// An integer literal, or a binding that only ever holds one: its
    /// declaration initializes it to the literal and every write assigns
    /// the same literal (`let d = 1; … d = 1;`, the tracer's step register).
    fn constant_integer(&self, expression: &Expression<'_>) -> Option<u32> {
        if let Some(value) = integer(expression) {
            return Some(value);
        }
        let symbol = self.identifier_name_symbol(expression)?;
        let nodes = self.parse.nodes;
        let AstKind::VariableDeclarator(declarator) = nodes.kind(self.parse.scoping.symbol_declaration(symbol)) else {
            return None;
        };
        let value = integer(declarator.init.as_ref()?)?;
        for reference in self.parse.scoping.get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            match nodes.kind(nodes.parent_id(reference.node_id())) {
                AstKind::AssignmentExpression(assignment)
                    if assignment.operator == oxc_ast::ast::AssignmentOperator::Assign && integer(&assignment.right) == Some(value) => {}
                _ => return None,
            }
        }
        Some(value)
    }

    /// The initializer of a `let`/`const` binding that is never written again.
    fn single_init(&self, symbol: SymbolId) -> Option<&Expression<'_>> {
        if self.parse.scoping.get_resolved_references(symbol).any(|reference| reference.is_write()) {
            return None;
        }
        let AstKind::VariableDeclarator(declarator) = self.parse.nodes.kind(self.parse.scoping.symbol_declaration(symbol)) else {
            return None;
        };
        declarator.init.as_ref()
    }

    /// The binding `symbol` copies when it is declared from a plain read of
    /// another binding and never written (`let t = acc`); else `symbol`.
    fn copied_binding(&self, symbol: SymbolId) -> SymbolId {
        match self.single_init(symbol).map(|init| init.without_parentheses()) {
            Some(Expression::Identifier(identifier)) => self.identifier_symbol(identifier).unwrap_or(symbol),
            _ => symbol,
        }
    }

    /// An array literal of strings, through plain copies.
    fn string_list(&self, node: NodeId) -> Option<Vec<String>> {
        let mut node = node;
        for _ in 0..TERM_HOPS {
            match self.parse.nodes.kind(node) {
                AstKind::ArrayExpression(array) => {
                    return array
                        .elements
                        .iter()
                        .map(|element| match element.as_expression()?.without_parentheses() {
                            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
                            // A numeric candidate (`(grid: 0)`), as written.
                            Expression::NumericLiteral(literal) if literal.value.fract() == 0.0 => {
                                Some(format!("{}", literal.value as i64))
                            }
                            _ => None,
                        })
                        .collect();
                }
                // `[a, L][k]`: the literal element `k`.
                AstKind::ComputedMemberExpression(element) => {
                    let (Expression::ArrayExpression(array), Some(index)) = (element.object.without_parentheses(), integer(&element.expression)) else {
                        return None;
                    };
                    node = self.expression_node(array.elements.get(index as usize)?.as_expression()?)?;
                }
                AstKind::IdentifierReference(_) => {
                    // A list binding: its declaration (lists are not
                    // reassigned between fill and read).
                    let symbol = self.reference_symbol(node)?;
                    let declaration = self.parse.scoping.symbol_declaration(symbol);
                    // A list binding declared without a value (`let L;
                    // … L = [...]`): its nearest definition.
                    node = match self.parse.nodes.kind(declaration) {
                        AstKind::VariableDeclarator(declarator) if declarator.init.is_some() => {
                            self.expression_node(declarator.init.as_ref()?)?
                        }
                        _ => self.identifier_value(node)?.0,
                    };
                }
                _ => return None,
            }
        }
        None
    }

    /// The probed strings of a boolean list `list` (a binding) in index
    /// order: filled by `list.push(test(L[i]))` in an ascending loop over a
    /// string list `L` (a `push(false)` in the `catch` of the same try is
    /// the failure value), or `list = helper(…, L)` where the helper returns
    /// `param.map(…)` of that argument.
    fn boolean_list_items(&self, list: SymbolId) -> Option<Vec<String>> {
        let nodes = self.parse.nodes;
        let mut found: Option<Vec<String>> = None;
        let take = |items: Vec<String>, found: &mut Option<Vec<String>>| -> bool {
            match found {
                None => {
                    *found = Some(items);
                    true
                }
                Some(previous) => *previous == items,
            }
        };
        for reference in self.parse.scoping.get_resolved_references(list) {
            let node = reference.node_id();
            let parent = nodes.parent_id(node);
            // list.push(arg)
            if let AstKind::StaticMemberExpression(member) = nodes.kind(parent) {
                if member.property.name != "push" {
                    continue;
                }
                let call = nodes.parent_id(parent);
                let AstKind::CallExpression(call_expression) = nodes.kind(call) else { continue };
                let [argument] = call_expression.arguments.as_slice() else { return None };
                let Some(argument) = argument.as_expression() else { return None };
                if matches!(argument.without_parentheses(), Expression::BooleanLiteral(_)) {
                    continue;
                }
                // The argument reads `L[i]`, `i` the ascending loop index.
                let argument = self.expression_node(argument)?;
                let looped = self.enclosing_loop(call)?;
                let mut items = None;
                for id in self.nodes_within(nodes.kind(argument).span()) {
                    // `L[i]` itself, or a register copied from it in the
                    // loop (`k = L[i]; list.push(test(k))`).
                    let id = match nodes.kind(id) {
                        AstKind::IdentifierReference(_) => match self.identifier_value(id) {
                            Some((value, _)) if matches!(nodes.kind(value), AstKind::ComputedMemberExpression(_)) => value,
                            _ => continue,
                        },
                        _ => id,
                    };
                    let AstKind::ComputedMemberExpression(element) = nodes.kind(id) else { continue };
                    let Some(index) = self.identifier_name_symbol(&element.expression) else { continue };
                    if self.step_of(index, nodes.kind(looped).span()) != Some(true) {
                        continue;
                    }
                    if let Some(list) = self.expression_node(&element.object).and_then(|object| self.string_list(object)) {
                        items = Some(list);
                    }
                }
                if !take(items?, &mut found) {
                    return None;
                }
                continue;
            }
            // list = helper(…, L)
            if !reference.is_write() {
                continue;
            }
            let AstKind::AssignmentExpression(assignment) = nodes.kind(parent) else { continue };
            match assignment.right.without_parentheses() {
                // `list = []` before the fill.
                Expression::ArrayExpression(array) if array.elements.is_empty() => {}
                right => {
                    let items = self.mapped_items(right)?;
                    if !take(items, &mut found) {
                        return None;
                    }
                }
            }
        }
        // let list = helper(…, L)
        let declaration = self.parse.scoping.symbol_declaration(list);
        if let AstKind::VariableDeclarator(declarator) = nodes.kind(declaration) {
            if let Some(init) = &declarator.init {
                if let Some(items) = self.mapped_items(init) {
                    if !take(items, &mut found) {
                        return None;
                    }
                }
            }
        }
        found
    }

    /// `helper(…, L)` where the helper returns `param.map(…)` of the
    /// string-list argument `L`: the items of `L`.
    fn mapped_items(&self, value: &Expression<'_>) -> Option<Vec<String>> {
        let Expression::CallExpression(call) = value.without_parentheses() else { return None };
        self.mapped_call(call)
    }

    fn mapped_call(&self, call: &oxc_ast::ast::CallExpression<'_>) -> Option<Vec<String>> {
        let helper = self.identifier_name_symbol(&call.callee)?;
        let (position, items) = call.arguments.iter().enumerate().find_map(|(position, argument)| {
            let node = self.expression_node(argument.as_expression()?)?;
            self.string_list(node).map(|items| (position, items))
        })?;
        if self.maps_parameter(helper, position) {
            return Some(items);
        }
        // A register callee (`f = helper; f = f(w, "q", [...])`): the
        // binding its reaching definition copies.
        let callee = self.expression_node(&call.callee)?;
        let (value, Updates::Or(ors)) = self.identifier_value(callee)? else { return None };
        if !ors.is_empty() || !matches!(self.parse.nodes.kind(value), AstKind::IdentifierReference(_)) {
            return None;
        }
        let copied = self.reference_symbol(value)?;
        (copied != helper && self.maps_parameter(copied, position)).then_some(items)
    }

    /// The function bound to `helper` returns `p.map(…)` of its parameter at
    /// `position` (index-preserving).
    fn maps_parameter(&self, helper: SymbolId, position: usize) -> bool {
        let nodes = self.parse.nodes;
        let mut symbol = helper;
        for _ in 0..TERM_HOPS {
            let declaration = self.parse.scoping.symbol_declaration(symbol);
            let function = match nodes.kind(declaration) {
                AstKind::Function(_) => declaration,
                AstKind::VariableDeclarator(declarator) => match declarator.init.as_ref().map(|init| init.without_parentheses()) {
                    Some(Expression::Identifier(identifier)) => match self.identifier_symbol(identifier) {
                        Some(next) => {
                            symbol = next;
                            continue;
                        }
                        None => return false,
                    },
                    Some(init) => match self.expression_node(init) {
                        Some(node) => node,
                        None => return false,
                    },
                    None => return false,
                },
                _ => return false,
            };
            let (params, body) = match nodes.kind(function) {
                AstKind::Function(function) => (&function.params, function.body.as_ref().map(|body| body.span)),
                AstKind::ArrowFunctionExpression(arrow) => (&arrow.params, Some(arrow.body.span)),
                _ => return false,
            };
            let Some(parameter) = params.items.get(position) else { return false };
            let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else { return false };
            let Some(parameter) = identifier.symbol_id.get() else { return false };
            let Some(body) = body else { return false };
            return self.nodes_within(body).into_iter().any(|id| {
                let AstKind::ReturnStatement(statement) = nodes.kind(id) else { return false };
                let Some(Expression::CallExpression(call)) = statement.argument.as_ref().map(|argument| argument.without_parentheses()) else {
                    return false;
                };
                let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() else { return false };
                member.property.name == "map" && self.identifier_name_symbol(&member.object) == Some(parameter)
            });
        }
        false
    }

    /// `X[j] << c` (directly or through a binding defined so): the element
    /// and the counter.
    fn shift_term(&self, node: NodeId) -> Option<(NodeId, SymbolId)> {
        let nodes = self.parse.nodes;
        let node = match nodes.kind(node) {
            AstKind::IdentifierReference(_) => self.identifier_value(node)?.0,
            _ => node,
        };
        let AstKind::BinaryExpression(shift) = nodes.kind(node) else { return None };
        if shift.operator != BinaryOperator::ShiftLeft {
            return None;
        }
        let counter = self.identifier_name_symbol(&shift.right)?;
        Some((self.expression_node(&shift.left)?, counter))
    }

    /// The packing loop of the accumulator read by `value` (an identifier)
    /// at a byte write: the nearest `acc = acc | X[j] << c` (or
    /// `acc |= X[j] << c`) before the read, inside a loop that steps `j` one
    /// way and flushes at `c === n`; the items of `X` in stream order.
    pub(crate) fn loop_stream(&self, value: NodeId) -> Result<LoopStream, String> {
        let nodes = self.parse.nodes;
        // A byte written through the argument tuple (`t = [buf, i, acc];
        // t[0][t[1]] = t[2] ^ k()`, `t = [i, acc, k]; buf[t[0]] = t[1] ^
        // t[2]()`): the tuple's value slot.
        let value = match nodes.kind(value) {
            AstKind::BinaryExpression(binary)
                if binary.operator == BinaryOperator::BitwiseXOR && mask_call(&binary.right) =>
            {
                self.tuple_slot(&binary.left).unwrap_or(value)
            }
            _ => value,
        };
        let value = match nodes.kind(value) {
            AstKind::ComputedMemberExpression(member) => self.tuple_member_slot(member).unwrap_or(value),
            _ => value,
        };
        let value = self.loop_value(value);
        // `acc` or, at an in-loop flush, `acc | t` (`t = X[j] << c`).
        let value = match nodes.kind(value) {
            AstKind::BinaryExpression(binary) if binary.operator == BinaryOperator::BitwiseOR => {
                let left = self.tuple_slot(&binary.left).or_else(|| self.expression_node(&binary.left)).ok_or("unresolved operand")?;
                let right = self.expression_node(&binary.right).ok_or("unresolved operand")?;
                if self.shift_term(right).is_some() {
                    left
                } else if self.shift_term(left).is_some() {
                    right
                } else {
                    return Err("not an accumulator".into());
                }
            }
            _ => value,
        };
        // A per-iteration copy (`let t = acc; … (t | x) …`) reads the accumulator.
        let accumulator = self.reference_symbol(value).map(|symbol| self.copied_binding(self.copy_root(symbol))).ok_or("not an accumulator")?;
        let read_at = nodes.kind(value).span().start;
        let read_loop = self.enclosing_loop(value);
        // The nearest packing update before the read.
        let mut best: Option<(u32, NodeId, NodeId, SymbolId)> = None; // (end, update, element, counter)
        for reference in self.parse.scoping.get_resolved_references(accumulator) {
            if !reference.is_write() {
                continue;
            }
            let parent = nodes.parent_id(reference.node_id());
            let AstKind::AssignmentExpression(assignment) = nodes.kind(parent) else { continue };
            // Before the read, or anywhere in the loop around an in-loop
            // flush (`if (c === 8) write(acc | t) else acc = acc | t`).
            let same_loop = read_loop.is_some() && self.enclosing_loop(parent) == read_loop;
            if assignment.span.end > read_at && !same_loop {
                continue;
            }
            let shifted = match (assignment.operator, assignment.right.without_parentheses()) {
                (oxc_ast::ast::AssignmentOperator::BitwiseOR, right) => Some(right),
                (oxc_ast::ast::AssignmentOperator::Assign, Expression::BinaryExpression(binary))
                    if binary.operator == BinaryOperator::BitwiseOR
                        && self.operand_symbol(&binary.left).map(|left| self.copied_binding(self.copy_root(left))) == Some(accumulator) =>
                {
                    Some(binary.right.without_parentheses())
                }
                _ => None,
            };
            let Some(shifted) = shifted
                .and_then(|shifted| self.expression_node(shifted))
                .or_else(|| self.temp_update(&assignment.right, accumulator))
            else {
                continue;
            };
            let Some((element, counter)) = self.shift_term(shifted) else { continue };
            let rank = if same_loop { u32::MAX } else { assignment.span.end };
            if best.is_none_or(|(end, _, _, _)| rank > end) {
                best = Some((rank, parent, element, counter));
            }
        }
        let (_, update, element, counter) = best.ok_or("no packing update")?;
        let looped = self.enclosing_loop(update).ok_or("packing update outside a loop")?;
        let loop_span = nodes.kind(looped).span();
        if self.reassigned_after(accumulator, loop_span, value) {
            return Err("accumulator reassigned after the packing loop".into());
        }
        // `let c = k; … X[j] << c …; if (Number(k) + 1 === n)` (or `++k;
        // if (k === n)`): the shift counter is a per-iteration copy of the
        // counted binding, so the flush test may read that binding.
        let counter_source = {
            let declaration = self.parse.scoping.symbol_declaration(counter);
            match nodes.kind(declaration) {
                AstKind::VariableDeclarator(declarator)
                    if !self.parse.scoping.get_resolved_references(counter).any(|reference| reference.is_write()) =>
                {
                    declarator.init.as_ref().and_then(|init| match init.without_parentheses() {
                        Expression::Identifier(_) => self.identifier_name_symbol(init),
                        _ => None,
                    })
                }
                _ => None,
            }
        };
        // The flush size: `if (c === n)` in the loop.
        let byte = self
            .nodes_within(loop_span)
            .into_iter()
            .find_map(|id| match nodes.kind(id) {
                AstKind::BinaryExpression(test)
                    if matches!(test.operator, BinaryOperator::StrictEquality | BinaryOperator::Equality) =>
                {
                    // `c === 8` (counter stepped before the test) or
                    // `c + 1 === 8` (stepped in the other arm): 8 per byte.
                    // The counter, or `let v = Number(c)` copied from it.
                    let is_counter = |side: &Expression<'_>| {
                        let Some(symbol) = self.identifier_name_symbol(side) else { return false };
                        if symbol == counter || Some(symbol) == counter_source {
                            return true;
                        }
                        let declaration = self.parse.scoping.symbol_declaration(symbol);
                        let AstKind::VariableDeclarator(declarator) = nodes.kind(declaration) else { return false };
                        let copied = match declarator.init.as_ref().map(|init| init.without_parentheses()) {
                            Some(Expression::CallExpression(call)) if call.arguments.len() == 1 => match call.callee.without_parentheses() {
                                Expression::Identifier(callee) if callee.name == "Number" && self.identifier_symbol(callee).is_none() => {
                                    call.arguments[0].as_expression().and_then(|argument| self.identifier_name_symbol(argument))
                                }
                                _ => None,
                            },
                            Some(init) => self.identifier_name_symbol(init),
                            None => None,
                        };
                        copied == Some(counter) && !self.parse.scoping.get_resolved_references(symbol).any(|reference| reference.is_write())
                    };
                    let counted = |side: &Expression<'_>| match side.without_parentheses() {
                        Expression::BinaryExpression(sum) if sum.operator == BinaryOperator::Addition => {
                            is_counter(&sum.left) && integer(&sum.right) == Some(1)
                        }
                        // `n = Number(c) + 1; if (n === 8)` (a tracer spelling).
                        Expression::Identifier(_) if !is_counter(side) => self
                            .identifier_name_symbol(side)
                            .and_then(|symbol| self.single_init(symbol))
                            .is_some_and(|init| match init.without_parentheses() {
                                Expression::BinaryExpression(sum) if sum.operator == BinaryOperator::Addition && integer(&sum.right) == Some(1) => {
                                    match sum.left.without_parentheses() {
                                        Expression::CallExpression(call)
                                            if matches!(call.callee.without_parentheses(), Expression::Identifier(callee) if callee.name == "Number")
                                                && call.arguments.len() == 1 =>
                                        {
                                            call.arguments[0].as_expression().and_then(|argument| self.identifier_name_symbol(argument)).is_some_and(|argument| argument == counter || Some(argument) == counter_source)
                                        }
                                        other => is_counter(other),
                                    }
                                }
                                _ => false,
                            }),
                        other => is_counter(other),
                    };
                    if counted(&test.left) {
                        integer(&test.right)
                    } else if counted(&test.right) {
                        integer(&test.left)
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .or_else(|| self.flush_size_more(loop_span, counter))
            .ok_or("no flush test")?;
        if byte == 0 || byte > 8 {
            return Err(format!("flush size {byte}"));
        }
        // The element `X[j]` and the loop direction over `j`.
        // `X[j]`, or a per-iteration copy of it declared in the loop (`let v
        // = X[j]; … v << c …`).
        let member = match nodes.kind(element) {
            AstKind::ComputedMemberExpression(member) => member,
            AstKind::IdentifierReference(_) => {
                let copied = self.reference_symbol(element).and_then(|symbol| {
                    let declaration = nodes.kind(self.parse.scoping.symbol_declaration(symbol)).span();
                    if declaration.start < loop_span.start || declaration.end > loop_span.end {
                        return None;
                    }
                    match self.single_init(symbol)?.without_parentheses() {
                        Expression::ComputedMemberExpression(member) => self.node_with_span(member.span),
                        _ => None,
                    }
                });
                match copied.map(|node| nodes.kind(node)) {
                    Some(AstKind::ComputedMemberExpression(member)) => member,
                    _ => return Err("packed value is not a list element".into()),
                }
            }
            _ => return Err("packed value is not a list element".into()),
        };
        let list = self.operand_symbol(&member.object).ok_or("list is not a binding")?;
        // `X[i]`, also `X[i - 1]` / `X[i + 1]` (an offset keeps the direction).
        let index = self
            .operand_symbol(&member.expression)
            .or_else(|| match member.expression.without_parentheses() {
                Expression::BinaryExpression(offset)
                    if matches!(offset.operator, BinaryOperator::Addition | BinaryOperator::Subtraction)
                        && self.constant_integer(&offset.right).is_some() =>
                {
                    self.identifier_name_symbol(&offset.left)
                }
                _ => None,
            })
            .ok_or("index is not a binding")?;
        let ascending = self
            .step_of(index, loop_span)
            .or_else(|| self.counted_down(index, loop_span))
            .ok_or("loop direction unknown")?;
        // The list's reaching definition at the read (`x = helper(…, L)`,
        // registers are reused), else a list filled by `push`.
        let reaching = self
            .tuple_slot(&member.object)
            .or_else(|| self.expression_node(&member.object))
            .and_then(|object| self.identifier_value(object))
            .and_then(|(value, _)| match self.parse.nodes.kind(value) {
                AstKind::CallExpression(call) => self.mapped_call(call),
                _ => None,
            });
        let mut items = match reaching {
            Some(items) => items,
            None => self
                .boolean_list_items(list)
                .or_else(|| self.more_list_items(list))
                .or_else(|| {
                    let object = self.expression_node(&member.object)?;
                    self.windowed_fill_items(list, object, element)
                })
                .ok_or("probed list not found")?,
        };
        self.unflushed_fits(loop_span, counter, items.len())?;
        if !ascending {
            items.reverse();
        }
        Ok(LoopStream {
            items,
            byte,
            looped: loop_span,
        })
    }
}

// ===========================================================================
// Store contexts
// ===========================================================================

/// Function boundaries crossed when reading a store's context.
const CONTEXT_FUNCTIONS: usize = 2;

impl FinderCtx<'_, '_, '_> {
    /// Tokens inside `node` (no definitions followed).
    fn tokens_in(&self, node: NodeId, depth: usize, out: &mut Vec<(String, usize)>) {
        let slice = self.slice_of(node, 0);
        for map in [&slice.members, &slice.arguments, &slice.strings, &slice.globals] {
            out.extend(map.keys().map(|name| (name.clone(), depth)));
        }
        out.extend(slice.calls.keys().map(|(method, _, _)| (method.clone(), depth)));
    }

    /// Tokens inside `node` but outside the function literals it contains
    /// (the use site of a callback, without the callback's own body).
    fn tokens_outside_functions(&self, node: NodeId, depth: usize, out: &mut Vec<(String, usize)>) {
        let nodes = self.parse.nodes;
        let inside = self.nodes_within(nodes.kind(node).span());
        let functions: Vec<Span> = inside
            .iter()
            .filter(|id| matches!(nodes.kind(**id), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)))
            .map(|id| nodes.kind(*id).span())
            .collect();
        let generated = |name: &str| name.starts_with('$');
        for id in inside {
            let span = nodes.kind(id).span();
            if functions.iter().any(|function| function.start <= span.start && span.end <= function.end) {
                continue;
            }
            let name = match nodes.kind(id) {
                AstKind::StaticMemberExpression(member) => Some(member.property.name.to_string()),
                AstKind::StringLiteral(literal) => Some(literal.value.to_string()),
                AstKind::IdentifierReference(identifier) if self.identifier_symbol(identifier).is_none() => {
                    Some(identifier.name.to_string())
                }
                _ => None,
            };
            if let Some(name) = name.filter(|name| !generated(name)) {
                out.push((name, depth));
            }
        }
    }

    /// The stores of a state boolean read by `base`: `flag = v` for a local
    /// binding `flag`, `flags[k] = v` for an element `flags[k]` (literal
    /// `k`). Returns the assignment nodes.
    fn state_stores(&self, base: NodeId) -> Vec<NodeId> {
        let nodes = self.parse.nodes;
        let (symbol, index) = match nodes.kind(base) {
            AstKind::IdentifierReference(_) => {
                // A register read gets its meaning from its reaching
                // definition; only a state binding (reaching definition a
                // literal, or none) is described by its stores.
                let state = match self.identifier_value(base) {
                    None => true,
                    Some((value, _)) => matches!(
                        nodes.kind(value),
                        AstKind::BooleanLiteral(_) | AstKind::NullLiteral(_) | AstKind::NumericLiteral(_)
                    ),
                };
                match self.reference_symbol(base) {
                    Some(symbol) if state => (symbol, None),
                    _ => return Vec::new(),
                }
            }
            AstKind::ComputedMemberExpression(member) => {
                let (Expression::Identifier(_), Some(index)) = (member.object.without_parentheses(), integer(&member.expression))
                else {
                    return Vec::new();
                };
                let Some(object) = self.expression_node(&member.object) else { return Vec::new() };
                match self.reference_symbol(object) {
                    Some(symbol) => (symbol, Some(index)),
                    None => return Vec::new(),
                }
            }
            _ => return Vec::new(),
        };
        let mut out = Vec::new();
        for reference in self.parse.scoping.get_resolved_references(symbol) {
            let node = reference.node_id();
            let target = match index {
                None if reference.is_write() => node,
                None => continue,
                Some(index) => {
                    let parent = nodes.parent_id(node);
                    match nodes.kind(parent) {
                        AstKind::ComputedMemberExpression(member) if integer(&member.expression) == Some(index) => parent,
                        _ => continue,
                    }
                }
            };
            // The assignment whose left side is the target.
            let mut child = target;
            for _ in 0..3 {
                let parent = nodes.parent_id(child);
                if parent == child {
                    break;
                }
                if let AstKind::AssignmentExpression(assignment) = nodes.kind(parent) {
                    let left = assignment.left.span();
                    let target_span = nodes.kind(target).span();
                    if left.start <= target_span.start && target_span.end <= left.end && assignment.operator.is_assign() {
                        out.push(parent);
                    }
                    break;
                }
                child = parent;
            }
        }
        out
    }

    /// Context tokens of the stores of the state boolean `base`: the stored
    /// value (depth 1), the tests of the `if` statements enclosing a store
    /// (depth 1 inside the store's own function, deeper beyond), and for a
    /// store inside a nested function the call the function is passed to or
    /// the object literal it is stored in (`addEventListener("x", f)`,
    /// `{get: f}`), one level deeper per function crossed.
    pub(crate) fn store_contexts(&self, base: NodeId) -> Vec<(String, usize)> {
        let nodes = self.parse.nodes;
        let mut out = Vec::new();
        for store in self.state_stores(base) {
            let AstKind::AssignmentExpression(assignment) = nodes.kind(store) else { continue };
            if let Some(right) = self.expression_node(&assignment.right) {
                self.tokens_in(right, 1, &mut out);
            }
            let mut depth = 1;
            let mut child = store;
            let mut crossed = 0;
            loop {
                let parent = nodes.parent_id(child);
                if parent == child {
                    break;
                }
                match nodes.kind(parent) {
                    AstKind::IfStatement(statement) => {
                        let inside_test = statement.test.span().start <= nodes.kind(child).span().start
                            && nodes.kind(child).span().end <= statement.test.span().end;
                        if !inside_test {
                            if let Some(test) = self.expression_node(&statement.test) {
                                self.tokens_in(test, depth, &mut out);
                                // `if (r)` on a register: what `r` was set
                                // to (`r = s.indexOf("#<Navigator>") >= 0`).
                                if let Some(register) = self.identifier_name_symbol(&statement.test) {
                                    let root = self.flow_root(parent);
                                    let before = statement.span.start;
                                    // `const r = …; if (r)`: the declared value.
                                    let declaration = self.parse.scoping.symbol_declaration(register);
                                    if let AstKind::VariableDeclarator(declarator) = nodes.kind(declaration) {
                                        if declarator.span.start < before && self.flow_root(declaration) == root {
                                            if let Some(value) =
                                                declarator.init.as_ref().and_then(|init| self.expression_node(init))
                                            {
                                                self.tokens_in(value, depth + 1, &mut out);
                                            }
                                        }
                                    }
                                    for reference in self.parse.scoping.get_resolved_references(register) {
                                        if !reference.is_write() || nodes.kind(reference.node_id()).span().start >= before {
                                            continue;
                                        }
                                        let write = nodes.parent_id(reference.node_id());
                                        if self.flow_root(write) != root {
                                            continue;
                                        }
                                        if let AstKind::AssignmentExpression(stored) = nodes.kind(write) {
                                            if let Some(value) = self.expression_node(&stored.right) {
                                                self.tokens_in(value, depth + 1, &mut out);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => {
                        crossed += 1;
                        if crossed > CONTEXT_FUNCTIONS {
                            break;
                        }
                        depth += 1;
                        for site in self.function_uses(parent) {
                            self.tokens_outside_functions(site, depth, &mut out);
                        }
                    }
                    AstKind::Program(_) => break,
                    _ => {}
                }
                child = parent;
            }
        }
        out
    }

    /// The innermost function literal containing `node`.
    fn enclosing_function(&self, node: NodeId) -> Option<NodeId> {
        let nodes = self.parse.nodes;
        let mut child = node;
        loop {
            let parent = nodes.parent_id(child);
            if parent == child {
                return None;
            }
            if matches!(nodes.kind(parent), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)) {
                return Some(parent);
            }
            child = parent;
        }
    }

    /// Where a function is used: the call or `new` it is an argument of, or
    /// the object literal it is a property value of (followed to the call the
    /// object is passed to), for the function itself or, for a declaration
    /// or a binding initialised with it, for every read of its name.
    fn function_uses(&self, function: NodeId) -> Vec<NodeId> {
        let nodes = self.parse.nodes;
        let mut values = vec![function];
        let symbol = match nodes.kind(function) {
            AstKind::Function(function) => function.id.as_ref().and_then(|id| id.symbol_id.get()),
            _ => None,
        };
        let symbol = symbol.or_else(|| match nodes.kind(nodes.parent_id(function)) {
            AstKind::VariableDeclarator(declarator) => match &declarator.id {
                oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get(),
                _ => None,
            },
            _ => None,
        });
        if let Some(symbol) = symbol {
            values.extend(
                self.parse
                    .scoping
                    .get_resolved_references(symbol)
                    .filter(|reference| reference.is_read())
                    .map(|reference| reference.node_id()),
            );
        }
        let mut out = Vec::new();
        for value in values {
            let mut node = value;
            for _ in 0..4 {
                let parent = nodes.parent_id(node);
                if parent == node {
                    break;
                }
                match nodes.kind(parent) {
                    AstKind::CallExpression(_) | AstKind::NewExpression(_) => {
                        out.push(parent);
                        break;
                    }
                    AstKind::ObjectExpression(_) => {
                        out.push(parent);
                        // `o = {get: f}` then `defineProperty(x, "k", o)`.
                        node = parent;
                        continue;
                    }
                    AstKind::VariableDeclarator(declarator) => {
                        if let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) = &declarator.id {
                            if let Some(holder) = identifier.symbol_id.get() {
                                out.extend(self.argument_uses(holder, declarator.span.end));
                            }
                        }
                        break;
                    }
                    AstKind::AssignmentExpression(assignment) => {
                        if let oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                            if let Some(holder) = self.identifier_symbol(identifier) {
                                out.extend(self.argument_uses(holder, assignment.span.end));
                            }
                        }
                        break;
                    }
                    AstKind::ObjectProperty(_) | AstKind::ParenthesizedExpression(_) => {}
                    _ => break,
                }
                node = parent;
            }
        }
        out
    }

    /// The call a binding is passed to as an argument first after offset
    /// `after` (registers are reused: a later call passes another value).
    fn argument_uses(&self, symbol: SymbolId, after: u32) -> Vec<NodeId> {
        let nodes = self.parse.nodes;
        self.parse
            .scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_read())
            .filter(|reference| nodes.kind(reference.node_id()).span().start >= after)
            .filter_map(|reference| {
                let node = reference.node_id();
                let parent = nodes.parent_id(node);
                let span = nodes.kind(node).span();
                match nodes.kind(parent) {
                    AstKind::CallExpression(call) if call.arguments.iter().any(|argument| argument.span() == span) => {
                        Some((span.start, parent))
                    }
                    AstKind::NewExpression(_) => Some((span.start, parent)),
                    _ => None,
                }
            })
            .min_by_key(|(start, _)| *start)
            .map(|(_, call)| call)
            .into_iter()
            .collect()
    }
}

// ===========================================================================
// Record labels
// ===========================================================================

/// Where a byte's booleans may come from: the slot's spans and the record
/// labels the byte's terms read (to pick the record construction they come
/// from).
pub(crate) struct Scope {
    pub spans: Vec<crate::signals::schema::SourceSpan>,
    pub labels: BTreeSet<String>,
    /// Labels read as lists (`rec.k[i]`): a record construction storing a
    /// value that is never a list there is not the one read (empty: no
    /// filter).
    pub indexed: BTreeSet<String>,
}

impl FinderCtx<'_, '_, '_> {
    /// The record label a boolean reads: `rec.k` (`label`), `rec.k[i]`
    /// (`label`, `Some(i)`), through plain copies (`x = rec.k; x[i]`).
    fn record_label(&self, base: NodeId) -> Option<(String, Option<u32>)> {
        let nodes = self.parse.nodes;
        let (mut node, index) = match nodes.kind(base) {
            AstKind::ComputedMemberExpression(member) if integer(&member.expression).is_some() => {
                (self.expression_node(&member.object)?, integer(&member.expression))
            }
            _ => (base, None),
        };
        for _ in 0..TERM_HOPS {
            if let Some(label) = self.read_key(node) {
                return Some((label, index));
            }
            match nodes.kind(node) {
                AstKind::IdentifierReference(_) => match self.identifier_value(node)? {
                    (value, Updates::Or(ors)) if ors.is_empty() => node = value,
                    _ => return None,
                },
                _ => return None,
            }
        }
        None
    }

    /// The values a record label was filled with, in the record
    /// constructions of the slot that hold every label of the byte (a copy
    /// record whose value is the same label read from another record is
    /// skipped); for `rec.k[i]` the `i`-th element when the value is an array
    /// literal.
    fn record_values(&self, base: NodeId, scope: &Scope) -> Vec<(NodeId, Option<u32>)> {
        let Some((label, index)) = self.record_label(base) else { return Vec::new() };
        let labels: Vec<&str> = scope.labels.iter().map(String::as_str).collect();
        let mut out = Vec::new();
        for record in self.records_with_keys(&labels) {
            let span = self.parse.nodes.kind(record.node).span();
            if !scope.spans.iter().any(|outer| outer.start <= span.start && span.end <= outer.end) {
                continue;
            }
            if !self.record_fits(&record, scope) {
                continue;
            }
            let Some(mut value) = record.values.get(&label).copied() else { continue };
            if self.read_key(value).as_deref() == Some(label.as_str()) {
                continue;
            }
            let mut element = None;
            if let Some(index) = index {
                for _ in 0..TERM_HOPS {
                    match self.parse.nodes.kind(value) {
                        AstKind::ArrayExpression(array) => {
                            element = array
                                .elements
                                .get(index as usize)
                                .and_then(|element| element.as_expression())
                                .and_then(|expression| self.expression_node(expression));
                            break;
                        }
                        AstKind::IdentifierReference(_) => match self.identifier_value(value) {
                            Some((next, Updates::Or(ors))) if ors.is_empty() => value = next,
                            _ => break,
                        },
                        _ => break,
                    }
                }
            }
            out.push(match element {
                Some(element) => (element, None),
                None => (value, index),
            });
        }
        out
    }
}

// ===========================================================================
// Naming
// ===========================================================================

/// The probes of each flag of a namer.
fn probes(namer: Namer, flags: &[String]) -> Vec<(String, Vec<String>)> {
    match namer {
        Namer::Card { prefixed } => flags
            .iter()
            .map(|flag| {
                let probe = if prefixed {
                    flag.split_once('.').map_or(flag.as_str(), |(_, rest)| rest)
                } else {
                    flag.as_str()
                };
                (flag.clone(), vec![probe.to_string()])
            })
            .collect(),
        Namer::Probes(table) => table
            .iter()
            .map(|(flag, probes)| (flag.to_string(), probes.iter().map(|probe| probe.to_string()).collect()))
            .collect(),
        Namer::Loop => Vec::new(),
    }
}

/// What a boolean reads, token -> smallest depth: the members, string
/// arguments, string literals, globals and called names of its backward
/// slice, the member it reads itself (depth 0, also a `$`-prefixed host
/// name), and for a state binding or flag-array element set elsewhere
/// (`flag = true`, `flags[k] = …`) the contexts of those stores
/// ([`FinderCtx::store_contexts`], one level deeper), and for a record label
/// what the collector stored under it ([`FinderCtx::record_value`]).
fn evidence_map(ctx: &FinderCtx, base: NodeId, scope: &Scope) -> Vec<BTreeMap<String, usize>> {
    // `helper(buf, i, v, recv).m`: what `recv.m` reads, not the store the
    // helper makes on the way ([`FinderCtx::returned_argument`]).
    let slice = ctx.slice_of(ctx.helper_receiver(base).unwrap_or(base), NAME_HOPS);
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    fn keep(out: &mut BTreeMap<String, usize>, name: &str, depth: usize) {
        let entry = out.entry(name.to_string()).or_insert(depth);
        *entry = (*entry).min(depth);
    }
    for map in [&slice.members, &slice.arguments, &slice.strings, &slice.globals] {
        for (name, depth) in map {
            keep(&mut out, name, *depth);
        }
    }
    for ((method, _, _), depth) in &slice.calls {
        keep(&mut out, method, *depth);
    }
    if let Some((_, member)) = ctx.direct_member(base) {
        keep(&mut out, &member, 0);
    }
    for (name, depth) in ctx.store_contexts(base) {
        keep(&mut out, &name, depth);
    }
    // A record label: what the collector stored under it (one level
    // deeper), `@i` for an element of a stored list that is not a literal.
    // One evidence map per candidate record construction.
    let candidates = ctx.record_values(base, scope);
    if candidates.is_empty() {
        return vec![out];
    }
    // `rec.k[i]` filled with a literal list per handler (`[true, false]`
    // in one listener, `[false, true]` in another): the element is the
    // handler's own event. Evidence: the registrations of the functions
    // whose record stores `true` there.
    let literal = |node: NodeId| match ctx.parse.nodes.kind(node) {
        AstKind::BooleanLiteral(literal) => Some(literal.value),
        _ => None,
    };
    if candidates.iter().all(|(value, index)| index.is_none() && literal(*value).is_some()) {
        let mut map = out;
        for (value, _) in candidates.iter().filter(|(value, _)| literal(*value) == Some(true)) {
            if let Some(function) = ctx.enclosing_function(*value) {
                for site in ctx.function_uses(function) {
                    let mut tokens = Vec::new();
                    ctx.tokens_outside_functions(site, 2, &mut tokens);
                    for (name, depth) in tokens {
                        keep(&mut map, &name, depth);
                    }
                }
            }
        }
        return vec![map];
    }
    candidates
        .into_iter()
        .map(|(value, index)| {
            let mut map = out.clone();
            let stored = ctx.slice_of(value, NAME_HOPS);
            for source in [&stored.members, &stored.arguments, &stored.strings, &stored.globals] {
                for (name, depth) in source {
                    keep(&mut map, name, depth + 1);
                }
            }
            for ((method, _, _), depth) in &stored.calls {
                keep(&mut map, method, depth + 1);
            }
            if let AstKind::BinaryExpression(binary) = ctx.parse.nodes.kind(value) {
                if binary.operator == BinaryOperator::Instanceof {
                    keep(&mut map, "instanceof", 1);
                }
            }
            if let Some(index) = index {
                keep(&mut map, &format!("@{index}"), 1);
            }
            map
        })
        .collect()
}

/// The flags whose probes `base` reads at the smallest depth (see
/// [`Namer::Probes`]); a `!x` probe requires that `x` is not read at the
/// flag's depth or nearer.
fn nearest_flags(ctx: &FinderCtx, base: NodeId, scope: &Scope, probes: &[(String, Vec<String>)]) -> Vec<String> {
    // With several candidate record constructions, every candidate must name
    // the same flag.
    let mut names: Option<Vec<String>> = None;
    for evidence in evidence_map(ctx, base, scope) {
        let found = nearest_in(&evidence, probes);
        match &names {
            None => names = Some(found),
            Some(first) if *first == found => {}
            Some(_) => return Vec::new(),
        }
    }
    names.unwrap_or_default()
}

fn nearest_in(evidence: &BTreeMap<String, usize>, probes: &[(String, Vec<String>)]) -> Vec<String> {
    let scored: Vec<(usize, usize, &String)> = probes
        .iter()
        .filter_map(|(flag, probes)| {
            if probes.iter().all(|probe| probe.starts_with('!')) {
                // No positive probe: the flag is named only by elimination.
                return None;
            }
            let mut deepest = 0;
            for probe in probes.iter().filter(|probe| !probe.starts_with('!')) {
                deepest = deepest.max(*evidence.get(probe.as_str())?);
            }
            // `!x`: `x` is not read as near as the flag's own probes.
            for absent in probes.iter().filter_map(|probe| probe.strip_prefix('!')) {
                if evidence.get(absent).is_some_and(|depth| *depth <= deepest) {
                    return None;
                }
            }
            Some((deepest, probes.len(), flag))
        })
        .collect();
    let Some(best) = scored.iter().map(|(depth, _, _)| *depth).min() else {
        return Vec::new();
    };
    let specific = scored
        .iter()
        .filter(|(depth, _, _)| *depth == best)
        .map(|(_, count, _)| *count)
        .max()
        .unwrap_or(0);
    let mut out: Vec<String> = Vec::new();
    for (_, _, flag) in scored.into_iter().filter(|(depth, count, _)| *depth == best && *count == specific) {
        if !out.contains(flag) {
            out.push(flag.clone());
        }
    }
    out
}

/// One packed field's leaves to name: per write site the terms.
struct Target {
    message: usize,
    key: String,
    field: String,
    sites: Vec<Vec<PackedTerm>>,
}

/// Name the bits of every packed flags field bound in `binding` (or only
/// of card field `only`), recording them on the binding. Fails when any of
/// them cannot be derived or named.
/// The packed card fields bound in `binding`, in key order.
pub(crate) fn bound_packed_fields(signal: &str, binding: &FieldBinding) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (_, _, name) in binding.entries() {
        let packed = packed_field(signal, name).is_some() || crate::signals::flags::rules::is_packed(signal, name);
        if packed && !out.iter().any(|known| known == name) {
            out.push(name.to_string());
        }
    }
    out
}

/// [`apply_flags`] for the whole slot; when that fails, field by field. A
/// field whose bits cannot all be derived and named keeps its plain `byte`
/// rendering and is returned with the reason (never a guessed bit).
pub(crate) fn apply_flags_or_fall_back(
    signal: &str,
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
    binding: &mut FieldBinding,
) -> Vec<(String, String)> {
    let mut attempt = binding.clone();
    if apply_flags(signal, view, layout, ctx, &mut attempt, None).is_ok() {
        *binding = attempt;
        return Vec::new();
    }
    let mut fallbacks = Vec::new();
    for field in bound_packed_fields(signal, binding) {
        let mut attempt = binding.clone();
        match apply_flags(signal, view, layout, ctx, &mut attempt, Some(&field)) {
            Ok(()) => *binding = attempt,
            Err(reason) => fallbacks.push((field, reason)),
        }
    }
    fallbacks
}

pub(crate) fn apply_flags(
    signal: &str,
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
    binding: &mut FieldBinding,
    only: Option<&str>,
) -> Result<(), String> {
    // Signals whose booleans are named on their resolved values.
    if crate::signals::flags::rules::handles(signal) {
        return crate::signals::flags::rules::apply_flags(signal, view, layout, ctx, binding, only);
    }
    let wanted: Vec<(usize, String, String)> = binding
        .entries()
        .filter(|(message, key, name)| {
            packed_field(signal, name).is_some()
                && binding.flags(*message, key).is_none()
                && only.is_none_or(|only| only == *name)
        })
        .map(|(message, key, name)| (message, key.to_string(), name.to_string()))
        .collect();
    if wanted.is_empty() {
        return Ok(());
    }
    // Stage 1: the terms of every write site of every packed field (loop
    // fields are named right away).
    let mut targets = Vec::new();
    let mut loops: Vec<(usize, String, String, BTreeMap<String, u32>)> = Vec::new();
    for (message, key, field) in wanted {
        let mut sites = Vec::new();
        for (emission_index, emission) in layout.emissions.iter().enumerate() {
            for (part, (_, part_layout)) in emission.parts.iter().enumerate() {
                if layout.part_message(emission_index, part) != Some(message) {
                    continue;
                }
                let Some((node, offset)) = find_key(&part_layout.fields, &key) else {
                    continue;
                };
                let leaves = emission.part_leaves(part);
                for inner in flag_leaf_offsets(node) {
                    let leaf = leaves
                        .get(offset + inner)
                        .and_then(|index| view.emissions().get(emission.emission)?.leaves.get(*index))
                        .ok_or_else(|| format!("flags {field}: leaf not found"))?;
                    let value = ctx
                        .caller_value_node(leaf)
                        .ok_or_else(|| format!("flags {field}: value not found"))?;
                    // A constant byte (`m ^ 0`): no bits at this site.
                    let Some(value) = ctx.site_value(value) else { continue };
                    if let Some(PackedField { namer: Namer::Loop, .. }) = packed_field(signal, &field) {
                        let stream = ctx.loop_stream(value).map_err(|reason| format!("flags {field}: {reason}"))?;
                        let inside = ctx.flush_site(leaf.write.site(), stream.looped);
                        // Flush bytes (inside the loop): stream positions; the
                        // tail byte: positions from the last full byte on.
                        let full = (stream.items.len() as u32 / stream.byte) * stream.byte;
                        let mut bits = BTreeMap::new();
                        for (position, item) in stream.items.iter().enumerate() {
                            let position = position as u32;
                            let bit = if inside && position < full {
                                position / stream.byte * 8 + position % stream.byte
                            } else if inside {
                                continue;
                            } else if position >= full {
                                position - full
                            } else {
                                continue;
                            };
                            if bits.insert(item.clone(), bit).is_some() {
                                return Err(format!("flags {field}: {item} probed twice"));
                            }
                        }
                        loops.push((message, key.clone(), field.clone(), bits));
                        continue;
                    }
                    let terms = ctx.packed_terms(value).map_err(|reason| format!("flags {field}: {reason}"));
                    sites.push(terms?);
                }
            }
        }
        if sites.is_empty() {
            // Only literal bytes (a message that sends constant zeros).
            continue;
        }
        targets.push(Target {
            message,
            key,
            field,
            sites,
        });
    }
    // Loop fields: every name on the card, every site agreeing.
    let mut loop_bits: BTreeMap<(usize, String), (String, BTreeMap<String, u32>)> = BTreeMap::new();
    for (message, key, field, bits) in loops {
        let packed = packed_field(signal, &field).ok_or("packed field")?;
        // A loop field without `flags_from` uses its own card list.
        let flags = match packed.flags_from {
            [] => card_flags(signal, &field),
            _ => field_flags(signal, packed),
        }
        .ok_or_else(|| format!("flags {field}: no card flags list"))?;
        let bits: BTreeMap<String, u32> =
            bits.into_iter().map(|(item, bit)| (loops::card_item(&item, &flags), bit)).collect();
        if let Some(missing) = bits.keys().find(|name| !flags.contains(name)) {
            return Err(format!("flags {field}: probed item {missing} is not a card flag"));
        }
        match loop_bits.get(&(message, key.clone())) {
            Some((_, previous)) if *previous != bits => {
                return Err(format!("flags {field}: write sites disagree on the bits"))
            }
            _ => {
                loop_bits.insert((message, key), (field, bits));
            }
        }
    }
    for ((message, key), (_, bits)) in loop_bits {
        binding.set_flags(message, key, bits);
    }
    // Stage 2: name the terms.
    let mut receiver_members: BTreeMap<(SymbolId, Option<NodeId>), BTreeSet<String>> = BTreeMap::new();
    for target in &targets {
        for term in target.sites.iter().flatten() {
            if let Some((receiver, member)) = term.base.and_then(|base| ctx.receiver_member(base)) {
                receiver_members.entry(receiver).or_default().insert(member);
            }
        }
    }
    let slot = loops::slot_scope(ctx, view, targets.iter().flat_map(|target| target.sites.iter().flatten()));
    // Per target, per site: bit -> name (`None`: a constant bit).
    let mut named: Vec<Vec<BTreeMap<u32, Option<String>>>> = Vec::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut unmatched: Vec<String> = Vec::new();
    let mut all_flags: Vec<String> = Vec::new();
    for target in &targets {
        let packed = packed_field(signal, &target.field).ok_or("packed field")?;
        let flags = field_flags(signal, packed).ok_or_else(|| format!("flags {}: no card flags list", target.field))?;
        for flag in &flags {
            if !all_flags.contains(flag) {
                all_flags.push(flag.clone());
            }
        }
        let probes = probes(packed.namer, &flags);
        let mut sites = Vec::new();
        for terms in &target.sites {
            let scope = site_scope(ctx, view, terms);
            let mut site_bits = BTreeMap::new();
            for term in terms {
                let Some(base) = term.base else {
                    site_bits.insert(term.bit, None);
                    continue;
                };
                let mut names = nearest_flags(ctx, base, &scope, &probes);
                if names.is_empty() {
                    names = loops::retry_names(ctx, base, &slot, &probes);
                }
                if names.len() > 1 {
                    if let Namer::Card { prefixed: true } = packed.namer {
                        names = by_receiver(ctx, base, &names, &flags, &receiver_members);
                    }
                }
                let name = match names.as_slice() {
                    [name] => name.clone(),
                    [] => {
                        // Named only by elimination, like a constant bit.
                        let span = ctx.parse.nodes.kind(base).span();
                        unmatched.push(format!(
                            "flags {}: bit {} (`{}`) reads no card flag",
                            target.field,
                            term.bit,
                            snippet(ctx, span)
                        ));
                        site_bits.insert(term.bit, None);
                        continue;
                    }
                    many => {
                        return Err(format!(
                            "flags {}: bit {} reads {} card flags ({})",
                            target.field,
                            term.bit,
                            many.len(),
                            many.join(", ")
                        ))
                    }
                };
                used.insert(name.clone());
                site_bits.insert(term.bit, Some(name));
            }
            sites.push(site_bits);
        }
        named.push(sites);
    }
    // A constant bit (a folded test) or a boolean without evidence (a state
    // set only in a `catch`) is named only by elimination: exactly one card
    // flag of the signal is read by no other bit, and every such bit of the
    // slot is that one bit position of one field.
    // Keyed by field: one field may be sent in several messages.
    let constants: BTreeSet<(&str, u32)> = named
        .iter()
        .enumerate()
        .flat_map(|(index, sites)| {
            let field = targets[index].field.as_str();
            sites
                .iter()
                .flat_map(move |site| site.iter().filter(|(_, name)| name.is_none()).map(move |(bit, _)| (field, *bit)))
        })
        .collect();
    let leftover: Vec<&String> = all_flags.iter().filter(|flag| !used.contains(*flag)).collect();
    let constant_name = match (constants.len(), leftover.as_slice()) {
        (0, _) => None,
        (1, [only]) => Some((*only).clone()),
        _ => {
            return Err(unmatched.into_iter().next().unwrap_or_else(|| {
                format!(
                    "flags: {} constant or unmatched bit(s) with {} unnamed card flag(s)",
                    constants.len(),
                    leftover.len()
                )
            }))
        }
    };
    let mut claimed: BTreeMap<(usize, String), String> = BTreeMap::new();
    for (target, sites) in targets.into_iter().zip(named) {
        let mut bits: Option<BTreeMap<String, u32>> = None;
        for site in sites {
            let mut site_bits = BTreeMap::new();
            for (bit, name) in site {
                let name = match name {
                    Some(name) => name,
                    None => constant_name.clone().ok_or("flags: unnamed constant bit")?,
                };
                if site_bits.insert(name.clone(), bit).is_some() {
                    return Err(format!("flags {}: {name} on two bits", target.field));
                }
            }
            match &bits {
                None => bits = Some(site_bits),
                Some(first) if *first == site_bits => {}
                Some(_) => return Err(format!("flags {}: write sites disagree on the bits", target.field)),
            }
        }
        let bits = bits.unwrap_or_default();
        // One boolean is packed once per message.
        for name in bits.keys() {
            if let Some(other) = claimed.insert((target.message, name.clone()), target.field.clone()) {
                return Err(format!("flags: {name} on {other} and {}", target.field));
            }
        }
        binding.set_flags(target.message, target.key, bits);
    }
    Ok(())
}

/// The naming scope of one write site: the slot spans and the record labels
/// its terms read.
fn site_scope(ctx: &FinderCtx, view: &SlotView, terms: &[PackedTerm]) -> Scope {
    Scope {
        spans: view.spans(),
        labels: terms
            .iter()
            .filter_map(|term| ctx.record_label(term.base?).map(|(label, _)| label))
            .collect(),
        indexed: BTreeSet::new(),
    }
}

/// The card flag list of a packed field (union of its `flags_from` fields,
/// in order), or the [`Namer::Probes`] table's flags.
fn field_flags(signal: &str, packed: &PackedField) -> Option<Vec<String>> {
    if packed.flags_from.is_empty() {
        return match packed.namer {
            Namer::Probes(table) => Some(table.iter().map(|(flag, _)| flag.to_string()).collect()),
            Namer::Card { .. } | Namer::Loop => None,
        };
    }
    let mut out: Vec<String> = Vec::new();
    for from in packed.flags_from {
        for flag in card_flags(signal, from)? {
            if !out.contains(&flag) {
                out.push(flag);
            }
        }
    }
    Some(out)
}

/// For a prefixed card list (`window.x`, `document.x`): keep the candidate
/// whose prefix fits every member the same receiver is read for.
fn by_receiver(
    ctx: &FinderCtx,
    base: NodeId,
    names: &[String],
    flags: &[String],
    receiver_members: &BTreeMap<(SymbolId, Option<NodeId>), BTreeSet<String>>,
) -> Vec<String> {
    let Some((receiver, _)) = ctx.receiver_member(base) else {
        return names.to_vec();
    };
    let Some(members) = receiver_members.get(&receiver) else {
        return names.to_vec();
    };
    let flags: BTreeSet<&str> = flags.iter().map(String::as_str).collect();
    names
        .iter()
        .filter(|name| {
            let prefix = name.split_once('.').map_or("", |(prefix, _)| prefix);
            members.iter().all(|member| flags.contains(format!("{prefix}.{member}").as_str()))
        })
        .cloned()
        .collect()
}

fn snippet(ctx: &FinderCtx, span: Span) -> String {
    let text = ctx
        .parse
        .source
        .get(span.start as usize..span.end as usize)
        .unwrap_or("");
    let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > 160 {
        format!("{}…", text.chars().take(160).collect::<String>())
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every packed field's flag list exists on its card.
    #[test]
    fn packed_fields_have_card_flags() {
        for packed in PACKED {
            match packed.flags_from {
                [_, ..] => {
                    for from in packed.flags_from {
                        let flags = card_flags(packed.signal, from)
                            .unwrap_or_else(|| panic!("{}.{from}: card has no flags list", packed.signal));
                        assert!(!flags.is_empty(), "{}.{from}: empty flags", packed.signal);
                    }
                    if let Namer::Probes(table) = packed.namer {
                        let flags = field_flags(packed.signal, packed).unwrap_or_default();
                        for (flag, _) in table {
                            assert!(flags.iter().any(|f| f == flag), "{}: {flag} not on card", packed.signal);
                        }
                    }
                }
                [] => {
                    let Namer::Probes(table) = packed.namer else {
                        if let Namer::Loop = packed.namer {
                            for field in packed.fields {
                                assert!(
                                    card_flags(packed.signal, field).is_some_and(|flags| !flags.is_empty()),
                                    "{}.{field}: card has no flags list",
                                    packed.signal
                                );
                            }
                            continue;
                        }
                        panic!("{}: a card namer needs flags_from", packed.signal);
                    };
                    for field in packed.fields {
                        let flags = card_flags(packed.signal, field)
                            .unwrap_or_else(|| panic!("{}.{field}: card has no flags list", packed.signal));
                        for (flag, _) in table {
                            assert!(flags.iter().any(|f| f == flag), "{}.{field}: {flag} not on card", packed.signal);
                        }
                    }
                }
            }
        }
    }
}
