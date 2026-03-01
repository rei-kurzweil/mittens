use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
}

impl GizmoAxis {
    pub fn unit_vec3(self) -> [f32; 3] {
        match self {
            GizmoAxis::X => [1.0, 0.0, 0.0],
            GizmoAxis::Y => [0.0, 1.0, 0.0],
            GizmoAxis::Z => [0.0, 0.0, 1.0],
        }
    }
}

/// Handle marker: translate along an axis.
///
/// This component is intended to be an ancestor of the entire clickable handle subtree.
#[derive(Debug, Clone, Copy)]
pub struct GizmoTranslateComponent {
    pub axis: GizmoAxis,
}

impl GizmoTranslateComponent {
    pub fn new(axis: GizmoAxis) -> Self {
        Self { axis }
    }
}

impl Component for GizmoTranslateComponent {
    fn name(&self) -> &'static str {
        "gizmo_translate"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "axis".to_string(),
            serde_json::json!(match self.axis {
                GizmoAxis::X => "x",
                GizmoAxis::Y => "y",
                GizmoAxis::Z => "z",
            }),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(axis) = data.get("axis") {
            let axis: String = serde_json::from_value(axis.clone())
                .map_err(|e| format!("Failed to decode axis: {e}"))?;
            self.axis = match axis.as_str() {
                "x" | "X" => GizmoAxis::X,
                "y" | "Y" => GizmoAxis::Y,
                "z" | "Z" => GizmoAxis::Z,
                other => return Err(format!("Unknown axis '{other}'")),
            };
        }
        Ok(())
    }
}

/// Handle marker: rotate around an axis.
///
/// This component is intended to be an ancestor of the entire clickable handle subtree.
#[derive(Debug, Clone, Copy)]
pub struct GizmoRotateComponent {
    pub axis: GizmoAxis,
}

impl GizmoRotateComponent {
    pub fn new(axis: GizmoAxis) -> Self {
        Self { axis }
    }
}

impl Component for GizmoRotateComponent {
    fn name(&self) -> &'static str {
        "gizmo_rotate"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "axis".to_string(),
            serde_json::json!(match self.axis {
                GizmoAxis::X => "x",
                GizmoAxis::Y => "y",
                GizmoAxis::Z => "z",
            }),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(axis) = data.get("axis") {
            let axis: String = serde_json::from_value(axis.clone())
                .map_err(|e| format!("Failed to decode axis: {e}"))?;
            self.axis = match axis.as_str() {
                "x" | "X" => GizmoAxis::X,
                "y" | "Y" => GizmoAxis::Y,
                "z" | "Z" => GizmoAxis::Z,
                other => return Err(format!("Unknown axis '{other}'")),
            };
        }
        Ok(())
    }
}

/// Handle marker: scale along an axis.
///
/// This component is intended to be an ancestor of the entire clickable handle subtree.
#[derive(Debug, Clone, Copy)]
pub struct GizmoScaleComponent {
    pub axis: GizmoAxis,
}

impl GizmoScaleComponent {
    pub fn new(axis: GizmoAxis) -> Self {
        Self { axis }
    }
}

impl Component for GizmoScaleComponent {
    fn name(&self) -> &'static str {
        "gizmo_scale"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "axis".to_string(),
            serde_json::json!(match self.axis {
                GizmoAxis::X => "x",
                GizmoAxis::Y => "y",
                GizmoAxis::Z => "z",
            }),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(axis) = data.get("axis") {
            let axis: String = serde_json::from_value(axis.clone())
                .map_err(|e| format!("Failed to decode axis: {e}"))?;
            self.axis = match axis.as_str() {
                "x" | "X" => GizmoAxis::X,
                "y" | "Y" => GizmoAxis::Y,
                "z" | "Z" => GizmoAxis::Z,
                other => return Err(format!("Unknown axis '{other}'")),
            };
        }
        Ok(())
    }
}

/// A simple transform gizmo.
///
/// Attach this as a child of a TransformComponent you want to manipulate.
/// On init, a 9-part visual subtree is spawned under the gizmo component.
/// When a drag gesture is active on a gizmo renderable, GizmoSystem applies the drag delta
/// to the TransformComponent it is attached under.
#[derive(Debug, Clone, Copy)]
pub struct GizmoComponent {
    /// Runtime: resolved target TransformComponent id.
    ///
    /// This is bound during `REGISTER_GIZMO` by walking up ancestry and finding the nearest
    /// TransformComponent.
    pub target_transform: Option<ComponentId>,

    /// Runtime: raycaster currently driving this gizmo (single-pointer for now).
    pub active_raycaster: Option<ComponentId>,

    /// Root TransformComponent id of the gizmo visual subtree (spawned on init).
    pub visual_root: Option<ComponentId>,

    /// Runtime: optional debug plane subtree root.
    ///
    /// When enabled, GizmoSystem spawns a thin quad/cube aligned to the drag plane captured at
    /// DragStart to visualize the projection surface used by screen-space dragging.
    pub debug_drag_plane_root: Option<ComponentId>,

    component: Option<ComponentId>,
}

impl GizmoComponent {
    /// Create a gizmo.
    ///
    /// The target transform is resolved automatically from gizmo ancestry on init.
    pub fn new() -> Self {
        Self {
            target_transform: None,
            active_raycaster: None,
            visual_root: None,
            debug_drag_plane_root: None,
            component: None,
        }
    }

    /// Back-compat constructor name (gizmos are no longer mode-based).
    pub fn translate() -> Self {
        Self::new()
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

        if let Some(root) = self.debug_drag_plane_root.take() {
            queue.queue_remove_subtree(root);
        }
    }
}
