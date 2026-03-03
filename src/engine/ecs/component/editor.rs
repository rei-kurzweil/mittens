use crate::engine::ecs::component::Component;
use crate::engine::ecs::ComponentId;

/// Marks an "editor root" subtree.
///
/// When a renderable under this subtree is clicked, the editor selection system can reattach
/// the editor's gizmos (e.g. TransformGizmo) to the clicked target.
#[derive(Debug, Default, Clone, Copy)]
pub struct EditorComponent {
    /// Runtime cache: resolved TransformGizmoComponent id within this editor subtree.
    ///
    /// Not serialized.
    pub transform_gizmo: Option<ComponentId>,

    component: Option<ComponentId>,
}

impl EditorComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Component for EditorComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "editor"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
