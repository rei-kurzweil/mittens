use crate::engine::ecs::component::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Declares a renderable-backed stencil clip boundary.
///
/// On `init()`, emits `RegisterStencilClip` so VisualWorld records the renderable that should be
/// used as the clip source.
///
/// The renderer draws the referenced renderable into the stencil buffer
/// (color write off, stencil INCR, ref = nesting depth) before drawing the TC's
/// descendants, then restores stencil afterward with DECR. The same renderable
/// also draws normally in the color pass — it does double duty.
///
/// ## Layout use
///
/// `sync_bg_quad` attaches a layout-owned `StencilClipComponent` as a sibling of the generated
/// `__bg` helper whenever `overflow: Hidden | Scroll` is set on a style component. In that layout-
/// owned case, the computed `__bg` renderable remains the clip shape.
///
/// ## Manual use
///
/// Attach somewhere under the renderable whose mesh should define the clip region.
/// The nearest ancestor `RenderableComponent` determines the clip shape.
pub struct StencilClipComponent {
    /// Stencil reference depth. `0` = auto-assign based on ancestor nesting depth.
    pub stencil_ref: u8,
}

impl StencilClipComponent {
    pub fn new() -> Self {
        Self { stencil_ref: 0 }
    }
}

impl Default for StencilClipComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for StencilClipComponent {
    fn name(&self) -> &'static str {
        "stencil_clip"
    }

    fn set_id(&mut self, _component: ComponentId) {}

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RegisterStencilClip {
                component_ids: vec![component],
            },
        );
    }

    fn cleanup(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::UnregisterStencilClip {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        if self.stencil_ref == 0 {
            ce("StencilClip")
        } else {
            ce("StencilClip").with_call("stencil_ref", vec![num(self.stencil_ref as f64)])
        }
    }
}
