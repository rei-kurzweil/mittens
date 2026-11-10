//! Pointer constructors and authoring controls for materialized scene trees.
use super::*;
pub(super) fn create(
    world: &mut World,
    ctor: Option<&str>,
    args: &[Value],
) -> Result<ComponentId, String> {
    let pointer = if ctor == Some("disabled") {
        PointerComponent::disabled()
    } else {
        PointerComponent::new()
    };
    let id = world.add_component(pointer);
    if let Some(method) = ctor.filter(|method| *method != "disabled") {
        apply(world, id, method, args)?;
    }
    Ok(id)
}
pub(super) fn apply(
    world: &mut World,
    id: ComponentId,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    if method == "debug_enable" {
        let enabled = arg_bool(args, 0)?;
        world
            .get_component_by_id_as_mut::<PointerComponent>(id)
            .expect("pointer checked above")
            .debug_enabled = enabled;
        return Ok(());
    }
    if method == "grab_pose" {
        let pose = match arg_str(args, 0)? {
            "hand_bone" => crate::engine::ecs::component::PointerGrabPose::HandBone,
            "controller_grip" => crate::engine::ecs::component::PointerGrabPose::ControllerGrip,
            _ => return Err("Pointer.grab_pose expects hand_bone or controller_grip".into()),
        };
        world
            .get_component_by_id_as_mut::<PointerComponent>(id)
            .unwrap()
            .grab_pose = pose;
        return Ok(());
    }
    if method == "grab_attachment" {
        let reference = arg_component_ref(world, args, 0)?;
        world
            .get_component_by_id_as_mut::<PointerComponent>(id)
            .unwrap()
            .grab_attachment = Some(reference);
        return Ok(());
    }
    let value = val_as_f32(arg(args, 0)?)?;
    let pointer = world
        .get_component_by_id_as_mut::<PointerComponent>(id)
        .expect("pointer checked above");
    match method {
        "min_grab_distance" if value.is_finite() && value >= 0.0 => {
            pointer.min_grab_distance = Some(value)
        }
        "click_max_screen_distance_px" if value.is_finite() && value >= 0.0 => {
            pointer.click_max_screen_distance_px = value
        }
        "click_max_ray_angle_deg" if value.is_finite() && (0.0..=180.0).contains(&value) => {
            pointer.click_max_ray_angle_deg = value
        }
        "click_max_origin_distance" if value.is_finite() && value >= 0.0 => {
            pointer.click_max_origin_distance = value
        }
        "click_max_ray_angle_deg" => {
            return Err(
                "Pointer.click_max_ray_angle_deg must be finite and between 0 and 180 degrees"
                    .into(),
            );
        }
        "min_grab_distance" | "click_max_screen_distance_px" | "click_max_origin_distance" => {
            return Err(format!("Pointer.{method} must be finite and non-negative"));
        }
        _ => {}
    }
    Ok(())
}
