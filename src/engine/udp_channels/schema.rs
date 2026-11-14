use meow_meow_script::{RecordType, TransportValue, TypeTag};
use std::fmt;

/// Closed wire schema using the language's existing type identities.
/// Record order is canonical (lexicographic), not source declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub(super) kind: SchemaKind,
    numeric_len: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SchemaKind {
    Primitive(TypeTag),
    Record(RecordType),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelError(pub String);

impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for ChannelError {}
impl From<std::io::Error> for ChannelError {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}

pub(super) fn error(message: impl Into<String>) -> ChannelError {
    ChannelError(message.into())
}
pub(super) const MAX_FIELDS: usize = 32;
pub(super) const MAX_NAME: usize = 96;

impl Schema {
    pub fn primitive(tag: TypeTag) -> Self {
        Self {
            kind: SchemaKind::Primitive(tag),
            numeric_len: width(tag),
        }
    }

    pub fn record(record: RecordType) -> Result<Self, ChannelError> {
        if record.0.is_empty() || record.0.len() > MAX_FIELDS {
            return Err(error("record must contain 1..=32 fields"));
        }
        for name in record.0.keys() {
            validate_name(name)?;
        }
        let numeric_len = record
            .0
            .values()
            .try_fold(0, |n, tag| width(*tag).map(|w| n + w));
        Ok(Self {
            kind: SchemaKind::Record(record),
            numeric_len,
        })
    }

    /// Accept exactly the type-tag / record-type values transported by MMS.
    /// An ordinary table containing tags is not a record type.
    pub fn from_type(value: &TransportValue) -> Result<Self, ChannelError> {
        match value {
            TransportValue::TypeTag(tag) => Ok(Self::primitive(*tag)),
            TransportValue::RecordType(record) => Self::record(record.clone()),
            _ => Err(error(
                "channel type must be an imported type tag or a record type",
            )),
        }
    }

    /// Number of f32 lanes in numeric schemas; None for schemas containing strings.
    pub fn numeric_len(&self) -> Option<usize> {
        self.numeric_len
    }

    /// Fixed payload size, excluding the channel packet header.
    pub fn fixed_size(&self) -> Option<usize> {
        self.numeric_len.map(|n| n * 4)
    }
}

pub(super) fn validate_name(name: &str) -> Result<(), ChannelError> {
    if name.is_empty() || name.len() > MAX_NAME || name.contains('\0') {
        return Err(error("names must contain 1..=96 UTF-8 bytes and no NUL"));
    }
    Ok(())
}

pub(super) fn width(tag: TypeTag) -> Option<usize> {
    match tag {
        TypeTag::F32 => Some(1),
        TypeTag::Vec2 => Some(2),
        TypeTag::Vec3 => Some(3),
        TypeTag::Vec4 => Some(4),
        TypeTag::Str => None,
    }
}
