use crate::engine::ecs::{ComponentId, World, component::MirrorComponent};
use crate::scripting::{MittensBinding, object::Value};
use meow_meow_script as mms;

pub(super) fn configure(component: &mut mms::ComponentBuilder<'_, MittensBinding>) {
    component.constructor(
        "quality",
        mms::ValueSignature::new([mms::ValueType::I32], mms::ValueType::Component),
    );
    let enabled = mms::ValueSignature::new([mms::ValueType::Bool], mms::ValueType::Component);
    component
        .constructor("enabled", enabled.clone())
        .builder_call("enabled", enabled);
    super::runtime_config::host_method(
        component,
        "Mirror",
        "set_enabled",
        mms::ValueSignature::new([mms::ValueType::Bool], mms::ValueType::Null),
    );
}
pub(super) fn construct(method: Option<&str>, args: &[Value]) -> Result<MirrorComponent, String> {
    let mut mirror = MirrorComponent::default();
    if let Some(method) = method {
        apply(&mut mirror, method, args)?;
    }
    Ok(mirror)
}
pub(super) fn apply(
    mirror: &mut MirrorComponent,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    match (method, args) {
        ("quality", [Value::Number(value)]) if value.is_finite() => {
            mirror.quality = (*value as i32).clamp(64, 2048);
        }
        ("enabled", [Value::Bool(value)]) => mirror.enabled = *value,
        _ => return Err(format!("invalid Mirror.{method} arguments")),
    }
    Ok(())
}
pub(super) fn set_enabled(
    world: &mut World,
    id: ComponentId,
    args: &[Value],
    mut emit: impl FnMut(crate::engine::ecs::IntentValue),
) -> Result<Value, String> {
    if world
        .get_component_by_id_as::<MirrorComponent>(id)
        .is_none()
    {
        return Err("set_enabled(): not a Mirror".into());
    }
    let [Value::Bool(enabled)] = args else {
        return Err("set_enabled(): expected one boolean".into());
    };
    emit(crate::engine::ecs::IntentValue::MirrorSetEnabled {
        component_id: id,
        enabled: *enabled,
    });
    Ok(Value::Null)
}
