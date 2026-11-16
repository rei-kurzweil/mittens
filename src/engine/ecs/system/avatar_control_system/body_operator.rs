//! Body operator creation: yaw-follow between the avatar control and model root.
use super::*;

pub(super) fn create_body_operator(
    world: &mut World,
    id: ComponentId,
    model_root_id: ComponentId,
    body_yaw_threshold: f32,
    body_yaw_rate: f32,
    resolved_initial_body_yaw: f32,
    resolved_body_forward_plus_z: bool,
    emit: &mut dyn SignalEmitter,
) {
    // -----------------------------------------------------------------------
    // Body operator: created as a child of AVC; model_root re-parented under it.
    //
    // Topology:
    //   AVC
    //     └── body_operator  (TransformOperatorComponent)
    //           rotation_ops: YawFollow { threshold, rate, initial_yaw, forward_plus_z }
    //           model_root  ← re-parented here
    // -----------------------------------------------------------------------
    {
        use crate::engine::ecs::component::transform_operator::TransformQuatOp;
        let mut body_operator = TransformOperatorComponent::fork_trs();
        body_operator.push_rotation(TransformQuatOp::YawFollow {
            threshold: body_yaw_threshold,
            rate: body_yaw_rate,
            initial_yaw: resolved_initial_body_yaw,
            forward_plus_z: resolved_body_forward_plus_z,
        });
        let body_operator_id = world.add_component(body_operator);
        let body_operator_serialize_id = world.add_component(SerializeComponent::off());
        let _ = world.set_parent(body_operator_serialize_id, Some(body_operator_id));

        if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
            c.body_operator_id = Some(body_operator_id);
        }

        emit_attach(emit, id, body_operator_id);
        emit_attach(emit, body_operator_id, model_root_id);
    }
}
