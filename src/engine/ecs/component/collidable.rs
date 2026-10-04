use super::{Component, ComponentRef};
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollidableMode {
    Static,
    Slide,
}

/// Opts a parent zone into static contact. A slide collider constrains a pose
/// supplied by another system; it owns no velocity or integration state.
#[derive(Debug, Clone, PartialEq)]
pub struct CollidableComponent {
    pub mode: CollidableMode,
    pub enabled: bool,
    pub movement_target_source: Option<ComponentRef>,
    pub movement_target_id: Option<ComponentId>,
    pub movement_target_required: bool,
}

impl CollidableComponent {
    pub fn static_() -> Self {
        Self::new(CollidableMode::Static)
    }

    pub fn slide() -> Self {
        Self::new(CollidableMode::Slide)
    }

    fn new(mode: CollidableMode) -> Self {
        Self {
            mode,
            enabled: true,
            movement_target_source: None,
            movement_target_id: None,
            movement_target_required: false,
        }
    }

    pub fn movement_target(mut self, target: ComponentRef) -> Self {
        self.movement_target_source = Some(target);
        self
    }

    pub fn with_runtime_movement_target(mut self, target: Option<ComponentId>) -> Self {
        self.movement_target_id = target;
        self.movement_target_required = true;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Component for CollidableComponent {
    fn name(&self) -> &'static str {
        "collidable"
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
    ) -> crate::scripting::ast::ComponentExpression {
        use super::ce_helpers::*;
        let ctor = match self.mode {
            CollidableMode::Static => "static",
            CollidableMode::Slide => "slide",
        };
        let mut ce = ce_call("Collidable", ctor, vec![]);
        if !self.enabled {
            ce = ce.with_call("enabled", vec![b(false)]);
        }
        if let Some(target) = &self.movement_target_source {
            let reference = match target {
                ComponentRef::Guid(guid) => format!("@uuid:{guid}"),
                ComponentRef::Query(query) => query.clone(),
            };
            ce = ce.with_call("movement_target", vec![s(&reference)]);
        }
        ce
    }
}
