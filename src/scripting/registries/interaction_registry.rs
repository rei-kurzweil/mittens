use super::{ComponentId, GrabbableComponent, Value, World, arg_bool};

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
        Some("parent") => GrabbableComponent::parent(),
        Some("off") => GrabbableComponent::off(),
        Some("on") => GrabbableComponent::on(),
        _ => GrabbableComponent::new(),
    };
    Ok(world.add_component(component))
}
