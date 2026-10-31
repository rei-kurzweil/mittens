//! JSON conversion at the MMS host boundary.
use super::runtime_config::{MittensApi, MittensBinding};
use meow_meow_script as mms;

pub(super) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    builder.namespace("JSON", |namespace| {
        for (name, api, result) in [
            ("parse", MittensApi::JsonParse, mms::ValueType::Any),
            (
                "try_parse_object",
                MittensApi::JsonTryParseObject,
                mms::ValueType::Any,
            ),
            (
                "stringify",
                MittensApi::JsonStringify,
                mms::ValueType::String,
            ),
        ] {
            let argument = if name == "stringify" {
                mms::ValueType::Any
            } else {
                mms::ValueType::String
            };
            namespace.api(
                name,
                mms::ValueSignature::new(vec![argument], result),
                MittensBinding::Api(api),
            );
        }
    });
}

/// Network handlers can reject malformed JSON and non-object roots without
/// aborting their callback before replying. Valid objects retain parse semantics.
pub(super) fn try_parse_object(
    args: Vec<mms::TransportValue>,
) -> Result<mms::HostResponse, mms::HostError> {
    let [mms::TransportValue::String(text)] = args.as_slice() else {
        return Err(mms::HostError::failure(
            "JSON.try_parse_object",
            "expected one string argument",
        ));
    };
    let value = match serde_json::from_str(text) {
        Ok(value @ serde_json::Value::Object(_)) => json_to_transport(value)?,
        _ => mms::TransportValue::Null,
    };
    Ok(mms::HostResponse::Transport(value))
}

pub(super) fn parse(args: Vec<mms::TransportValue>) -> Result<mms::HostResponse, mms::HostError> {
    let [mms::TransportValue::String(text)] = args.as_slice() else {
        return Err(mms::HostError::failure(
            "JSON.parse",
            "expected one string argument",
        ));
    };
    let value = serde_json::from_str(text)
        .map_err(|error| mms::HostError::failure("JSON.parse", error.to_string()))?;
    Ok(mms::HostResponse::Transport(json_to_transport(value)?))
}

pub(super) fn stringify(
    args: Vec<mms::TransportValue>,
) -> Result<mms::HostResponse, mms::HostError> {
    let [value] = args.as_slice() else {
        return Err(mms::HostError::failure(
            "JSON.stringify",
            "expected one argument",
        ));
    };
    let text = serde_json::to_string(&transport_to_json(value)?)
        .map_err(|error| mms::HostError::failure("JSON.stringify", error.to_string()))?;
    Ok(mms::HostResponse::Transport(mms::TransportValue::String(
        text,
    )))
}

fn json_to_transport(value: serde_json::Value) -> Result<mms::TransportValue, mms::HostError> {
    Ok(match value {
        serde_json::Value::Null => mms::TransportValue::Null,
        serde_json::Value::Bool(value) => mms::TransportValue::Bool(value),
        serde_json::Value::Number(value) => {
            mms::TransportValue::Number(value.as_f64().ok_or_else(|| {
                mms::HostError::failure("JSON.parse", "number cannot be represented by MMS")
            })?)
        }
        serde_json::Value::String(value) => mms::TransportValue::String(value),
        serde_json::Value::Array(values) => mms::TransportValue::Array(
            values
                .into_iter()
                .map(json_to_transport)
                .collect::<Result<_, _>>()?,
        ),
        serde_json::Value::Object(values) => mms::TransportValue::Table(
            values
                .into_iter()
                .map(|(key, value)| Ok((key, json_to_transport(value)?)))
                .collect::<Result<_, mms::HostError>>()?,
        ),
    })
}

fn transport_to_json(value: &mms::TransportValue) -> Result<serde_json::Value, mms::HostError> {
    Ok(match value {
        mms::TransportValue::Null => serde_json::Value::Null,
        mms::TransportValue::Bool(value) => serde_json::Value::Bool(*value),
        mms::TransportValue::Number(value) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| mms::HostError::failure("JSON.stringify", "numbers must be finite"))?,
        mms::TransportValue::String(value) => serde_json::Value::String(value.clone()),
        mms::TransportValue::TypeTag(_) | mms::TransportValue::RecordType(_) => {
            return Err(mms::HostError::failure(
                "JSON.stringify",
                "type tags are not JSON values",
            ));
        }
        mms::TransportValue::Array(values) => serde_json::Value::Array(
            values
                .iter()
                .map(transport_to_json)
                .collect::<Result<_, _>>()?,
        ),
        mms::TransportValue::Table(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| Ok((key.clone(), transport_to_json(value)?)))
                .collect::<Result<_, mms::HostError>>()?,
        ),
        mms::TransportValue::Component(_) | mms::TransportValue::TypedComponent { .. } => {
            return Err(mms::HostError::failure(
                "JSON.stringify",
                "component handles are not JSON values",
            ));
        }
        mms::TransportValue::Callback(_) => {
            return Err(mms::HostError::failure(
                "JSON.stringify",
                "functions are not JSON values",
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_packets_and_non_object_roots_can_be_rejected_without_aborting_callbacks() {
        for input in ["{", "[]", "null", "42", "\"text\""] {
            assert!(matches!(
                try_parse_object(vec![mms::TransportValue::String(input.into())]).unwrap(),
                mms::HostResponse::Transport(mms::TransportValue::Null)
            ));
        }
        assert!(matches!(
            try_parse_object(vec![mms::TransportValue::String("{\"version\":1}".into())]).unwrap(),
            mms::HostResponse::Transport(mms::TransportValue::Table(_))
        ));
    }
}
