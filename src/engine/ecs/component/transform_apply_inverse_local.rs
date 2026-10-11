use super::{Component, ComponentRef, QueryRootMode, resolve_component_ref};
use crate::engine::ecs::{ComponentId, World};

/// Applies the inverse of a referenced transform's authored local matrix to
/// the inherited transform stream. The affected destination is expressed by
/// this component's child topology.
#[derive(Debug, Clone)]
pub struct TransformApplyInverseLocalComponent {
    pub source: ComponentRef,
}

impl TransformApplyInverseLocalComponent {
    pub fn new(source: ComponentRef) -> Self {
        Self { source }
    }

    pub fn with_source(mut self, source: ComponentRef) -> Self {
        self.source = source;
        self
    }

    pub fn resolve_source_component(
        &self,
        world: &World,
        owner: ComponentId,
    ) -> Option<ComponentId> {
        resolve_component_ref(world, &self.source, Some(owner), QueryRootMode::WorldRoot)
    }
}

impl Component for TransformApplyInverseLocalComponent {
    fn name(&self) -> &'static str {
        "transform_apply_inverse_local"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::UpdateTransformWorld {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::{ce_call, s};

        let source = match &self.source {
            ComponentRef::Guid(guid) => format!("@uuid:{guid}"),
            ComponentRef::Query(query) => query.clone(),
        };
        ce_call("TransformOperator", "inverse_local", vec![s(&source)])
    }
}
