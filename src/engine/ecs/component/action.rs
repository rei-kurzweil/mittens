use super::Component;
use crate::engine::ecs::{ComponentId, IntentValue};
use slotmap::{Key, KeyData};

/// How a ComponentId target was authored. Preserved verbatim through dump
/// so save → reload reproduces the original source form.
///
/// `Guid` covers two authoring paths that collapse to "we know the target's
/// uuid": author wrote `@uuid:<hex>` as a selector string, or author passed
/// a live `Value::ComponentObject` (let-bound / query result) which the
/// registry resolves to a guid at call-construction time.
///
/// `Query` is anything else the author wrote as a selector string —
/// `#name`, `[attr=value]`, etc. — preserved as-is so dump emits the same
/// string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionTarget {
    Guid(uuid::Uuid),
    Query(String),
}

#[derive(Debug, Clone)]
pub struct ActionComponent {
    /// Runtime intent. ComponentId slots inside are `ComponentId::null()`
    /// placeholders until resolution; after resolution, they're real ids
    /// filled in from `target_sources` in the variant's declaration order.
    pub signal: IntentValue,
    /// Authoring metadata, one entry per ComponentId slot in `signal`,
    /// ordered by the slot's declaration order in the variant. Used by
    /// dump (lossless round-trip) and by resolution (look up ids).
    pub target_sources: Vec<ActionTarget>,
    /// Whether `signal`'s ComponentId slots hold real ids (true) or null
    /// placeholders (false). Set by the resolution pass invoked by
    /// `AnimationSystem` per the owning `AnimationComponent`'s configured
    /// resolve timing.
    pub resolved: bool,
}

impl ActionComponent {
    /// Construct from an already-resolved IntentValue (no ComponentId
    /// targets, or all targets pre-resolved). Use this for built-in /
    /// engine-emitted actions; MMS authoring goes through the registry
    /// which builds with `new_authored` instead.
    pub fn new(signal: IntentValue) -> Self {
        Self {
            signal,
            target_sources: Vec::new(),
            resolved: true,
        }
    }

    /// Construct from a signal whose ComponentId slots are placeholders
    /// plus the authoring sources for each slot (in declaration order).
    /// `resolved` starts false; resolution happens when the owning
    /// `AnimationSystem` first processes this action.
    pub fn new_authored(signal: IntentValue, target_sources: Vec<ActionTarget>) -> Self {
        Self {
            signal,
            target_sources,
            resolved: false,
        }
    }

    pub fn print(message: impl Into<String>) -> Self {
        Self::new(IntentValue::Print {
            message: message.into(),
        })
    }
}

impl Default for ActionComponent {
    fn default() -> Self {
        Self {
            signal: IntentValue::Noop,
            target_sources: Vec::new(),
            resolved: true,
        }
    }
}

// ---------------------------------------------------------------------------
// IntentValue slot enumeration
//
// Used by ActionComponent for: (a) sanity-checking that `target_sources.len()`
// matches the number of ComponentId slots in the variant; (b) reading current
// slot values for dump; (c) writing resolved ids into slots after lookup.
//
// Only covers the variants ActionComponent.signal actually carries. Variants
// the engine emits internally (Register*, intra-system bookkeeping) are not
// authorable from MMS and never appear here — they return zero slots.
// ---------------------------------------------------------------------------

/// Number of ComponentId slots in `signal`'s variant (counts each element
/// of any `Vec<ComponentId>` field).
pub fn signal_target_slot_count(signal: &IntentValue) -> usize {
    use IntentValue::*;
    match signal {
        Noop | Print { .. } => 0,

        SetColor { component_ids, .. }
        | SetText { component_ids, .. }
        | SetPosition { component_ids, .. }
        | Detach { component_ids }
        | RemoveSubtree { component_ids }
        | AudioGraphRebuild { component_ids }
        | RequestRaycast { component_ids }
        | AudioLowPassSetCutoffHz { component_ids, .. }
        | AudioBandPassSetCenterHz { component_ids, .. }
        | OscillatorSetEnabled { component_ids, .. }
        | OscillatorSetPitch { component_ids, .. }
        | OscillatorScheduleSetPitch { component_ids, .. }
        | OscillatorScheduleSetNote { component_ids, .. }
        | OscillatorScheduleMusicNote { component_ids, .. }
        | MusicSetNote { component_ids, .. }
        | UpdateTransform { component_ids, .. } => component_ids.len(),

        Attach { parents, .. } | AttachClone { parents, .. } => parents.len() + 1,
        RemoveChild { parents, .. } | RemoveChildren { parents } => parents.len(),

        // Variants ActionComponent never carries — no authored targets.
        _ => 0,
    }
}

/// Apply resolved ids back into `signal`'s ComponentId slots in declaration
/// order. Caller must guarantee `ids.len() == signal_target_slot_count(signal)`.
pub fn apply_resolved_targets(signal: &mut IntentValue, ids: &[ComponentId]) {
    use IntentValue::*;
    let mut cursor = 0usize;
    let mut take = |n: usize| -> &[ComponentId] {
        let slice = &ids[cursor..cursor + n];
        cursor += n;
        slice
    };
    match signal {
        Noop | Print { .. } => {}

        SetColor { component_ids, .. }
        | SetText { component_ids, .. }
        | SetPosition { component_ids, .. }
        | Detach { component_ids }
        | RemoveSubtree { component_ids }
        | AudioGraphRebuild { component_ids }
        | RequestRaycast { component_ids }
        | AudioLowPassSetCutoffHz { component_ids, .. }
        | AudioBandPassSetCenterHz { component_ids, .. }
        | OscillatorSetEnabled { component_ids, .. }
        | OscillatorSetPitch { component_ids, .. }
        | OscillatorScheduleSetPitch { component_ids, .. }
        | OscillatorScheduleSetNote { component_ids, .. }
        | OscillatorScheduleMusicNote { component_ids, .. }
        | MusicSetNote { component_ids, .. }
        | UpdateTransform { component_ids, .. } => {
            let n = component_ids.len();
            component_ids.copy_from_slice(take(n));
        }

        Attach { parents, child } | AttachClone { parents, prefab_root: child } => {
            let n = parents.len();
            parents.copy_from_slice(take(n));
            *child = take(1)[0];
        }
        RemoveChild { parents, .. } | RemoveChildren { parents } => {
            let n = parents.len();
            parents.copy_from_slice(take(n));
        }

        _ => {}
    }
    debug_assert_eq!(cursor, ids.len(), "slot count mismatch in apply_resolved_targets");
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

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterAction {
                component_ids: vec![component],
            },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();

        match &self.signal {
            IntentValue::Noop => {
                map.insert("variant".to_string(), serde_json::json!("Noop"));
            }
            IntentValue::Print { message } => {
                map.insert("variant".to_string(), serde_json::json!("Print"));
                map.insert("message".to_string(), serde_json::json!(message));
            }

            IntentValue::SetColor {
                component_ids,
                rgba,
            } => {
                map.insert("variant".to_string(), serde_json::json!("SetColor"));
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
                map.insert("rgba".to_string(), serde_json::json!(rgba));
            }
            IntentValue::SetText {
                component_ids,
                text,
            } => {
                map.insert("variant".to_string(), serde_json::json!("SetText"));
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
                map.insert("text".to_string(), serde_json::json!(text));
            }
            IntentValue::SetPosition {
                component_ids,
                position,
            } => {
                map.insert("variant".to_string(), serde_json::json!("SetPosition"));
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
                map.insert("position".to_string(), serde_json::json!(position));
            }
            IntentValue::Attach { parents, child } => {
                map.insert("variant".to_string(), serde_json::json!("Attach"));
                map.insert(
                    "parents".to_string(),
                    serde_json::json!(encode_ids(parents)),
                );
                map.insert("child".to_string(), serde_json::json!(encode_id(*child)));
            }
            IntentValue::AttachClone {
                parents,
                prefab_root,
            } => {
                map.insert("variant".to_string(), serde_json::json!("AttachClone"));
                map.insert(
                    "parents".to_string(),
                    serde_json::json!(encode_ids(parents)),
                );
                map.insert(
                    "prefab_root".to_string(),
                    serde_json::json!(encode_id(*prefab_root)),
                );
            }
            IntentValue::Detach { component_ids } => {
                map.insert("variant".to_string(), serde_json::json!("Detach"));
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
            }
            IntentValue::RemoveChild { parents, index } => {
                map.insert("variant".to_string(), serde_json::json!("RemoveChild"));
                map.insert(
                    "parents".to_string(),
                    serde_json::json!(encode_ids(parents)),
                );
                map.insert("index".to_string(), serde_json::json!(index));
            }
            IntentValue::RemoveChildren { parents } => {
                map.insert("variant".to_string(), serde_json::json!("RemoveChildren"));
                map.insert(
                    "parents".to_string(),
                    serde_json::json!(encode_ids(parents)),
                );
            }
            IntentValue::RemoveSubtree { component_ids } => {
                map.insert("variant".to_string(), serde_json::json!("RemoveSubtree"));
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
            }

            IntentValue::AudioGraphRebuild { component_ids } => {
                map.insert(
                    "variant".to_string(),
                    serde_json::json!("AudioGraphRebuild"),
                );
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
            }
            IntentValue::RequestRaycast { component_ids } => {
                map.insert("variant".to_string(), serde_json::json!("RequestRaycast"));
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
            }
            IntentValue::AudioLowPassSetCutoffHz {
                component_ids,
                cutoff_hz,
            } => {
                map.insert(
                    "variant".to_string(),
                    serde_json::json!("AudioLowPassSetCutoffHz"),
                );
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
                map.insert("cutoff_hz".to_string(), serde_json::json!(cutoff_hz));
            }
            IntentValue::AudioBandPassSetCenterHz {
                component_ids,
                center_hz,
            } => {
                map.insert(
                    "variant".to_string(),
                    serde_json::json!("AudioBandPassSetCenterHz"),
                );
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
                map.insert("center_hz".to_string(), serde_json::json!(center_hz));
            }
            IntentValue::OscillatorSetEnabled {
                component_ids,
                enabled,
            } => {
                map.insert(
                    "variant".to_string(),
                    serde_json::json!("OscillatorSetEnabled"),
                );
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
                map.insert("enabled".to_string(), serde_json::json!(enabled));
            }
            IntentValue::OscillatorSetPitch {
                component_ids,
                frequency_hz,
            } => {
                map.insert(
                    "variant".to_string(),
                    serde_json::json!("OscillatorSetPitch"),
                );
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
                map.insert("frequency_hz".to_string(), serde_json::json!(frequency_hz));
            }
            IntentValue::OscillatorScheduleSetPitch {
                component_ids,
                beat_offset,
                beat_context,
                frequency_hz,
            } => {
                map.insert(
                    "variant".to_string(),
                    serde_json::json!("OscillatorScheduleSetPitch"),
                );
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
                map.insert("beat_offset".to_string(), serde_json::json!(beat_offset));
                map.insert("beat_context".to_string(), serde_json::json!(beat_context));
                map.insert("frequency_hz".to_string(), serde_json::json!(frequency_hz));
            }
            IntentValue::OscillatorScheduleSetNote {
                component_ids,
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
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
                map.insert("beat_offset".to_string(), serde_json::json!(beat_offset));
                map.insert("beat_context".to_string(), serde_json::json!(beat_context));
                map.insert("pitch".to_string(), serde_json::json!(pitch));
                map.insert("octave".to_string(), serde_json::json!(octave));
                map.insert(
                    "duration_beats".to_string(),
                    serde_json::json!(duration_beats),
                );
            }
            IntentValue::OscillatorScheduleMusicNote {
                component_ids,
                beat_offset,
                beat_context,
                note,
            } => {
                map.insert(
                    "variant".to_string(),
                    serde_json::json!("OscillatorScheduleMusicNote"),
                );
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
                map.insert("beat_offset".to_string(), serde_json::json!(beat_offset));
                map.insert("beat_context".to_string(), serde_json::json!(beat_context));
                map.insert("note".to_string(), serde_json::json!(note));
            }
            IntentValue::MusicSetNote {
                component_ids,
                note,
            } => {
                map.insert("variant".to_string(), serde_json::json!("MusicSetNote"));
                map.insert(
                    "component_ids".to_string(),
                    serde_json::json!(encode_ids(component_ids)),
                );
                map.insert("note".to_string(), serde_json::json!(note));
            }

            other => {
                map.insert("variant".to_string(), serde_json::json!("Noop"));
                map.insert(
                    "error".to_string(),
                    serde_json::json!(format!(
                        "Unsupported intent variant in ActionComponent: {other:?}"
                    )),
                );
            }
        }

        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let variant = get_string(data, "variant")?;

        self.signal = match variant.as_str() {
            "Noop" => IntentValue::Noop,
            "Print" => IntentValue::Print {
                message: get_string(data, "message")?,
            },

            "SetColor" => IntentValue::SetColor {
                component_ids: get_ids(data, "component_ids")?,
                rgba: get_value_as(data, "rgba")?,
            },
            "SetText" => IntentValue::SetText {
                component_ids: get_ids(data, "component_ids")?,
                text: get_string(data, "text")?,
            },
            "SetPosition" => IntentValue::SetPosition {
                component_ids: get_ids(data, "component_ids")?,
                position: get_value_as(data, "position")?,
            },
            "Attach" => IntentValue::Attach {
                parents: get_ids(data, "parents")?,
                child: get_id(data, "child")?,
            },
            "AttachClone" => IntentValue::AttachClone {
                parents: get_ids(data, "parents")?,
                prefab_root: get_id(data, "prefab_root")?,
            },
            "Detach" => IntentValue::Detach {
                component_ids: get_ids(data, "component_ids")?,
            },
            "RemoveChild" => IntentValue::RemoveChild {
                parents: get_ids(data, "parents")?,
                index: get_value_as(data, "index")?,
            },
            "RemoveChildren" => IntentValue::RemoveChildren {
                parents: get_ids(data, "parents")?,
            },
            "RemoveSubtree" => IntentValue::RemoveSubtree {
                component_ids: get_ids(data, "component_ids")?,
            },
            "AudioGraphRebuild" => IntentValue::AudioGraphRebuild {
                component_ids: get_ids(data, "component_ids")?,
            },
            "RequestRaycast" => IntentValue::RequestRaycast {
                component_ids: get_ids(data, "component_ids")?,
            },
            "AudioLowPassSetCutoffHz" => IntentValue::AudioLowPassSetCutoffHz {
                component_ids: get_ids(data, "component_ids")?,
                cutoff_hz: get_value_as(data, "cutoff_hz")?,
            },
            "AudioBandPassSetCenterHz" => IntentValue::AudioBandPassSetCenterHz {
                component_ids: get_ids(data, "component_ids")?,
                center_hz: get_value_as(data, "center_hz")?,
            },
            "OscillatorSetEnabled" => IntentValue::OscillatorSetEnabled {
                component_ids: get_ids(data, "component_ids")?,
                enabled: get_value_as(data, "enabled")?,
            },
            "OscillatorSetPitch" => IntentValue::OscillatorSetPitch {
                component_ids: get_ids(data, "component_ids")?,
                frequency_hz: get_value_as(data, "frequency_hz")?,
            },
            "OscillatorScheduleSetPitch" => IntentValue::OscillatorScheduleSetPitch {
                component_ids: get_ids(data, "component_ids")?,
                beat_offset: get_value_as(data, "beat_offset")?,
                beat_context: get_value_as(data, "beat_context")?,
                frequency_hz: get_value_as(data, "frequency_hz")?,
            },
            "OscillatorScheduleSetNote" => IntentValue::OscillatorScheduleSetNote {
                component_ids: get_ids(data, "component_ids")?,
                beat_offset: get_value_as(data, "beat_offset")?,
                beat_context: get_value_as(data, "beat_context")?,
                pitch: get_value_as(data, "pitch")?,
                octave: get_value_as(data, "octave")?,
                duration_beats: get_value_as(data, "duration_beats")?,
            },
            "OscillatorScheduleMusicNote" => IntentValue::OscillatorScheduleMusicNote {
                component_ids: get_ids(data, "component_ids")?,
                beat_offset: get_value_as(data, "beat_offset")?,
                beat_context: get_value_as(data, "beat_context")?,
                note: get_value_as(data, "note")?,
            },
            "MusicSetNote" => IntentValue::MusicSetNote {
                component_ids: get_ids(data, "component_ids")?,
                note: get_value_as(data, "note")?,
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
    Ok(ffi
        .into_iter()
        .map(|x| KeyData::from_ffi(x).into())
        .collect())
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

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::KeyData;

    fn cid(n: u64) -> ComponentId {
        ComponentId::from(KeyData::from_ffi(n))
    }

    #[test]
    fn slot_count_matches_apply_for_attach_variant() {
        let mut iv = IntentValue::Attach {
            parents: vec![ComponentId::null(), ComponentId::null()],
            child: ComponentId::null(),
        };
        assert_eq!(signal_target_slot_count(&iv), 3);
        apply_resolved_targets(&mut iv, &[cid(10), cid(11), cid(12)]);
        let IntentValue::Attach { parents, child } = iv else {
            unreachable!()
        };
        assert_eq!(parents, vec![cid(10), cid(11)]);
        assert_eq!(child, cid(12));
    }

    #[test]
    fn slot_count_matches_apply_for_vec_only_variant() {
        let mut iv = IntentValue::SetColor {
            component_ids: vec![ComponentId::null(), ComponentId::null()],
            rgba: [1.0, 0.0, 0.0, 1.0],
        };
        assert_eq!(signal_target_slot_count(&iv), 2);
        apply_resolved_targets(&mut iv, &[cid(7), cid(8)]);
        let IntentValue::SetColor { component_ids, .. } = iv else {
            unreachable!()
        };
        assert_eq!(component_ids, vec![cid(7), cid(8)]);
    }

    #[test]
    fn no_slots_for_print() {
        let iv = IntentValue::Print {
            message: "hi".into(),
        };
        assert_eq!(signal_target_slot_count(&iv), 0);
    }
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
