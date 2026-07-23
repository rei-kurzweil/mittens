use crate::engine::ecs::component::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Plane used to constrain pointer-driven translation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DraggablePlane {
    Object,
    Camera,
    WorldAxes([[f32; 3]; 2]),
}

impl Default for DraggablePlane {
    fn default() -> Self {
        Self::Object
    }
}

/// Marks its immediate parent Transform as movable by pointer drag gestures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DraggableComponent {
    pub enabled: bool,
    pub move_parent: bool,
    pub plane: DraggablePlane,
}

impl DraggableComponent {
    pub fn new() -> Self {
        Self::on()
    }
    pub fn on() -> Self {
        Self {
            enabled: true,
            move_parent: false,
            plane: DraggablePlane::Object,
        }
    }
    pub fn off() -> Self {
        Self {
            enabled: false,
            move_parent: false,
            plane: DraggablePlane::Object,
        }
    }
    pub fn parent() -> Self {
        Self {
            enabled: true,
            move_parent: true,
            plane: DraggablePlane::Object,
        }
    }
    pub fn with_plane(mut self, plane: DraggablePlane) -> Self {
        self.plane = plane;
        self
    }
}

impl Default for DraggableComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for DraggableComponent {
    fn name(&self) -> &'static str {
        "draggable"
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
            IntentValue::RegisterDraggable {
                component_ids: vec![component],
            },
        );
    }
    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let expression = if !self.enabled {
            ce_call("Draggable", "off", vec![])
        } else if self.move_parent {
            ce_call("Draggable", "parent", vec![])
        } else {
            ce("Draggable")
        };
        match self.plane {
            DraggablePlane::Object => expression,
            DraggablePlane::Camera => expression.with_call("plane", vec![s("camera")]),
            DraggablePlane::WorldAxes(axes) => expression.with_call(
                "plane",
                vec![array(
                    axes.into_iter()
                        .map(|axis| array(nums(axis.into_iter().map(|v| v as f64))))
                        .collect(),
                )],
            ),
        }
    }
}
