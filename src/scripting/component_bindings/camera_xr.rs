use crate::engine::{ecs::component::CameraXRComponent, graphics::CameraTarget};
use crate::scripting::{MittensBinding, object::Value};
use meow_meow_script as mms;

pub(super) fn configure(component: &mut mms::ComponentBuilder<'_, MittensBinding>) {
    for method in ["on", "off"] {
        component.constructor(
            method,
            mms::ValueSignature::new([], mms::ValueType::Component),
        );
    }
    component.builder_call(
        "enabled",
        mms::ValueSignature::new([mms::ValueType::Bool], mms::ValueType::Component),
    );
    component.builder_call(
        "target",
        mms::ValueSignature::new([mms::ValueType::String], mms::ValueType::Component),
    );
    for method in ["near", "far"] {
        let signature = mms::ValueSignature::new([mms::ValueType::F32], mms::ValueType::Component);
        component
            .constructor(method, signature.clone())
            .builder_call(method, signature);
    }
}
pub(super) fn construct(method: Option<&str>, args: &[Value]) -> Result<CameraXRComponent, String> {
    let mut camera = CameraXRComponent::on();
    if let Some(method) = method {
        apply(&mut camera, method, args)?;
    }
    Ok(camera)
}
pub(super) fn apply(
    camera: &mut CameraXRComponent,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    match method {
        "on" | "new" => camera.enabled = true,
        "off" => camera.enabled = false,
        "enabled" => match args.first() {
            Some(Value::Bool(value)) => camera.enabled = *value,
            _ => return Err("CameraXR.enabled expects a boolean".into()),
        },
        "target" => match args.first() {
            Some(Value::String(value)) => {
                camera.target = if value == "window" {
                    CameraTarget::Window
                } else {
                    CameraTarget::Xr
                }
            }
            _ => return Err("CameraXR.target expects a string".into()),
        },
        "near" | "far" => {
            let value = match args.first() {
                Some(Value::Number(value)) if value.is_finite() && *value > 0.0 => *value as f32,
                _ => {
                    return Err(format!(
                        "CameraXR.{method} expects a positive finite distance"
                    ));
                }
            };
            let (near, far) = if method == "near" {
                (value, camera.z_far)
            } else {
                (camera.z_near, value)
            };
            if !near.is_finite() || !far.is_finite() || near >= far {
                return Err("CameraXR requires 0 < near < far".into());
            }
            camera.z_near = near;
            camera.z_far = far;
        }
        _ => return Err(format!("unknown CameraXR method: {method}")),
    }
    Ok(())
}
