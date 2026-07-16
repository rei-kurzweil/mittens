use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Select a mesh for a Renderable by string key.
///
/// This is intended to be attached as a descendant of a `RenderableComponent`.
/// The key can refer to imported meshes (e.g. "{gltf}:{mesh}:{prim}") registered in `RenderAssets`.
#[derive(Debug, Clone)]
pub struct MeshComponent {
    pub key: String,
}

impl MeshComponent {
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into() }
    }
}

impl Component for MeshComponent {
    fn name(&self) -> &'static str {
        "mesh"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, _emit: &mut dyn crate::engine::ecs::SignalEmitter, _component: ComponentId) {
        // No-op: RenderableSystem resolves this opportunistically during flush.
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call("Mesh", "new", vec![s(&self.key)])
    }
}
