use super::{Component, ComponentRef};
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollidableMode {
    Static,
    Slide,
}

/// Collision participation is independent of permission to correct movement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CollisionResponse {
    #[default]
    Slide,
    ReadOnly,
}
impl CollisionResponse {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "slide" => Ok(Self::Slide),
            "read_only" => Ok(Self::ReadOnly),
            _ => Err("collision response must be 'slide' or 'read_only'".into()),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Slide => "slide",
            Self::ReadOnly => "read_only",
        }
    }
}

/// Opts a parent zone into static contact. A slide collider constrains a pose
/// supplied by another system; it owns no velocity or integration state.
#[derive(Debug, Clone, PartialEq)]
pub struct CollidableComponent {
    pub mode: CollidableMode,
    pub response: CollisionResponse,
    pub enabled: bool,
    /// Runtime self-filtering owner, independent of any response target.
    pub query_owner: Option<ComponentId>,
    /// Coulomb friction coefficient supplied by a static surface; zero is frictionless.
    pub friction: f32,
    pub movement_target_source: Option<ComponentRef>,
    pub movement_target_id: Option<ComponentId>,
    pub movement_target_required: bool,
    /// Motion provider for the generic sweeping-zone query. Binding a
    /// `Velocity` opts this collidable into velocity-aware sweeps; the event
    /// remains scoped to the parent zone.
    pub velocity_source: Option<ComponentRef>,
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
            response: if mode == CollidableMode::Static {
                CollisionResponse::ReadOnly
            } else {
                CollisionResponse::Slide
            },
            enabled: true,
            query_owner: None,
            friction: 0.0,
            movement_target_source: None,
            movement_target_id: None,
            movement_target_required: false,
            velocity_source: None,
        }
    }

    pub fn with_response(mut self, response: CollisionResponse) -> Result<Self, String> {
        if self.mode == CollidableMode::Static && response != CollisionResponse::ReadOnly {
            return Err("static geometry cannot have slide response".into());
        }
        self.response = response;
        Ok(self)
    }

    pub fn with_friction(mut self, friction: f32) -> Result<Self, String> {
        if self.mode != CollidableMode::Static {
            return Err("friction belongs to Collidable.static() surfaces".into());
        }
        if !friction.is_finite() || friction < 0.0 {
            return Err("friction must be finite and nonnegative".into());
        }
        self.friction = friction;
        Ok(self)
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

    pub fn velocity(mut self, velocity: ComponentRef) -> Self {
        self.velocity_source = Some(velocity);
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
        if self.mode == CollidableMode::Slide && self.response != CollisionResponse::Slide {
            ce = ce.with_call("response", vec![s(self.response.as_str())]);
        }
        if self.friction != 0.0 {
            ce = ce.with_call("friction", vec![num(self.friction as f64)]);
        }
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
        if let Some(velocity) = &self.velocity_source {
            let reference = match velocity {
                ComponentRef::Guid(guid) => format!("@uuid:{guid}"),
                ComponentRef::Query(query) => query.clone(),
            };
            ce = ce.with_call("velocity", vec![s(&reference)]);
        }
        ce
    }
}
