use crate::engine::ecs::component::Component;

/// Controls which pointer event types a raycastable captures vs. passes through.
///
/// When the gesture system walks the depth-sorted hit list, it stops at the first hit that
/// captures the event type being resolved. Objects behind a capturer never see that event type.
///
/// ```
/// depth-sorted hits: [drag_plane (DragOnly), row (All)]
///
/// for drag  → drag_plane captures, stops. row never sees DragStart/DragMove.
/// for click → drag_plane passes (DragOnly). row captures, stops.
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerEvents {
    /// Captures drag and click. Default for all raycastable geometry.
    #[default]
    All,
    /// Captures drag events only; click propagates to the next hit.
    DragOnly,
    /// Captures click events only; drag propagates to the next hit.
    ClickOnly,
    /// Passes all pointer events through. Geometry is hittable but invisible to the gesture
    /// system (e.g. a structural collision volume that should never receive input).
    PassThrough,
}

impl PointerEvents {
    pub fn captures_drag(self) -> bool {
        matches!(self, Self::All | Self::DragOnly)
    }
    pub fn captures_click(self) -> bool {
        matches!(self, Self::All | Self::ClickOnly)
    }
}

/// Controls whether renderables should be eligible for ray casting (BVH insertion).
///
/// This is intentionally separate from `RenderableComponent` so raycasting policy can be
/// expressed via topology/components rather than renderable data.
#[derive(Debug, Default, Clone, Copy)]
pub struct RaycastableComponent {
    /// If true, ray casting is enabled.
    pub enable: bool,
    /// Which pointer event types this object captures vs. passes through to hits behind it.
    pub pointer_events: PointerEvents,
    /// Higher values win interaction ordering before distance-based tie-breaking.
    pub interaction_priority: u8,
}

impl RaycastableComponent {
    pub fn new(enable: bool) -> Self {
        Self {
            enable,
            pointer_events: PointerEvents::All,
            interaction_priority: 0,
        }
    }

    pub fn enabled() -> Self {
        Self::new(true)
    }

    pub fn disabled() -> Self {
        Self::new(false)
    }

    /// Captures drag events only; click falls through to hits behind this object.
    pub fn drag_only() -> Self {
        Self {
            enable: true,
            pointer_events: PointerEvents::DragOnly,
            interaction_priority: 0,
        }
    }

    /// Captures click events only; drag falls through to hits behind this object.
    pub fn click_only() -> Self {
        Self {
            enable: true,
            pointer_events: PointerEvents::ClickOnly,
            interaction_priority: 0,
        }
    }

    pub fn with_interaction_priority(mut self, interaction_priority: u8) -> Self {
        self.interaction_priority = interaction_priority;
        self
    }
}

impl Component for RaycastableComponent {
    fn init(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: crate::engine::ecs::ComponentId,
    ) {
        emit.push_intent(
            component,
            crate::engine::ecs::IntentSignal::now(
                crate::engine::ecs::IntentValue::RegisterRaycastable {
                    component_ids: vec![component],
                },
            ),
        );
    }

    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: crate::engine::ecs::ComponentId,
    ) {
        emit.push_intent(
            component,
            crate::engine::ecs::IntentSignal::now(
                crate::engine::ecs::IntentValue::RemoveRaycastable {
                    component_ids: vec![component],
                },
            ),
        );
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "raycastable"
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match (self.enable, self.pointer_events) {
            (false, _) => "disabled",
            (true, PointerEvents::DragOnly) => "drag_only",
            (true, PointerEvents::ClickOnly) => "click_only",
            (true, _) => "enabled",
        };
        let mut ce = ce_call("Raycastable", ctor, vec![]);
        if self.enable && matches!(self.pointer_events, PointerEvents::PassThrough) {
            ce = ce.with_call("pointer_events", vec![s("pass_through")]);
        }
        if self.enable && self.interaction_priority > 0 {
            ce = ce.with_call(
                "interaction_priority",
                vec![num(self.interaction_priority as f64)],
            );
        }
        ce
    }
}
