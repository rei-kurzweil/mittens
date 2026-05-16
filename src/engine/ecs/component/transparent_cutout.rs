use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Marker component: route this renderable into the "transparent cutout" pass.
///
/// Intended to be attached as a descendant of a `RenderableComponent`.
///
/// Semantics:
/// - Uses alpha-to-coverage (MSAA) instead of blending.
/// - Depth test/write stays enabled, so it behaves like opaque geometry for ordering.
#[derive(Debug, Clone, Copy)]
pub struct TransparentCutoutComponent {
    pub enabled: bool,
}

impl TransparentCutoutComponent {
    pub fn new() -> Self {
        Self { enabled: true }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Default for TransparentCutoutComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TransparentCutoutComponent {
    fn name(&self) -> &'static str {
        "transparent_cutout"
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
            crate::engine::ecs::IntentValue::RegisterTransparentCutout {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        if self.enabled {
            ce("TransparentCutout")
        } else {
            ce_call("TransparentCutout", "disabled", vec![])
        }
    }
}
