use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Debug component: visualises vertex normals of the parent `RenderableComponent`.
///
/// Spawns one thin cyan emissive cube per vertex, oriented along the vertex normal.
/// Cubes are 10× taller along Y than X/Z, making them read as needle-like indicators.
///
/// Attach as a child of a `RenderableComponent`. The visualisation subtree is spawned
/// automatically on init and torn down on cleanup.
#[derive(Debug, Clone)]
pub struct NormalVisualisationComponent {
    /// Thickness of each indicator cube (X and Z scale).
    /// Y scale = thickness * 10.
    pub thickness: f32,

    /// Root ComponentIds of spawned indicator subtrees, stored for cleanup.
    pub spawned_roots: Vec<ComponentId>,

    /// Internal: own ComponentId, stored by set_id.
    component: Option<ComponentId>,
}

impl NormalVisualisationComponent {
    pub fn new() -> Self {
        Self {
            thickness: 0.02,
            spawned_roots: Vec::new(),
            component: None,
        }
    }

    pub fn with_thickness(mut self, t: f32) -> Self {
        self.thickness = t;
        self
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Default for NormalVisualisationComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for NormalVisualisationComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "normal_visualisation"
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
            crate::engine::ecs::IntentValue::RegisterNormalVis {
                component_ids: vec![component],
            },
        );
    }

    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        _component: ComponentId,
    ) {
        for root in self.spawned_roots.drain(..) {
            emit.push_intent_now(
                root,
                crate::engine::ecs::IntentValue::RemoveSubtree {
                    component_ids: vec![root],
                },
            );
        }
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call("NormalVis", "thickness", vec![num(self.thickness as f64)])
    }
}
