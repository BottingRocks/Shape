//! The approved field card of a signal: what it collects and why, and its
//! fields with their wire types, repeats and packed flag names.
//!
//! Each catalog file holds its signal's card as `pub(crate) const CARD`
//! (registered in [`super::SIGNALS`]). The schema names fields only from the
//! card, the flag decoders name bits only from a card field's `flags`, and
//! the pages under `docs/signals/` are rendered from it
//! (`shape signals docs <dir>`, [`crate::signals::docs`]).

/// One signal's card.
#[derive(Debug)]
pub struct Card {
    /// The canonical signal name.
    pub signal: &'static str,
    /// What the signal collects.
    pub summary: &'static str,
    /// Why a bot detector collects it.
    pub why: &'static str,
    /// Captured decoded JSON example, owned by this card; never a runtime schema.
    /// None means uncaptured or no payload; it does not mean emitted JSON null.
    pub sample_payload: Option<&'static str>,
    /// Sent by name through the generic encoder (one named object or bare
    /// value) instead of field by field with a positional index.
    pub by_name: bool,
    pub body: CardBody,
}

/// The fields of a card: one field list, or one per message shape.
#[derive(Debug)]
pub enum CardBody {
    Fields(&'static [CardField]),
    Messages(&'static [CardMessage]),
}

/// One message shape of a signal that sends several.
#[derive(Debug)]
pub struct CardMessage {
    /// When this message is sent.
    pub when: &'static str,
    pub fields: &'static [CardField],
}

/// One card field.
#[derive(Debug)]
pub struct CardField {
    pub name: &'static str,
    pub kind: CardFieldKind,
    /// What the value means (empty for a group).
    pub meaning: &'static str,
    /// What one repetition is (`"each font"`), when the field repeats.
    pub repeat: Option<&'static str>,
    /// The flag names of a packed flags byte, in card order (the bit of
    /// each flag is per build). Empty for other fields.
    pub flags: &'static [&'static str],
    /// This byte encodes a logical boolean using source-derived tags.
    pub boolean: bool,
    /// Literal byte alternatives in this scoped field are documented null markers.
    /// Number and string payloads retain their values, including numeric sentinels.
    pub null_marker: bool,
    /// Scoped components of a scalar's documented multi-byte null alternative.
    pub null_components: &'static [CardField],
}

/// The wire type of a card field.
#[derive(Debug)]
pub enum CardFieldKind {
    Number,
    Byte,
    String,
    /// A record of nested fields (usually repeated).
    Group(&'static [CardField]),
}

const NULL_COMPONENTS: &[CardField] = &[
    CardField::null("nullMarker", "First component of this field's null marker."),
    CardField::null("nullFlag", "Second component of this field's null marker."),
];

impl CardField {
    const fn leaf(name: &'static str, kind: CardFieldKind, meaning: &'static str) -> CardField {
        CardField { name, kind, meaning, repeat: None, flags: &[], boolean: false, null_marker: false, null_components: &[] }
    }

    pub const fn number(name: &'static str, meaning: &'static str) -> CardField {
        CardField::leaf(name, CardFieldKind::Number, meaning)
    }

    pub const fn byte(name: &'static str, meaning: &'static str) -> CardField {
        CardField::leaf(name, CardFieldKind::Byte, meaning)
    }

    pub const fn boolean(name: &'static str, meaning: &'static str) -> CardField {
        CardField { boolean: true, ..Self::byte(name, meaning) }
    }

    pub const fn null(name: &'static str, meaning: &'static str) -> CardField {
        CardField { null_marker: true, ..Self::byte(name, meaning) }
    }

    /// Certify the null-byte alternatives of a number or string field. Do not
    /// use for fields that also encode non-null tags (e.g. "unchanged").
    pub const fn nullable(self) -> CardField {
        CardField { null_marker: true, ..self }
    }

    /// This scalar's missing alternative may be a two-byte marker record.
    /// Both named components are encoding-only; each retains its own inverse.
    pub const fn nullable_pair(self) -> CardField {
        CardField {
            null_marker: true,
            null_components: NULL_COMPONENTS,
            ..self
        }
    }

    pub const fn string(name: &'static str, meaning: &'static str) -> CardField {
        CardField::leaf(name, CardFieldKind::String, meaning)
    }

    pub const fn group(name: &'static str, fields: &'static [CardField]) -> CardField {
        CardField::leaf(name, CardFieldKind::Group(fields), "")
    }

    pub const fn repeat(self, each: &'static str) -> CardField {
        CardField { repeat: Some(each), ..self }
    }

    pub const fn flags(self, flags: &'static [&'static str]) -> CardField {
        CardField { flags, ..self }
    }

    /// Nested group fields or scoped components of a scalar null alternative.
    pub fn fields(&self) -> &'static [CardField] {
        match self.kind {
            CardFieldKind::Group(fields) => fields,
            _ => self.null_components,
        }
    }
}

impl Card {
    /// A signal with no data: a declared-not-emitted slot (it never collects
    /// a value). Its schema entry is `type: "no_data"` with `fields: {}` and
    /// the page-visible `effect`; its card has an empty field list.
    pub fn no_data(&self) -> bool {
        matches!(self.body, CardBody::Fields(fields) if fields.is_empty())
    }

    /// The messages of the card; a single-list card is one message without
    /// a `when`.
    pub fn messages(&self) -> Vec<(Option<&'static str>, &'static [CardField])> {
        match self.body {
            CardBody::Fields(fields) => vec![(None, fields)],
            CardBody::Messages(messages) => messages.iter().map(|m| (Some(m.when), m.fields)).collect(),
        }
    }

    /// Every field of the card in document order: messages in order, each
    /// field before its nested fields.
    pub fn all_fields(&self) -> Vec<&'static CardField> {
        fn walk(fields: &'static [CardField], out: &mut Vec<&'static CardField>) {
            for field in fields {
                out.push(field);
                walk(field.fields(), out);
            }
        }
        let mut out = Vec::new();
        for (_, fields) in self.messages() {
            walk(fields, &mut out);
        }
        out
    }

    /// The `flags` list of the first field named `field` that has one.
    pub fn flags_of(&self, field: &str) -> Option<&'static [&'static str]> {
        self.all_fields()
            .into_iter()
            .find(|f| f.name == field && !f.flags.is_empty())
            .map(|f| f.flags)
    }
}
