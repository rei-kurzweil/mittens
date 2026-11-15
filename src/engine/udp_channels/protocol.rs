//! Version 1: explicit little-endian fields, never Rust discriminants/layout.
//! Header: magic[4], version:u8, kind:u8, publisher_epoch[16], channel:u64,
//! sequence:u64. Schema/name appear only in declarations.
use super::payload::{put, take};
use super::schema::{MAX_FIELDS, SchemaKind, error, validate_name};
use super::{ChannelError, MAX_DATAGRAM, Schema};
use meow_meow_script::{RecordType, TypeTag};
use std::collections::BTreeMap;

pub(super) const HEADER: usize = 38;
pub(super) const DECLARE: u8 = 1;
pub(super) const ACK: u8 = 2;
pub(super) const SAMPLE: u8 = 3;
pub(super) const WITHDRAW: u8 = 4;
pub(super) const REJECT: u8 = 5;

pub(super) struct Packet<'a> {
    pub kind: u8,
    pub epoch: [u8; 16],
    pub channel: u64,
    pub sequence: u64,
    pub payload: &'a [u8],
}

pub(super) fn header(
    out: &mut [u8],
    kind: u8,
    epoch: [u8; 16],
    channel: u64,
    sequence: u64,
) -> Result<(), ChannelError> {
    if out.len() < HEADER
        || channel == 0
        || !matches!(kind, DECLARE | ACK | SAMPLE | WITHDRAW | REJECT)
    {
        return Err(error("invalid channel header"));
    }
    out[..4].copy_from_slice(b"MCHN");
    out[4] = 1;
    out[5] = kind;
    out[6..22].copy_from_slice(&epoch);
    out[22..30].copy_from_slice(&channel.to_le_bytes());
    out[30..38].copy_from_slice(&sequence.to_le_bytes());
    Ok(())
}

pub(super) fn decode(bytes: &[u8]) -> Result<Packet<'_>, ChannelError> {
    if bytes.len() < HEADER || bytes.len() > MAX_DATAGRAM || &bytes[..4] != b"MCHN" || bytes[4] != 1
    {
        return Err(error("invalid channel packet magic, version or length"));
    }
    let kind = bytes[5];
    let sequence = u64::from_le_bytes(bytes[30..38].try_into().unwrap());
    let channel = u64::from_le_bytes(bytes[22..30].try_into().unwrap());
    if channel == 0
        || !matches!(kind, DECLARE | ACK | SAMPLE | WITHDRAW | REJECT)
        || (kind != SAMPLE && sequence != 0)
        || (kind == SAMPLE && sequence == 0)
        || (matches!(kind, ACK | WITHDRAW | REJECT) && bytes.len() != HEADER)
    {
        return Err(error("invalid channel packet kind, identity or sequence"));
    }
    Ok(Packet {
        kind,
        epoch: bytes[6..22].try_into().unwrap(),
        channel,
        sequence,
        payload: &bytes[HEADER..],
    })
}

pub(super) fn declaration(
    name: &str,
    schema: &Schema,
    out: &mut [u8],
) -> Result<usize, ChannelError> {
    validate_name(name)?;
    let mut pos = HEADER;
    text(out, &mut pos, name)?;
    match &schema.kind {
        SchemaKind::Primitive(tag) => put(out, &mut pos, &[0, tag_byte(*tag)])?,
        SchemaKind::Record(record) => {
            put(out, &mut pos, &[1, record.0.len() as u8])?;
            for (name, tag) in &record.0 {
                text(out, &mut pos, name)?;
                put(out, &mut pos, &[tag_byte(*tag)])?;
            }
        }
    }
    Ok(pos)
}

pub(super) fn read_declaration(bytes: &[u8]) -> Result<(String, Schema), ChannelError> {
    let mut pos = 0;
    let name = read_text(bytes, &mut pos)?.to_owned();
    let kind = take(bytes, &mut pos, 1)?[0];
    let schema = match kind {
        0 => Schema::primitive(read_tag(take(bytes, &mut pos, 1)?[0])?),
        1 => {
            let count = take(bytes, &mut pos, 1)?[0] as usize;
            if count == 0 || count > MAX_FIELDS {
                return Err(error("invalid field count"));
            }
            let mut fields = BTreeMap::new();
            for _ in 0..count {
                let field = read_text(bytes, &mut pos)?.to_owned();
                // Accept only canonical order; duplicate names are invalid.
                if fields
                    .last_key_value()
                    .is_some_and(|(previous, _)| previous >= &field)
                {
                    return Err(error("noncanonical or duplicate record field"));
                }
                fields.insert(field, read_tag(take(bytes, &mut pos, 1)?[0])?);
            }
            Schema::record(RecordType(fields))?
        }
        _ => return Err(error("unknown schema kind")),
    };
    if pos != bytes.len() {
        return Err(error("trailing declaration bytes"));
    }
    Ok((name, schema))
}
fn text(out: &mut [u8], pos: &mut usize, value: &str) -> Result<(), ChannelError> {
    validate_name(value)?;
    put(out, pos, &[value.len() as u8])?;
    put(out, pos, value.as_bytes())
}
fn read_text<'a>(bytes: &'a [u8], pos: &mut usize) -> Result<&'a str, ChannelError> {
    let len = take(bytes, pos, 1)?[0] as usize;
    let text =
        std::str::from_utf8(take(bytes, pos, len)?).map_err(|_| error("invalid name UTF-8"))?;
    validate_name(text)?;
    Ok(text)
}
fn tag_byte(tag: TypeTag) -> u8 {
    match tag {
        TypeTag::F32 => 1,
        TypeTag::Vec2 => 2,
        TypeTag::Vec3 => 3,
        TypeTag::Vec4 => 4,
        TypeTag::Str => 5,
    }
}
fn read_tag(tag: u8) -> Result<TypeTag, ChannelError> {
    match tag {
        1 => Ok(TypeTag::F32),
        2 => Ok(TypeTag::Vec2),
        3 => Ok(TypeTag::Vec3),
        4 => Ok(TypeTag::Vec4),
        5 => Ok(TypeTag::Str),
        _ => Err(error("unknown wire type")),
    }
}
