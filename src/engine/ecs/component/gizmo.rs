use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformGizmoAxis {
    X,
    Y,
    Z,
}

impl TransformGizmoAxis {
    pub fn unit_vec3(self) -> [f32; 3] {
        match self {
            TransformGizmoAxis::X => [1.0, 0.0, 0.0],
            TransformGizmoAxis::Y => [0.0, 1.0, 0.0],
            TransformGizmoAxis::Z => [0.0, 0.0, 1.0],
        }
    }
}

/// Handle marker: translate along an axis.
///
/// This component is intended to be an ancestor of the entire clickable handle subtree.
#[derive(Debug, Clone, Copy)]
pub struct TransformGizmoTranslateComponent {
    pub axis: TransformGizmoAxis,
}

impl TransformGizmoTranslateComponent {
    pub fn new(axis: TransformGizmoAxis) -> Self {
        Self { axis }
    }
}

impl Component for TransformGizmoTranslateComponent {
    fn name(&self) -> &'static str {
        "transform_gizmo_translate"
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
                TransformGizmoAxis::X => "x",
                TransformGizmoAxis::Y => "y",
                TransformGizmoAxis::Z => "z",
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
                "x" | "X" => TransformGizmoAxis::X,
                "y" | "Y" => TransformGizmoAxis::Y,
                "z" | "Z" => TransformGizmoAxis::Z,
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
pub struct TransformGizmoRotateComponent {
    pub axis: TransformGizmoAxis,
}

impl TransformGizmoRotateComponent {
    pub fn new(axis: TransformGizmoAxis) -> Self {
        Self { axis }
    }
}

impl Component for TransformGizmoRotateComponent {
    fn name(&self) -> &'static str {
        "transform_gizmo_rotate"
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
                TransformGizmoAxis::X => "x",
                TransformGizmoAxis::Y => "y",
                TransformGizmoAxis::Z => "z",
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
                "x" | "X" => TransformGizmoAxis::X,
                "y" | "Y" => TransformGizmoAxis::Y,
                "z" | "Z" => TransformGizmoAxis::Z,
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
pub struct TransformGizmoScaleComponent {
    pub axis: TransformGizmoAxis,
}

impl TransformGizmoScaleComponent {
    pub fn new(axis: TransformGizmoAxis) -> Self {
        Self { axis }
    }
}

impl Component for TransformGizmoScaleComponent {
    fn name(&self) -> &'static str {
        "transform_gizmo_scale"
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
                TransformGizmoAxis::X => "x",
                TransformGizmoAxis::Y => "y",
                TransformGizmoAxis::Z => "z",
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
                "x" | "X" => TransformGizmoAxis::X,
                "y" | "Y" => TransformGizmoAxis::Y,
                "z" | "Z" => TransformGizmoAxis::Z,
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
/// When a drag gesture is active on a gizmo renderable, TransformGizmoSystem applies the drag delta
/// to the TransformComponent it is attached under.
#[derive(Debug, Clone, Copy)]
pub struct TransformGizmoComponent {
    /// Visual scale applied to the gizmo's rendered/interactive subtree.
    ///
    /// This scales the gizmo visuals without affecting the target transform.
    pub scale: f32,

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

impl TransformGizmoComponent {
    /// Create a gizmo.
    ///
    /// The target transform is resolved automatically from gizmo ancestry on init.
    pub fn new() -> Self {
        Self {
            scale: 1.0,
            target_transform: None,
            active_raycaster: None,
            visual_root: None,
            debug_drag_plane_root: None,
            component: None,
        }
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Back-compat constructor name (gizmos are no longer mode-based).
    pub fn translate() -> Self {
        Self::new()
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Component for TransformGizmoComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "transform_gizmo"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("scale".to_string(), serde_json::json!(self.scale));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("scale") {
            self.scale = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode scale: {e}"))?;
        }
        Ok(())
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterTransformGizmo { component },
        );
    }

    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        _component: ComponentId,
    ) {
        if let Some(root) = self.visual_root.take() {
            emit.push_intent_now(
                root,
                crate::engine::ecs::IntentValue::RemoveSubtree { target: vec![root] },
            );
        }

        if let Some(root) = self.debug_drag_plane_root.take() {
            emit.push_intent_now(
                root,
                crate::engine::ecs::IntentValue::RemoveSubtree { target: vec![root] },
            );
        }
    }
}
