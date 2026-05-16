use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Per-vertex UVs for a renderable.
///
/// This is intended to be attached as a descendant of a `RenderableComponent`.
///
/// Lifecycle note:
/// - UV overrides are applied when the renderable is flushed into `VisualWorld` / uploaded.
/// - If fewer UVs are provided than the mesh's vertex count, the missing UVs are filled with 0.
#[derive(Debug, Clone)]
pub struct UVComponent {
    pub uvs: Vec<[f32; 2]>,
}

impl UVComponent {
    pub fn new() -> Self {
        Self { uvs: Vec::new() }
    }

    /// Construct from a nested vector, where each inner vec is `[u, v]`.
    ///
    /// - If an inner vec has <2 values, missing values are treated as 0.
    /// - If it has >2 values, extras are ignored.
    pub fn from_vec(uvs: Vec<Vec<f32>>) -> Self {
        let mut out: Vec<[f32; 2]> = Vec::with_capacity(uvs.len());
        for uv in uvs {
            let u = uv.get(0).copied().unwrap_or(0.0);
            let v = uv.get(1).copied().unwrap_or(0.0);
            out.push([u, v]);
        }
        Self { uvs: out }
    }

    pub fn with_uv(mut self, u: f32, v: f32) -> Self {
        self.uvs.push([u, v]);
        self
    }
}

impl Default for UVComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for UVComponent {
    fn name(&self) -> &'static str {
        "uv"
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
            crate::engine::ecs::IntentValue::RegisterUv {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = ce("UV");
        for [u, v] in &self.uvs {
            ce = ce.with_call("uv", vec![num(*u as f64), num(*v as f64)]);
        }
        ce
    }
}
