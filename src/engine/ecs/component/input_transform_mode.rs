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

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match self.forward_axis {
            ForwardAxis::Y => "forward_y",
            ForwardAxis::Z => "forward_z",
        };
        let mut ce = ce_call("InputTransformMode", ctor, vec![]);
        if matches!(self.roll_axis, RollAxis::Y) {
            ce = ce.with_call("roll_axis_y", vec![]);
        }
        if self.fps_rotation {
            ce = ce.with_call("fps_rotation", vec![]);
        }
        ce
    }
}
