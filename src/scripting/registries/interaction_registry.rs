use super::{ComponentId, GrabbableComponent, Value, World, arg_bool};

pub(super) fn attachment_index(args: &[Value]) -> Result<i32, String> {
    if let [Value::Number(index)] = args
        && index.fract() == 0.0
        && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(index)
    {
        return Ok(*index as i32);
    }
    Err("expected a signed 32-bit attachment index".into())
}

pub(super) fn create_grabbable(
    world: &mut World,
    ctor: Option<&str>,
    args: &[Value],
) -> Result<ComponentId, String> {
    let component = match ctor {
        Some("to_hand") => {
            let mut component = GrabbableComponent::new();
            component.to_hand = arg_bool(args, 0)?;
            component
        }
        Some("attachment_index") => {
            let mut component = GrabbableComponent::new();
            component.attachment_index = Some(attachment_index(args)?);
            component
        }
        Some("parent") => GrabbableComponent::parent(),
        Some("off") => GrabbableComponent::off(),
        Some("on") => GrabbableComponent::on(),
        _ => GrabbableComponent::new(),
    };
    Ok(world.add_component(component))
}
