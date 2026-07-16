use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::ecs::{IntentValue, SignalEmitter};

/// Generic scroll state for moving a content track inside a clipped viewport.
///
/// `ScrollingComponent` does not own clipping; it only tracks viewport/content sizes,
/// current offset, which transform should be moved by the scroll runtime, and the
/// drag scope currently bridged into scroll events.
///
/// Expected topology:
/// ```text
/// viewport_root                  ← usually clipped by StyleComponent::overflow
///   └── ScrollingComponent       ← owned by ScrollingSystem
///         └── scroll_track       ← moved in +Y as scroll_offset increases
///               ├── child_0
///               ├── child_1
///               └── ...
/// ```
///
/// If no explicit `track` is assigned, `ScrollingSystem` now creates an internal `Router` plus an
/// owned `__scroll_track` transform under the `ScrollingComponent`, and routed children are sent
/// there both at init time and on later direct attaches.
#[derive(Debug, Clone)]
pub struct ScrollingComponent {
    /// Height of the clipped viewport in scroll-local/layout units.
    pub viewport_height: f32,
    /// Height of the scrollable content in scroll-local/layout units.
    pub content_height: f32,
    /// Current scroll offset in scroll-local/layout units. 0.0 = top.
    pub scroll_offset: f32,
    /// Transform moved by the scroll runtime.
    ///
    /// If unset at init time, the runtime will create an owned internal track by default.
    pub track: Option<ComponentId>,
    /// Base local-space position of `track` before any scrolling is applied.
    pub track_base_pos: [f32; 3],
    /// Ancestor scope currently forwarding drag motion into this scrolling component.
    pub drag_scope: Option<ComponentId>,

    component: Option<ComponentId>,
}

impl ScrollingComponent {
    pub fn new(viewport_height: f32, content_height: f32) -> Self {
        Self {
            viewport_height,
            content_height,
            scroll_offset: 0.0,
            track: None,
            track_base_pos: [0.0, 0.0, 0.0],
            drag_scope: None,
            component: None,
        }
    }

    pub fn set_track(&mut self, track: ComponentId, base_pos: [f32; 3]) {
        self.track = Some(track);
        self.track_base_pos = base_pos;
    }

    pub fn set_drag_scope(&mut self, drag_scope: ComponentId) {
        self.drag_scope = Some(drag_scope);
    }

    pub fn set_content_height(&mut self, content_height: f32) -> bool {
        self.content_height = content_height.max(0.0);
        self.clamp_to_content()
    }

    /// Maximum scroll distance in scroll-local/layout units.
    pub fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport_height).max(0.0)
    }

    /// Update `scroll_offset` by a scroll-local Y drag delta.
    ///
    /// Sign convention: dragging up (positive `delta_y`) reveals content lower in the list.
    pub fn apply_drag(&mut self, delta_y: f32) -> bool {
        let prev_offset = self.scroll_offset;
        self.scroll_offset -= delta_y;
        self.scroll_offset = self.scroll_offset.clamp(0.0, self.max_scroll());
        (self.scroll_offset - prev_offset).abs() > f32::EPSILON
    }

    /// Current translation that should be applied to the scroll track.
    pub fn track_translation(&self) -> [f32; 3] {
        [
            self.track_base_pos[0],
            self.track_base_pos[1] + self.scroll_offset,
            self.track_base_pos[2],
        ]
    }

    /// Clamp scroll after content size changes. Returns true if the position changed.
    pub fn clamp_to_content(&mut self) -> bool {
        let clamped = self.scroll_offset.clamp(0.0, self.max_scroll());
        if (clamped - self.scroll_offset).abs() > f32::EPSILON {
            self.scroll_offset = clamped;
            true
        } else {
            false
        }
    }
}

impl Component for ScrollingComponent {
    fn name(&self) -> &'static str {
        "scrolling"
    }

    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RegisterScrolling {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call(
            "Scrolling",
            "new",
            vec![
                num(self.viewport_height as f64),
                num(self.content_height as f64),
            ],
        )
    }
}
