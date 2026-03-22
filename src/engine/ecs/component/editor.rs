use crate::engine::ecs::ComponentId;
use crate::engine::ecs::IntentValue;
use crate::engine::ecs::SignalEmitter;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformGizmoCoordSpace {
    Local,
    World,
}

impl Default for TransformGizmoCoordSpace {
    fn default() -> Self {
        Self::World
    }
}

/// Marks an "editor root" subtree.
///
/// When a renderable under this subtree is clicked, the editor selection system can reattach
/// the editor's gizmos (e.g. TransformGizmo) to the clicked target.
#[derive(Debug, Clone, Copy)]
pub struct EditorComponent {
    /// Runtime cache: resolved TransformGizmoComponent id within this editor subtree.
    ///
    /// Not serialized.
    pub transform_gizmo: Option<ComponentId>,

    /// Runtime: currently selected TransformComponent.
    ///
    /// Set by EditorSystem on DragStart. Read by InspectorSystem to drive panel content.
    /// Not serialized.
    pub selected: Option<ComponentId>,

    /// Coordinate space used for translation handles (arrows).
    pub transform_gizmo_translation_space: TransformGizmoCoordSpace,

    /// Coordinate space used for rotation handles (rings).
    pub transform_gizmo_rotation_space: TransformGizmoCoordSpace,

    /// Spawn world-tree and inspector panels automatically on init. Default: true.
    pub spawn_panels: bool,

    /// World-space position of the world-tree panel. Default: (-0.7, 1.6, -1.2).
    pub world_panel_pos: (f32, f32, f32),

    /// World-space position of the inspector panel. Default: (0.5, 1.6, -1.2).
    pub inspector_panel_pos: (f32, f32, f32),

    component: Option<ComponentId>,
}

impl Default for EditorComponent {
    fn default() -> Self {
        Self {
            transform_gizmo: None,
            selected: None,
            // Default to the common editor expectation: translate in World, rotate in Local.
            transform_gizmo_translation_space: TransformGizmoCoordSpace::World,
            transform_gizmo_rotation_space: TransformGizmoCoordSpace::Local,
            spawn_panels: true,
            world_panel_pos: (-0.7, 1.6, -1.2),
            inspector_panel_pos: (0.5, 1.6, -1.2),
            component: None,
        }
    }
}

impl EditorComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_transform_gizmo_translation_space(
        mut self,
        space: TransformGizmoCoordSpace,
    ) -> Self {
        self.transform_gizmo_translation_space = space;
        self
    }

    pub fn with_transform_gizmo_rotation_space(mut self, space: TransformGizmoCoordSpace) -> Self {
        self.transform_gizmo_rotation_space = space;
        self
    }

    /// Suppress automatic panel spawning. Call as `.with_panels(false)`.
    pub fn with_panels(mut self, enabled: bool) -> Self {
        self.spawn_panels = enabled;
        self
    }

    /// Override panel positions (world_panel, inspector_panel).
    pub fn with_panel_positions(
        mut self,
        world_panel: (f32, f32, f32),
        inspector_panel: (f32, f32, f32),
    ) -> Self {
        self.world_panel_pos = world_panel;
        self.inspector_panel_pos = inspector_panel;
        self
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

    fn init(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RegisterEditor {
                component_ids: vec![component],
            },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "transform_gizmo_translation_space".to_string(),
            serde_json::json!(match self.transform_gizmo_translation_space {
                TransformGizmoCoordSpace::Local => "local",
                TransformGizmoCoordSpace::World => "world",
            }),
        );
        map.insert(
            "transform_gizmo_rotation_space".to_string(),
            serde_json::json!(match self.transform_gizmo_rotation_space {
                TransformGizmoCoordSpace::Local => "local",
                TransformGizmoCoordSpace::World => "world",
            }),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("transform_gizmo_translation_space") {
            let s: String = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode transform_gizmo_translation_space: {e}"))?;
            self.transform_gizmo_translation_space = match s.trim().to_ascii_lowercase().as_str() {
                "local" => TransformGizmoCoordSpace::Local,
                "world" => TransformGizmoCoordSpace::World,
                other => {
                    return Err(format!(
                        "Unknown transform_gizmo_translation_space '{other}'"
                    ));
                }
            };
        }

        if let Some(v) = data.get("transform_gizmo_rotation_space") {
            let s: String = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode transform_gizmo_rotation_space: {e}"))?;
            self.transform_gizmo_rotation_space = match s.trim().to_ascii_lowercase().as_str() {
                "local" => TransformGizmoCoordSpace::Local,
                "world" => TransformGizmoCoordSpace::World,
                other => return Err(format!("Unknown transform_gizmo_rotation_space '{other}'")),
            };
        }

        Ok(())
    }
}
