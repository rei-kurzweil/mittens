use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};
use crate::engine::ecs::component::Component;

/// Declares an ancestor renderable as a stencil clip boundary.
///
/// Attach under a `RenderableComponent` (directly or a few levels below it). On `init()`, emits
/// `RegisterStencilClip` so VisualWorld records the nearest ancestor renderable as the clip source.
///
/// The renderer draws the referenced renderable into the stencil buffer
/// (color write off, stencil INCR, ref = nesting depth) before drawing the TC's
/// descendants, then restores stencil afterward with DECR. The same renderable
/// also draws normally in the color pass — it does double duty.
///
/// ## Layout use
///
/// `sync_bg_quad` attaches this under the generated `__bg` renderable whenever
/// `overflow: Hidden | Scroll` is set on a style component. The background quad mesh is the clip shape.
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
            IntentValue::RegisterStencilClip { component_ids: vec![component] },
        );
    }

    fn cleanup(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::UnregisterStencilClip { component_ids: vec![component] },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("stencil_ref".to_string(), serde_json::json!(self.stencil_ref));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("stencil_ref") {
            self.stencil_ref = serde_json::from_value(v.clone())
                .map_err(|e| format!("stencil_ref: {e}"))?;
        }
        Ok(())
    }
}
