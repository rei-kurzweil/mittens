use super::{Component, ComponentRef};
use crate::engine::ecs::system::model::collision_types::CollisionShape;

/// An invisible spatial region shared by interaction, collision, and solver consumers.
///
/// A bare zone has no physical response. Consumer components/systems decide what
/// the region means. `frame_source` optionally redirects placement to another
/// component's nearest transform.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneComponent {
    pub shape: CollisionShape,
    pub frame_source: Option<ComponentRef>,
    pub roles: Vec<String>,
    pub enabled: bool,
    /// Opt in to overlap enter/exit events for enabled slide collidables.
    pub events_enabled: bool,
}

impl ZoneComponent {
    pub fn cube(half_extents: [f32; 3]) -> Self {
        Self::new(CollisionShape::cube_half_extents(half_extents))
    }

    pub fn sphere(radius: f32) -> Self {
        Self::new(CollisionShape::sphere_radius(radius))
    }

    pub fn capsule_y(radius: f32, half_segment: f32) -> Self {
        Self::new(CollisionShape::capsule_y(radius, half_segment))
    }

    pub fn new(shape: CollisionShape) -> Self {
        Self {
            shape: shape.normalized(),
            frame_source: None,
            roles: Vec::new(),
            enabled: true,
            events_enabled: false,
        }
    }

    /// Place this zone at the referenced component's nearest ancestor transform.
    ///
    /// Query references without an explicit `/` or `../` prefix are resolved in
    /// the zone's containing scope by the spatial-query layer.
    pub fn at(mut self, source: ComponentRef) -> Self {
        self.frame_source = Some(source);
        self
    }

    pub fn role(mut self, role: impl Into<String>) -> Self {
        let role = role.into();
        if !role.is_empty() && !self.roles.iter().any(|existing| existing == &role) {
            self.roles.push(role);
        }
        self
    }

    pub fn enable_events(mut self) -> Self {
        self.events_enabled = true;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Component for ZoneComponent {
    fn name(&self) -> &'static str {
        "zone"
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

        fn ref_expr(reference: &ComponentRef) -> crate::scripting::ast::Expression {
            match reference {
                ComponentRef::Guid(guid) => s(&format!("@uuid:{guid}")),
                ComponentRef::Query(query) => s(query),
            }
        }

        let mut ce = match self.shape {
            CollisionShape::Cube { half_extents } => ce_call(
                "Zone",
                "cube",
                vec![array(nums(half_extents.into_iter().map(f64::from)))],
            ),
            CollisionShape::Sphere { radius } => {
                ce_call("Zone", "sphere", vec![num(radius as f64)])
            }
            CollisionShape::CapsuleY {
                radius,
                half_segment,
            } => ce_call(
                "Zone",
                "capsule_y",
                vec![num(radius as f64), num(half_segment as f64)],
            ),
        };
        if let Some(source) = &self.frame_source {
            ce = ce.with_call("at", vec![ref_expr(source)]);
        }
        for role in &self.roles {
            ce = ce.with_call("role", vec![s(role)]);
        }
        if self.events_enabled {
            ce = ce.with_call("enable_events", vec![]);
        }
        if !self.enabled {
            ce = ce.with_call("enabled", vec![b(false)]);
        }
        ce
    }
}
