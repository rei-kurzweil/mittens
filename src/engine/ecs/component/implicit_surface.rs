use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImplicitSurfaceComponent {
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub voxel_size: f32,
    pub iso_level: f32,
    pub smooth_min_radius: f32,
    /// Intersect the field with the half-space above this local Y plane.
    pub clip_floor: Option<f32>,
}

impl Default for ImplicitSurfaceComponent {
    fn default() -> Self {
        Self {
            bounds_min: [-1.5; 3],
            bounds_max: [1.5; 3],
            voxel_size: 0.15,
            iso_level: 0.0,
            smooth_min_radius: 0.0,
            clip_floor: None,
        }
    }
}

impl Component for ImplicitSurfaceComponent {
    fn name(&self) -> &'static str {
        "implicit_surface"
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
        use crate::engine::ecs::component::ce_helpers::{CeBuilder, ce, nums};

        let mut result = ce("ImplicitSurface")
            .with_call(
                "bounds",
                nums(
                    self.bounds_min
                        .into_iter()
                        .chain(self.bounds_max)
                        .map(f64::from),
                ),
            )
            .with_call("voxel_size", nums([self.voxel_size as f64]))
            .with_call("iso_level", nums([self.iso_level as f64]))
            .with_call("smooth_min_radius", nums([self.smooth_min_radius as f64]));
        if let Some(floor) = self.clip_floor {
            result = result.with_call("clip_floor", nums([floor as f64]));
        }
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImplicitSphereComponent {
    pub radius: f32,
}

impl ImplicitSphereComponent {
    pub fn radius(radius: f32) -> Self {
        Self { radius }
    }
}

impl Default for ImplicitSphereComponent {
    fn default() -> Self {
        Self { radius: 1.0 }
    }
}

impl Component for ImplicitSphereComponent {
    fn name(&self) -> &'static str {
        "implicit_sphere"
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
        use crate::engine::ecs::component::ce_helpers::{ce_call, nums};
        ce_call("ImplicitSphere", "radius", nums([self.radius as f64]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::unparser::unparse_component;

    #[test]
    fn authored_parameters_serialize_without_runtime_state() {
        let surface = ImplicitSurfaceComponent {
            bounds_min: [-2.0, -1.0, -3.0],
            bounds_max: [4.0, 5.0, 6.0],
            voxel_size: 0.2,
            iso_level: 0.1,
            smooth_min_radius: 0.6,
            clip_floor: Some(-0.5),
        };
        let world = crate::engine::ecs::World::default();
        let text = unparse_component(&surface.to_mms_ast(&world));
        assert!(text.contains("ImplicitSurface.bounds"));
        assert!(text.contains("voxel_size("));
        assert!(text.contains("smooth_min_radius("));

        let sphere = ImplicitSphereComponent::radius(1.75);
        let text = unparse_component(&sphere.to_mms_ast(&world));
        assert!(text.contains("ImplicitSphere.radius(1.75)"));
    }
}
