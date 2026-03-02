use super::Component;
use crate::engine::ecs::{ComponentId, SignalValue};
use slotmap::{Key, KeyData};

#[derive(Debug, Clone)]
pub struct ActionComponent {
    pub signal: SignalValue,
}

impl ActionComponent {
    pub fn new(signal: SignalValue) -> Self {
        Self { signal }
    }

    pub fn print(message: impl Into<String>) -> Self {
        Self::new(SignalValue::Print {
            message: message.into(),
        })
    }
}

impl Default for ActionComponent {
    fn default() -> Self {
        Self {
            signal: SignalValue::Noop,
        }
    }
}

impl Component for ActionComponent {
    fn name(&self) -> &'static str {
        "action"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();

        match &self.signal {
            SignalValue::Noop => {
                map.insert("variant".to_string(), serde_json::json!("Noop"));
            }
            SignalValue::Print { message } => {
                map.insert("variant".to_string(), serde_json::json!("Print"));
                map.insert("message".to_string(), serde_json::json!(message));
            }

            SignalValue::SetColor { target, rgba } => {
                map.insert("variant".to_string(), serde_json::json!("SetColor"));
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("rgba".to_string(), serde_json::json!(rgba));
            }
            SignalValue::SetText { target, text } => {
                map.insert("variant".to_string(), serde_json::json!("SetText"));
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("text".to_string(), serde_json::json!(text));
            }
            SignalValue::SetPosition { target, position } => {
                map.insert("variant".to_string(), serde_json::json!("SetPosition"));
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("position".to_string(), serde_json::json!(position));
            }
            SignalValue::SetTransform {
                target,
                translation,
                rotation_quat_xyzw,
                scale,
            } => {
                map.insert("variant".to_string(), serde_json::json!("SetTransform"));
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("translation".to_string(), serde_json::json!(translation));
                map.insert(
                    "rotation_quat_xyzw".to_string(),
                    serde_json::json!(rotation_quat_xyzw),
                );
                map.insert("scale".to_string(), serde_json::json!(scale));
            }

            SignalValue::Attach { parents, child } => {
                map.insert("variant".to_string(), serde_json::json!("Attach"));
                map.insert("parents".to_string(), serde_json::json!(encode_ids(parents)));
                map.insert("child".to_string(), serde_json::json!(encode_id(*child)));
            }
            SignalValue::AttachClone {
                parents,
                prefab_root,
            } => {
                map.insert("variant".to_string(), serde_json::json!("AttachClone"));
                map.insert("parents".to_string(), serde_json::json!(encode_ids(parents)));
                map.insert(
                    "prefab_root".to_string(),
                    serde_json::json!(encode_id(*prefab_root)),
                );
            }
            SignalValue::Detach { target } => {
                map.insert("variant".to_string(), serde_json::json!("Detach"));
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
            }
            SignalValue::RemoveChild { parents, index } => {
                map.insert("variant".to_string(), serde_json::json!("RemoveChild"));
                map.insert("parents".to_string(), serde_json::json!(encode_ids(parents)));
                map.insert("index".to_string(), serde_json::json!(index));
            }
            SignalValue::RemoveChildren { parents } => {
                map.insert("variant".to_string(), serde_json::json!("RemoveChildren"));
                map.insert("parents".to_string(), serde_json::json!(encode_ids(parents)));
            }
            SignalValue::RemoveSubtree { target } => {
                map.insert("variant".to_string(), serde_json::json!("RemoveSubtree"));
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
            }

            SignalValue::AudioGraphRebuild { target } => {
                map.insert("variant".to_string(), serde_json::json!("AudioGraphRebuild"));
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
            }
            SignalValue::RequestRaycast { target } => {
                map.insert("variant".to_string(), serde_json::json!("RequestRaycast"));
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
            }
            SignalValue::AudioLowPassSetCutoffHz { target, cutoff_hz } => {
                map.insert(
                    "variant".to_string(),
                    serde_json::json!("AudioLowPassSetCutoffHz"),
                );
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("cutoff_hz".to_string(), serde_json::json!(cutoff_hz));
            }
            SignalValue::AudioBandPassSetCenterHz { target, center_hz } => {
                map.insert(
                    "variant".to_string(),
                    serde_json::json!("AudioBandPassSetCenterHz"),
                );
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("center_hz".to_string(), serde_json::json!(center_hz));
            }
            SignalValue::OscillatorSetEnabled { target, enabled } => {
                map.insert("variant".to_string(), serde_json::json!("OscillatorSetEnabled"));
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("enabled".to_string(), serde_json::json!(enabled));
            }
            SignalValue::OscillatorSetPitch {
                target,
                frequency_hz,
            } => {
                map.insert("variant".to_string(), serde_json::json!("OscillatorSetPitch"));
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("frequency_hz".to_string(), serde_json::json!(frequency_hz));
            }
            SignalValue::OscillatorScheduleSetPitch {
                target,
                beat_offset,
                beat_context,
                frequency_hz,
            } => {
                map.insert(
                    "variant".to_string(),
                    serde_json::json!("OscillatorScheduleSetPitch"),
                );
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("beat_offset".to_string(), serde_json::json!(beat_offset));
                map.insert("beat_context".to_string(), serde_json::json!(beat_context));
                map.insert("frequency_hz".to_string(), serde_json::json!(frequency_hz));
            }
            SignalValue::OscillatorScheduleSetNote {
                target,
                beat_offset,
                beat_context,
                pitch,
                octave,
                duration_beats,
            } => {
                map.insert(
                    "variant".to_string(),
                    serde_json::json!("OscillatorScheduleSetNote"),
                );
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("beat_offset".to_string(), serde_json::json!(beat_offset));
                map.insert("beat_context".to_string(), serde_json::json!(beat_context));
                map.insert("pitch".to_string(), serde_json::json!(pitch));
                map.insert("octave".to_string(), serde_json::json!(octave));
                map.insert("duration_beats".to_string(), serde_json::json!(duration_beats));
            }
            SignalValue::OscillatorScheduleMusicNote {
                target,
                beat_offset,
                beat_context,
                note,
            } => {
                map.insert(
                    "variant".to_string(),
                    serde_json::json!("OscillatorScheduleMusicNote"),
                );
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("beat_offset".to_string(), serde_json::json!(beat_offset));
                map.insert("beat_context".to_string(), serde_json::json!(beat_context));
                map.insert("note".to_string(), serde_json::json!(note));
            }
            SignalValue::MusicSetNote { target, note } => {
                map.insert("variant".to_string(), serde_json::json!("MusicSetNote"));
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("note".to_string(), serde_json::json!(note));
            }
            SignalValue::CommandQueue {
                target,
                command_name,
                params,
            } => {
                map.insert("variant".to_string(), serde_json::json!("CommandQueue"));
                map.insert("target".to_string(), serde_json::json!(encode_ids(target)));
                map.insert("command_name".to_string(), serde_json::json!(command_name));
                map.insert("params".to_string(), serde_json::json!(params));
            }

            // Non-action signals should not be persisted in ActionComponent.
            other => {
                map.insert("variant".to_string(), serde_json::json!("Noop"));
                map.insert(
                    "error".to_string(),
                    serde_json::json!(format!("Non-action signal in ActionComponent: {other:?}")),
                );
            }
        }

        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let variant = data
            .get("variant")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "ActionComponent missing 'variant'".to_string())?;

        self.signal = match variant {
            "Noop" => SignalValue::Noop,
            "Print" => SignalValue::Print {
                message: get_string(data, "message")?,
            },

            "SetColor" => SignalValue::SetColor {
                target: get_ids(data, "target")?,
                rgba: get_value_as(data, "rgba")?,
            },
            "SetText" => SignalValue::SetText {
                target: get_ids(data, "target")?,
                text: get_string(data, "text")?,
            },
            "SetPosition" => SignalValue::SetPosition {
                target: get_ids(data, "target")?,
                position: get_value_as(data, "position")?,
            },
            "SetTransform" => SignalValue::SetTransform {
                target: get_ids(data, "target")?,
                translation: get_value_as(data, "translation")?,
                rotation_quat_xyzw: get_value_as(data, "rotation_quat_xyzw")?,
                scale: get_value_as(data, "scale")?,
            },

            "Attach" => SignalValue::Attach {
                parents: get_ids(data, "parents")?,
                child: get_id(data, "child")?,
            },
            "AttachClone" => SignalValue::AttachClone {
                parents: get_ids(data, "parents")?,
                prefab_root: get_id(data, "prefab_root")?,
            },
            "Detach" => SignalValue::Detach {
                target: get_ids(data, "target")?,
            },
            "RemoveChild" => SignalValue::RemoveChild {
                parents: get_ids(data, "parents")?,
                index: get_value_as(data, "index")?,
            },
            "RemoveChildren" => SignalValue::RemoveChildren {
                parents: get_ids(data, "parents")?,
            },
            "RemoveSubtree" => SignalValue::RemoveSubtree {
                target: get_ids(data, "target")?,
            },
            "AudioGraphRebuild" => SignalValue::AudioGraphRebuild {
                target: get_ids(data, "target")?,
            },
            "RequestRaycast" => SignalValue::RequestRaycast {
                target: get_ids(data, "target")?,
            },
            "AudioLowPassSetCutoffHz" => SignalValue::AudioLowPassSetCutoffHz {
                target: get_ids(data, "target")?,
                cutoff_hz: get_value_as(data, "cutoff_hz")?,
            },
            "AudioBandPassSetCenterHz" => SignalValue::AudioBandPassSetCenterHz {
                target: get_ids(data, "target")?,
                center_hz: get_value_as(data, "center_hz")?,
            },
            "OscillatorSetEnabled" => SignalValue::OscillatorSetEnabled {
                target: get_ids(data, "target")?,
                enabled: get_value_as(data, "enabled")?,
            },
            "OscillatorSetPitch" => SignalValue::OscillatorSetPitch {
                target: get_ids(data, "target")?,
                frequency_hz: get_value_as(data, "frequency_hz")?,
            },
            "OscillatorScheduleSetPitch" => SignalValue::OscillatorScheduleSetPitch {
                target: get_ids(data, "target")?,
                beat_offset: get_value_as(data, "beat_offset")?,
                beat_context: get_value_as(data, "beat_context")?,
                frequency_hz: get_value_as(data, "frequency_hz")?,
            },
            "OscillatorScheduleSetNote" => SignalValue::OscillatorScheduleSetNote {
                target: get_ids(data, "target")?,
                beat_offset: get_value_as(data, "beat_offset")?,
                beat_context: get_value_as(data, "beat_context")?,
                pitch: get_value_as(data, "pitch")?,
                octave: get_value_as(data, "octave")?,
                duration_beats: get_value_as(data, "duration_beats")?,
            },
            "OscillatorScheduleMusicNote" => SignalValue::OscillatorScheduleMusicNote {
                target: get_ids(data, "target")?,
                beat_offset: get_value_as(data, "beat_offset")?,
                beat_context: get_value_as(data, "beat_context")?,
                note: get_value_as(data, "note")?,
            },
            "MusicSetNote" => SignalValue::MusicSetNote {
                target: get_ids(data, "target")?,
                note: get_value_as(data, "note")?,
            },
            "CommandQueue" => SignalValue::CommandQueue {
                target: get_ids(data, "target")?,
                command_name: get_string(data, "command_name")?,
                params: get_value_as(data, "params")?,
            },

            other => return Err(format!("Unknown action variant: {other}")),
        };

        Ok(())
    }
}

fn encode_id(id: ComponentId) -> u64 {
    id.data().as_ffi()
}

fn encode_ids(ids: &[ComponentId]) -> Vec<u64> {
    ids.iter().map(|id| encode_id(*id)).collect()
}

fn decode_id(v: &serde_json::Value) -> Result<ComponentId, String> {
    let ffi: u64 = serde_json::from_value(v.clone())
        .map_err(|e| format!("Failed to decode ComponentId (ffi u64): {e}"))?;
    Ok(KeyData::from_ffi(ffi).into())
}

fn decode_ids(v: &serde_json::Value) -> Result<Vec<ComponentId>, String> {
    let ffi: Vec<u64> = serde_json::from_value(v.clone())
        .map_err(|e| format!("Failed to decode ComponentId list (ffi u64[]): {e}"))?;
    Ok(ffi.into_iter().map(|x| KeyData::from_ffi(x).into()).collect())
}

fn get_string(
    data: &std::collections::HashMap<String, serde_json::Value>,
    key: &str,
) -> Result<String, String> {
    data.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("ActionComponent missing/invalid '{key}'"))
}

fn get_id(
    data: &std::collections::HashMap<String, serde_json::Value>,
    key: &str,
) -> Result<ComponentId, String> {
    let v = data
        .get(key)
        .ok_or_else(|| format!("ActionComponent missing '{key}'"))?;
    decode_id(v)
}

fn get_ids(
    data: &std::collections::HashMap<String, serde_json::Value>,
    key: &str,
) -> Result<Vec<ComponentId>, String> {
    let v = data
        .get(key)
        .ok_or_else(|| format!("ActionComponent missing '{key}'"))?;
    decode_ids(v)
}

fn get_value_as<T: serde::de::DeserializeOwned>(
    data: &std::collections::HashMap<String, serde_json::Value>,
    key: &str,
) -> Result<T, String> {
    let v = data
        .get(key)
        .ok_or_else(|| format!("ActionComponent missing '{key}'"))?;
    serde_json::from_value(v.clone()).map_err(|e| format!("ActionComponent bad '{key}': {e}"))
}
