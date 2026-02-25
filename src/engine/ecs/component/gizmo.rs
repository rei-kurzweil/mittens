use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    Translate,
}

/// A simple transform gizmo.
///
/// Attach this as a child of a RenderableComponent you want to be "hit" by raycasts.
/// When a drag gesture is active on that renderable, GizmoSystem will apply the drag delta
/// to `target_transform`.
#[derive(Debug, Clone, Copy)]
pub struct GizmoComponent {
    pub target_transform: ComponentId,
    pub mode: GizmoMode,

    /// Runtime: raycaster currently driving this gizmo (single-pointer for now).
    pub active_raycaster: Option<ComponentId>,

    /// Root TransformComponent id of the gizmo visual subtree (spawned on init).
    pub visual_root: Option<ComponentId>,

    component: Option<ComponentId>,
}

impl GizmoComponent {
    pub fn translate(target_transform: ComponentId) -> Self {
        Self {
            target_transform,
            mode: GizmoMode::Translate,
            active_raycaster: None,
            visual_root: None,
            component: None,
        }
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Component for GizmoComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "gizmo"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, queue: &mut crate::engine::ecs::CommandQueue, component: ComponentId) {
        // Defer spawning the visual subtree to the command queue flush phase.
        queue.queue_register_gizmo(component);
    }

    fn cleanup(&mut self, queue: &mut crate::engine::ecs::CommandQueue, _component: ComponentId) {
        if let Some(root) = self.visual_root.take() {
            queue.queue_remove_subtree(root);
        }
    }
}
