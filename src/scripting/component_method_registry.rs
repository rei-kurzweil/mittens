use crate::engine::ecs::component::{EmissiveComponent, TransformComponent, TransitionComponent};
use crate::engine::ecs::{ComponentId, IntentValue, PoseApplyMode, World};
use crate::scripting::object::Value;

pub(crate) fn supports_component_method(component_type: &str, method: &str) -> bool {
    (matches!(
        component_type,
        "T" | "Transform" | "TransformComponent" | "transform"
    ) && matches!(method, "update_transform" | "look_at" | "translation"))
        || (matches!(
            component_type,
            "PoseCapturePose" | "PoseCapturePoseComponent" | "pose_capture_pose"
        ) && matches!(method, "apply" | "overlay" | "apply_blended"))
        || (matches!(
            component_type,
            "EM" | "Emissive" | "EmissiveComponent" | "emissive"
        ) && matches!(method, "set_intensity" | "on" | "off"))
        || (matches!(
            component_type,
            "HttpClient" | "HttpClientComponent" | "http_client"
        ) && matches!(method, "get" | "post" | "put" | "delete"))
        || (matches!(
            component_type,
            "HttpServer" | "HttpServerComponent" | "http_server"
        ) && matches!(method, "reply_text"))
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
        (
            "PoseCapturePose" | "PoseCapturePoseComponent" | "pose_capture_pose",
            method @ ("apply" | "overlay" | "apply_blended"),
        ) => {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::PoseCapturePoseComponent>(id)
                .ok_or_else(|| format!("{method}(): not a PoseCapturePoseComponent"))?;
            let target = match args.first() {
                Some(Value::ComponentObject { id, .. }) => *id,
                other => return Err(format!("{method}(): expected a component target, got {other:?}")),
            };
            let expected = if method == "apply_blended" { 2 } else { 1 };
            if args.len() != expected {
                return Err(format!("{method}(): expected {expected} argument(s), got {}", args.len()));
            }
            let mode = match method {
                "apply" => PoseApplyMode::Replace,
                "overlay" => PoseApplyMode::Overlay,
                "apply_blended" => {
                    let amount = match args.get(1) {
                        Some(Value::Number(value)) => *value as f32,
                        other => return Err(format!("apply_blended(): expected numeric amount, got {other:?}")),
                    };
                    PoseApplyMode::RestBlend { amount: amount.clamp(0.0, 1.0) }
                }
                _ => unreachable!(),
            };
            emit_intent(IntentValue::PoseApply { target, pose: id, mode });
            Ok(Value::Null)
        }
        ("T" | "Transform" | "TransformComponent" | "transform", "translation") => {
            if !args.is_empty() {
                return Err(format!("translation(): expected no arguments, got {args:?}"));
            }
            let translation = world
                .get_component_by_id_as::<TransformComponent>(id)
                .ok_or_else(|| "translation(): not a TransformComponent".to_string())?
                .transform
                .translation;
            Ok(Value::Array(
                translation
                    .into_iter()
                    .map(|value| Value::Number(value as f64))
                    .collect(),
            ))
        }
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
        ("T" | "Transform" | "TransformComponent" | "transform", "look_at") => {
            let [target_world] = match args {
                [target_world] => [value_as_f32_array::<3>(target_world)?],
                other => {
                    return Err(format!(
                        "look_at: expected one vec3 array argument, got {:?}",
                        other
                    ));
                }
            };

            world
                .get_component_by_id_as::<TransformComponent>(id)
                .ok_or_else(|| "look_at(): not a TransformComponent".to_string())?;

            emit_intent(IntentValue::LookAt {
                component_ids: vec![id],
                target_world,
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
        ("HttpClient" | "HttpClientComponent" | "http_client", "get" | "delete") => {
            let [url] = match args {
                [url] => [value_as_string(url, method)?],
                other => {
                    return Err(format!(
                        "{method}: expected one string url argument, got {:?}",
                        other
                    ));
                }
            };
            emit_intent(IntentValue::HttpClientRequest {
                component_id: id,
                method: method.to_ascii_uppercase(),
                url,
                headers: vec![],
                body_text: None,
            });
            Ok(Value::Null)
        }
        ("HttpClient" | "HttpClientComponent" | "http_client", "post" | "put") => {
            let (url, body_text) = match args {
                [url, body_text] => (
                    value_as_string(url, method)?,
                    value_as_string(body_text, method)?,
                ),
                other => {
                    return Err(format!(
                        "{method}: expected url and body_text string arguments, got {:?}",
                        other
                    ));
                }
            };
            emit_intent(IntentValue::HttpClientRequest {
                component_id: id,
                method: method.to_ascii_uppercase(),
                url,
                headers: vec![],
                body_text: Some(body_text),
            });
            Ok(Value::Null)
        }
        ("HttpServer" | "HttpServerComponent" | "http_server", "reply_text") => {
            let (request_id, status, body_text) = match args {
                [request, status, body_text] => (
                    request_id_from_value(request)?,
                    value_as_u16(status, method)?,
                    value_as_string(body_text, method)?,
                ),
                other => {
                    return Err(format!(
                        "reply_text: expected request, status, body_text arguments, got {:?}",
                        other
                    ));
                }
            };
            emit_intent(IntentValue::HttpServerReply {
                component_id: id,
                request_id,
                status,
                headers: vec![],
                body_text,
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

fn value_as_string(value: &Value, method: &str) -> Result<String, String> {
    match value {
        Value::String(s) => Ok(s.clone()),
        other => Err(format!("{method}: expected string, got {:?}", other)),
    }
}

fn value_as_u16(value: &Value, method: &str) -> Result<u16, String> {
    match value {
        Value::Number(n) if *n >= 0.0 && *n <= u16::MAX as f64 => Ok(*n as u16),
        other => Err(format!("{method}: expected status number, got {:?}", other)),
    }
}

fn request_id_from_value(value: &Value) -> Result<u64, String> {
    let Value::Map(map) = value else {
        return Err(format!(
            "reply_text: expected request object, got {:?}",
            value
        ));
    };
    let Some(Value::Number(request_id)) = map.get("request_id") else {
        return Err("reply_text: request missing numeric request_id".to_string());
    };
    if *request_id < 0.0 {
        return Err("reply_text: request_id must be non-negative".to_string());
    }
    Ok(*request_id as u64)
}
