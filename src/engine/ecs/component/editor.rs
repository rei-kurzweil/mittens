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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorInteractionMode {
    Select,
    Cursor3d,
    SelectAndCursor,
}

impl Default for EditorInteractionMode {
    fn default() -> Self {
        Self::Select
    }
}

/// Marks an "editor root" subtree.
///
/// When a renderable under this subtree is clicked, the editor selection system can reattach
/// the editor's gizmos (e.g. TransformGizmo) to the clicked target.
#[derive(Debug, Clone)]
pub struct EditorComponent {
    /// Declaratively prefer this editor as the active workspace editor.
    pub active: bool,

    /// Runtime cache: resolved TransformGizmoComponent id within this editor subtree.
    ///
    /// Not serialized.
    pub transform_gizmo: Option<ComponentId>,

    /// Runtime: currently selected TransformComponent.
    ///
    /// Set by EditorSystem on DragStart. Read by InspectorSystem to drive panel content.
    /// Not serialized.
    pub selected: Option<ComponentId>,

    /// Runtime editor interaction mode.
    pub interaction_mode: EditorInteractionMode,

    /// Coordinate space used for translation handles (arrows).
    pub transform_gizmo_translation_space: TransformGizmoCoordSpace,

    /// Coordinate space used for rotation handles (rings).
    pub transform_gizmo_rotation_space: TransformGizmoCoordSpace,

    /// Spawn world-tree and inspector panels automatically on init. Default: true.
    pub spawn_panels: bool,

    /// Include editor-owned runtime UI and editor wrappers when serializing a scene.
    /// Default: false.
    pub serialize_editor_panels: bool,

    /// Optional asset directory to scan for MMS asset modules when this editor is registered.
    pub asset_dir: Option<String>,

    /// World-space position of the world-tree panel. Default: (-0.7, 1.6, -1.2).
    pub world_panel_pos: (f32, f32, f32),

    /// World-space position of the inspector panel.
    /// If set to the same x as `world_panel_pos` (the default), the inspector is
    /// auto-placed to the right of the world panel using `estimate_panel_width`.
    pub inspector_panel_pos: (f32, f32, f32),

    component: Option<ComponentId>,
}

impl Default for EditorComponent {
    fn default() -> Self {
        Self {
            active: false,
            transform_gizmo: None,
            selected: None,
            interaction_mode: EditorInteractionMode::Select,
            // Default to the common editor expectation: translate in World, rotate in Local.
            transform_gizmo_translation_space: TransformGizmoCoordSpace::World,
            transform_gizmo_rotation_space: TransformGizmoCoordSpace::Local,
            spawn_panels: true,
            serialize_editor_panels: false,
            asset_dir: None,
            world_panel_pos: (-0.7, 1.6, -1.2),
            // Same x as world_panel_pos intentionally — InspectorSystem::setup_panels_for_editor
            // detects this and auto-places the inspector to the right of the world panel using
            // LayoutSystem::estimate_panel_width + PANEL_GAP.
            inspector_panel_pos: (-0.7, 1.6, -1.2),
            component: None,
        }
    }
}

impl EditorComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_active(mut self, active: bool) -> Self {
        self.active = active;
        self
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

    pub fn with_interaction_mode(mut self, mode: EditorInteractionMode) -> Self {
        self.interaction_mode = mode;
        self
    }

    /// Suppress automatic panel spawning. Call as `.with_panels(false)`.
    pub fn with_panels(mut self, enabled: bool) -> Self {
        self.spawn_panels = enabled;
        self
    }

    pub fn with_serialize_editor_panels(mut self, enabled: bool) -> Self {
        self.serialize_editor_panels = enabled;
        self
    }

    pub fn with_asset_dir(mut self, path: impl Into<String>) -> Self {
        self.asset_dir = Some(path.into());
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

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let translation = match self.transform_gizmo_translation_space {
            TransformGizmoCoordSpace::Local => "local",
            TransformGizmoCoordSpace::World => "world",
        };
        let rotation = match self.transform_gizmo_rotation_space {
            TransformGizmoCoordSpace::Local => "local",
            TransformGizmoCoordSpace::World => "world",
        };
        let interaction_mode = match self.interaction_mode {
            EditorInteractionMode::Select => "select",
            EditorInteractionMode::Cursor3d => "cursor_3d",
            EditorInteractionMode::SelectAndCursor => "select_cursor",
        };
        let mut expr = ce("Editor")
            .with_call("interaction_mode", vec![s(interaction_mode)])
            .with_call("translation_space", vec![s(translation)])
            .with_call("rotation_space", vec![s(rotation)]);
        if self.active {
            expr = expr.with_call("active", vec![]);
        }
        if !self.spawn_panels {
            expr = expr.with_call("panels", vec![b(false)]);
        }
        if self.serialize_editor_panels {
            expr = expr.with_call("serialize_editor_panels", vec![b(true)]);
        }
        if let Some(asset_dir) = &self.asset_dir {
            expr = expr.with_call("asset_dir", vec![s(asset_dir)]);
        }
        expr
    }
}
