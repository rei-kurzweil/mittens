use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardAxis {
    /// Classic 2D-ish behavior: W/S move along -Y/+Y.
    Y,
    /// 3D-friendly behavior: W/S move along -Z/+Z.
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollAxis {
    X,
    Y,
    Z,
}

/// Input mode component that controls which axis is treated as "forward" for WASD.
///
/// Intended topology:
/// InputComponent -> (InputTransformModeComponent?)
/// InputComponent -> TransformComponent
#[derive(Debug, Clone, Copy)]
pub struct InputTransformModeComponent {
    pub forward_axis: ForwardAxis,
    pub roll_axis: RollAxis,

    /// If true, rotations are applied in world axes (FPS-style).
    /// If false, rotations are applied in local space (current behavior).
    pub fps_rotation: bool,
}

impl InputTransformModeComponent {
    pub fn forward_y() -> Self {
        Self {
            forward_axis: ForwardAxis::Y,
            roll_axis: RollAxis::Z,
            fps_rotation: false,
        }
    }

    pub fn forward_z() -> Self {
        Self {
            forward_axis: ForwardAxis::Z,
            roll_axis: RollAxis::Z,
            fps_rotation: false,
        }
    }

    pub fn with_fps_rotation(mut self) -> Self {
        self.fps_rotation = true;
        self
    }

    pub fn with_roll_axis_y(mut self) -> Self {
        self.roll_axis = RollAxis::Y;
        self
    }

    pub fn with_roll_axis_z(mut self) -> Self {
        self.roll_axis = RollAxis::Z;
        self
    }
}

impl Default for InputTransformModeComponent {
    fn default() -> Self {
        Self::forward_y()
    }
}

impl Component for InputTransformModeComponent {
    fn name(&self) -> &'static str {
        "input_transform_mode"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        let axis = match self.forward_axis {
            ForwardAxis::Y => "y",
            ForwardAxis::Z => "z",
        };
        map.insert("forward_axis".to_string(), serde_json::json!(axis));

        let roll_axis = match self.roll_axis {
            RollAxis::X => "x",
            RollAxis::Y => "y",
            RollAxis::Z => "z",
        };
        map.insert("roll_axis".to_string(), serde_json::json!(roll_axis));

        map.insert(
            "fps_rotation".to_string(),
            serde_json::json!(self.fps_rotation),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(axis) = data.get("forward_axis") {
            let axis: String = serde_json::from_value(axis.clone())
                .map_err(|e| format!("Failed to decode forward_axis: {}", e))?;
            self.forward_axis = match axis.as_str() {
                "y" | "Y" => ForwardAxis::Y,
                "z" | "Z" => ForwardAxis::Z,
                _ => return Err(format!("Unknown forward_axis: '{}'", axis)),
            };
        }

        if let Some(axis) = data.get("roll_axis") {
            let axis: String = serde_json::from_value(axis.clone())
                .map_err(|e| format!("Failed to decode roll_axis: {}", e))?;
            self.roll_axis = match axis.as_str() {
                "x" | "X" => RollAxis::X,
                "y" | "Y" => RollAxis::Y,
                "z" | "Z" => RollAxis::Z,
                _ => return Err(format!("Unknown roll_axis: '{}'", axis)),
            };
        }

        if let Some(v) = data.get("fps_rotation") {
            self.fps_rotation = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode fps_rotation: {}", e))?;
        }
        Ok(())
    }
}
