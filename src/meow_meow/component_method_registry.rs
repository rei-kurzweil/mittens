use crate::engine::ecs::component::{EmissiveComponent, TransformComponent, TransitionComponent};
use crate::engine::ecs::{ComponentId, IntentValue, World};
use crate::meow_meow::object::Value;

pub(crate) fn supports_component_method(component_type: &str, method: &str) -> bool {
    (matches!(
        component_type,
        "T" | "Transform" | "TransformComponent" | "transform"
    ) && method == "update_transform")
        || (matches!(
            component_type,
            "EM" | "Emissive" | "EmissiveComponent" | "emissive"
        ) && matches!(method, "set_intensity" | "on" | "off"))
}

pub(crate) fn invoke_component_method(
    world: &mut World,
    id: ComponentId,
    component_type: &str,
    method: &str,
    args: &[Value],
    mut emit_intent: impl FnMut(IntentValue),
) -> Result<Value, String> {
    match (component_type, method) {
        ("T" | "Transform" | "TransformComponent" | "transform", "update_transform") => {
            let [translation, rotation_euler, scale] = match args {
                [translation, rotation, scale] => [
                    value_as_f32_array::<3>(translation)?,
                    value_as_f32_array::<3>(rotation)?,
                    value_as_f32_array::<3>(scale)?,
                ],
                other => {
                    return Err(format!(
                        "update_transform: expected three vec3 array arguments, got {:?}",
                        other
                    ));
                }
            };

            world
                .get_component_by_id_as::<TransformComponent>(id)
                .ok_or_else(|| "update_transform(): not a TransformComponent".to_string())?;

            emit_intent(IntentValue::UpdateTransform {
                component_ids: vec![id],
                translation,
                rotation_quat_xyzw: TransformComponent::new()
                    .with_rotation_euler(rotation_euler[0], rotation_euler[1], rotation_euler[2])
                    .transform
                    .rotation,
                scale,
            });
            Ok(Value::Null)
        }
        ("EM" | "Emissive" | "EmissiveComponent" | "emissive", "set_intensity" | "on" | "off") => {
            let intensity = match method {
                "on" => 1.0,
                "off" => 0.0,
                "set_intensity" => match args.first() {
                    Some(Value::Number(n)) => (*n as f32).max(0.0),
                    Some(other) => {
                        return Err(format!(
                            "set_intensity: expected number argument, got {:?}",
                            other
                        ));
                    }
                    None => return Err("set_intensity: missing number argument".into()),
                },
                _ => unreachable!(),
            };

            world
                .get_component_by_id_as::<EmissiveComponent>(id)
                .ok_or_else(|| format!("{method}(): not an EmissiveComponent"))?;

            let has_transition_child = world.children_of(id).iter().any(|&child| {
                world
                    .get_component_by_id_as::<TransitionComponent>(child)
                    .is_some()
            });
            let is_attached = world.parent_of(id).is_some();
            if !(is_attached && has_transition_child) {
                let emissive = world
                    .get_component_by_id_as_mut::<EmissiveComponent>(id)
                    .ok_or_else(|| format!("{method}(): not an EmissiveComponent"))?;
                emissive.intensity = intensity;
            }

            emit_intent(IntentValue::SetEmissiveIntensity {
                component_ids: vec![id],
                intensity,
            });
            Ok(Value::Null)
        }
        _ => Err(format!(
            "unsupported live component method '{}.{}'",
            component_type, method
        )),
    }
}

fn value_as_f32_array<const N: usize>(value: &Value) -> Result<[f32; N], String> {
    let Value::Array(values) = value else {
        return Err(format!("expected array, got {:?}", value));
    };
    if values.len() != N {
        return Err(format!("expected array of len {}, got {}", N, values.len()));
    }
    let mut out = [0.0_f32; N];
    for (i, value) in values.iter().enumerate() {
        match value {
            Value::Number(n) => out[i] = *n as f32,
            other => return Err(format!("expected numeric array element, got {:?}", other)),
        }
    }
    Ok(out)
}
