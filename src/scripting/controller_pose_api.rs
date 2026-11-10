//! Copied controller poses for MMS, using the same TRS table as Transform.world_trs.
use super::runtime_config::{MittensBinding, component_signature, host_method};
use crate::engine::ecs::component::ControllerXRComponent;
use crate::engine::ecs::{ComponentId, World};
use meow_meow_script as mms;

pub(super) fn configure(component: &mut mms::ComponentBuilder<'_, MittensBinding>) {
    component.constructor(
        "new",
        component_signature([
            mms::ValueType::Bool,
            mms::ValueType::String,
            mms::ValueType::String,
        ]),
    );
    component.builder_call("laser", component_signature([]));
    host_method(
        component,
        "XRHand",
        "get_grip_pose",
        mms::ValueSignature::new(vec![], mms::ValueType::Any),
    );
}

pub(super) fn get_grip_pose(world: &World, id: ComponentId) -> Result<mms::Value, String> {
    let controller = world
        .get_component_by_id_as::<ControllerXRComponent>(id)
        .ok_or("receiver is not XRHand")?;
    let Some(pose) = controller.get_grip_pose(world) else {
        return Ok(mms::Value::Null);
    };
    let vector =
        |v: &[f32]| mms::Value::Array(v.iter().map(|v| mms::Value::Number(*v as f64)).collect());
    Ok(mms::Value::Map(std::collections::HashMap::from([
        ("translation".into(), vector(&pose.translation)),
        (
            "rotation_quat_xyzw".into(),
            vector(&pose.rotation_quat_xyzw),
        ),
        ("scale".into(), vector(&pose.scale)),
    ])))
}
