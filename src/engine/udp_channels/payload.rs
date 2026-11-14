use super::schema::{SchemaKind, error, width};
use super::{ChannelError, MAX_DATAGRAM, Schema};
use meow_meow_script::{TransportValue, TypeTag};

impl Schema {
    /// Native numeric hot path: no heap allocation, type/name tags or decimal conversion.
    /// Output is modified only after size and value validation succeeds.
    pub fn encode_numeric(&self, values: &[f32], out: &mut [u8]) -> Result<usize, ChannelError> {
        let count = self
            .numeric_len()
            .ok_or_else(|| error("schema contains nonnumeric fields"))?;
        if values.len() != count || out.len() < count * 4 || count * 4 > MAX_DATAGRAM {
            return Err(error(
                "numeric payload or output length does not match schema",
            ));
        }
        if values.iter().any(|v| !v.is_finite()) {
            return Err(error("nonfinite f32"));
        }
        for (value, bytes) in values.iter().zip(out.chunks_exact_mut(4)) {
            bytes.copy_from_slice(&value.to_le_bytes());
        }
        Ok(count * 4)
    }

    /// Decode directly into caller-owned f32 storage, preserving finite f32 bit patterns.
    pub fn decode_numeric(&self, bytes: &[u8], out: &mut [f32]) -> Result<(), ChannelError> {
        let count = self
            .numeric_len()
            .ok_or_else(|| error("schema contains nonnumeric fields"))?;
        if bytes.len() != count * 4 || out.len() != count {
            return Err(error(
                "numeric payload or output length does not match schema",
            ));
        }
        self.validate_payload(bytes)?;
        for (value, bytes) in out.iter_mut().zip(bytes.chunks_exact(4)) {
            *value = f32::from_le_bytes(bytes.try_into().unwrap());
        }
        Ok(())
    }

    /// MMS boundary path. Record field names are resolved here, never sent per sample.
    /// Numeric f64 values are rounded to f32; overflow/nonfinite values are rejected.
    pub fn encode_value(
        &self,
        value: &TransportValue,
        out: &mut [u8],
    ) -> Result<usize, ChannelError> {
        // Stage in bounded stack storage so errors cannot leave a partial packet in out.
        let mut scratch = [0u8; MAX_DATAGRAM];
        let mut pos = 0;
        match &self.kind {
            SchemaKind::Primitive(tag) => encode_field(*tag, value, &mut scratch, &mut pos)?,
            SchemaKind::Record(record) => {
                let TransportValue::Table(fields) = value else {
                    return Err(error("expected record value"));
                };
                if fields.len() != record.0.len() {
                    return Err(error("record fields do not match schema"));
                }
                for (name, tag) in &record.0 {
                    let mut matches = fields.iter().filter(|(key, _)| key == name);
                    let (_, field) = matches
                        .next()
                        .ok_or_else(|| error(format!("missing field {name}")))?;
                    if matches.next().is_some() {
                        return Err(error("duplicate record field"));
                    }
                    encode_field(*tag, field, &mut scratch, &mut pos)?;
                }
            }
        }
        if out.len() < pos {
            return Err(error("output buffer too small"));
        }
        out[..pos].copy_from_slice(&scratch[..pos]);
        Ok(pos)
    }

    /// Owned values are materialized only for consumers requiring MMS transport values.
    pub fn decode_value(&self, bytes: &[u8]) -> Result<TransportValue, ChannelError> {
        self.validate_payload(bytes)?;
        let mut pos = 0;
        Ok(match &self.kind {
            SchemaKind::Primitive(tag) => decode_field(*tag, bytes, &mut pos),
            SchemaKind::Record(record) => TransportValue::Table(
                record
                    .0
                    .iter()
                    .map(|(name, tag)| (name.clone(), decode_field(*tag, bytes, &mut pos)))
                    .collect(),
            ),
        })
    }

    pub(super) fn validate_payload(&self, bytes: &[u8]) -> Result<(), ChannelError> {
        if bytes.len() > MAX_DATAGRAM {
            return Err(error("payload exceeds datagram budget"));
        }
        let mut pos = 0;
        match &self.kind {
            SchemaKind::Primitive(tag) => validate_field(*tag, bytes, &mut pos)?,
            SchemaKind::Record(record) => {
                for tag in record.0.values() {
                    validate_field(*tag, bytes, &mut pos)?;
                }
            }
        }
        if pos != bytes.len() {
            return Err(error("trailing payload bytes"));
        }
        Ok(())
    }
}

fn encode_field(
    tag: TypeTag,
    value: &TransportValue,
    out: &mut [u8],
    pos: &mut usize,
) -> Result<(), ChannelError> {
    if tag == TypeTag::Str {
        let TransportValue::String(value) = value else {
            return Err(error("expected string"));
        };
        let len = u16::try_from(value.len()).map_err(|_| error("string too long"))?;
        put(out, pos, &len.to_le_bytes())?;
        return put(out, pos, value.as_bytes());
    }
    let mut number = |value: &TransportValue| -> Result<(), ChannelError> {
        let TransportValue::Number(value) = value else {
            return Err(error("expected number"));
        };
        let value = *value as f32;
        if !value.is_finite() {
            return Err(error("number outside finite f32 range"));
        }
        put(out, pos, &value.to_le_bytes())
    };
    if tag == TypeTag::F32 {
        return number(value);
    }
    let TransportValue::Array(values) = value else {
        return Err(error("expected vector array"));
    };
    if values.len() != width(tag).unwrap() {
        return Err(error("vector length does not match type"));
    }
    for value in values {
        number(value)?;
    }
    Ok(())
}

fn validate_field(tag: TypeTag, bytes: &[u8], pos: &mut usize) -> Result<(), ChannelError> {
    if tag == TypeTag::Str {
        let len = u16::from_le_bytes(take(bytes, pos, 2)?.try_into().unwrap()) as usize;
        std::str::from_utf8(take(bytes, pos, len)?).map_err(|_| error("invalid UTF-8"))?;
    } else {
        for _ in 0..width(tag).unwrap() {
            let v = f32::from_le_bytes(take(bytes, pos, 4)?.try_into().unwrap());
            if !v.is_finite() {
                return Err(error("nonfinite f32"));
            }
        }
    }
    Ok(())
}

// Called only after complete validation.
fn decode_field(tag: TypeTag, bytes: &[u8], pos: &mut usize) -> TransportValue {
    if tag == TypeTag::Str {
        let len = u16::from_le_bytes(take(bytes, pos, 2).unwrap().try_into().unwrap()) as usize;
        return TransportValue::String(
            std::str::from_utf8(take(bytes, pos, len).unwrap())
                .unwrap()
                .into(),
        );
    }
    let mut number = || {
        TransportValue::Number(
            f32::from_le_bytes(take(bytes, pos, 4).unwrap().try_into().unwrap()) as f64,
        )
    };
    if tag == TypeTag::F32 {
        number()
    } else {
        TransportValue::Array((0..width(tag).unwrap()).map(|_| number()).collect())
    }
}

pub(super) fn put(out: &mut [u8], pos: &mut usize, bytes: &[u8]) -> Result<(), ChannelError> {
    let end = pos
        .checked_add(bytes.len())
        .ok_or_else(|| error("length overflow"))?;
    out.get_mut(*pos..end)
        .ok_or_else(|| error("datagram/output buffer too small"))?
        .copy_from_slice(bytes);
    *pos = end;
    Ok(())
}
pub(super) fn take<'a>(
    bytes: &'a [u8],
    pos: &mut usize,
    len: usize,
) -> Result<&'a [u8], ChannelError> {
    let end = pos
        .checked_add(len)
        .ok_or_else(|| error("length overflow"))?;
    let value = bytes
        .get(*pos..end)
        .ok_or_else(|| error("truncated packet"))?;
    *pos = end;
    Ok(value)
}
