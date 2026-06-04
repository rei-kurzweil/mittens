use crate::engine::ecs::RxWorld;
use crate::engine::ecs::component::SignalRouteUpwardComponent;
use crate::engine::ecs::rx::signal_pipeline::{
    SignalPipeline, SignalPipelineOp, SignalRouteUpward,
};
use crate::engine::ecs::{ComponentId, World};

/// Maintains RxWorld-owned intent routing pipelines based on operator components.
#[derive(Debug, Default)]
pub struct PipelineSystem;

impl PipelineSystem {
    pub fn register_signal_route_upward(
        &mut self,
        world: &World,
        rx: &mut RxWorld,
        operator_component: ComponentId,
    ) {
        let Some(cfg) =
            world.get_component_by_id_as::<SignalRouteUpwardComponent>(operator_component)
        else {
            return;
        };

        let Some(owner) = world.parent_of(operator_component) else {
            return;
        };

        let pipeline = SignalPipeline {
            source_operator: operator_component,
            ops: vec![SignalPipelineOp::RouteUpward(SignalRouteUpward {
                intent_kind: cfg.intent_kind.clone(),
                parent_type: cfg.parent_type.clone(),
            })],
        };

        rx.register_pipeline(owner, pipeline);
    }

    pub fn remove_signal_route_upward(
        &mut self,
        rx: &mut RxWorld,
        operator_component: ComponentId,
    ) {
        rx.remove_pipelines_from_operator(operator_component);
    }
}
