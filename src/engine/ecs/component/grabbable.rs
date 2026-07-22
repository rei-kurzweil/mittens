use crate::engine::ecs::component::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Marks its immediate parent Transform as movable by pointer drag gestures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GrabbablePlane {
    /// Move across the dragged target's parent-local X/Y plane.
    Object,
    /// Preserve the pointer gesture's camera-facing drag plane.
    Camera,
    /// Move across the span of two world-space axes.
    WorldAxes([[f32; 3]; 2]),
}

impl Default for GrabbablePlane {
    fn default() -> Self {
        Self::Object
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrabbableComponent {
    pub enabled: bool,
    /// Handle mode: move the owner's parent Transform instead of the owner itself.
    pub move_parent: bool,
    pub plane: GrabbablePlane,
}

impl GrabbableComponent {
    pub fn new() -> Self {
        Self::on()
    }

    pub fn on() -> Self {
        Self {
            enabled: true,
            move_parent: false,
            plane: GrabbablePlane::Object,
        }
    }

    pub fn off() -> Self {
        Self {
            enabled: false,
            move_parent: false,
            plane: GrabbablePlane::Object,
        }
    }

    pub fn parent() -> Self {
        Self {
            enabled: true,
            move_parent: true,
            plane: GrabbablePlane::Object,
        }
    }

    pub fn with_plane(mut self, plane: GrabbablePlane) -> Self {
        self.plane = plane;
        self
    }
}

impl Default for GrabbableComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for GrabbableComponent {
    fn name(&self) -> &'static str {
        "grabbable"
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
            IntentValue::RegisterGrabbable {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;

        let mut expression = if !self.enabled {
            ce_call("Grabbable", "off", vec![])
        } else if self.move_parent {
            ce_call("Grabbable", "parent", vec![])
        } else {
            ce("Grabbable")
        };
        expression = match self.plane {
            GrabbablePlane::Object => expression,
            GrabbablePlane::Camera => expression.with_call("plane", vec![s("camera")]),
            GrabbablePlane::WorldAxes(axes) => expression.with_call(
                "plane",
                vec![array(
                    axes.into_iter()
                        .map(|axis| array(nums(axis.into_iter().map(|value| value as f64))))
                        .collect(),
                )],
            ),
        };
        expression
    }
}
