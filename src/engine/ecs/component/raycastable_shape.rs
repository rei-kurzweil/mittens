use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaycastableShapeType {
    /// Explicitly pick this renderable using an AABB only.
    Aabb,

    /// A finite cone aligned to the renderable's local +Z axis.
    Cone,

    /// A 2D ring/annulus lying in the renderable's local XY plane.
    Ring2D,

    /// A 2D quad lying in the renderable's local XY plane.
    Quad2D,

    /// A 2D triangle lying in the renderable's local XY plane.
    Triangle2D,

    /// A tetrahedron shape.
    Tetrahedron,

    /// A box/cube shape.
    Box,

    /// No explicit shape; infer from `Renderable.base_mesh`.
    InferFromBaseMesh,
}

impl Default for RaycastableShapeType {
    fn default() -> Self {
        Self::InferFromBaseMesh
    }
}

/// Explicit raycast/picking shape descriptor for a renderable.
///
/// This is intended to unify broad-phase bounds selection and future narrow-phase hit tests.
/// For now, the engine can infer a shape from `Renderable.base_mesh` when this component is
/// absent (or set to `InferFromBaseMesh`).
#[derive(Debug, Clone, Copy, Default)]
pub struct RaycastableShapeComponent {
    pub shape: RaycastableShapeType,
}

impl RaycastableShapeComponent {
    pub fn new(shape: RaycastableShapeType) -> Self {
        Self { shape }
    }

    pub fn infer_from_base_mesh() -> Self {
        Self::new(RaycastableShapeType::InferFromBaseMesh)
    }

    pub fn aabb() -> Self {
        Self::new(RaycastableShapeType::Aabb)
    }

    pub fn cone() -> Self {
        Self::new(RaycastableShapeType::Cone)
    }

    pub fn ring_2d() -> Self {
        Self::new(RaycastableShapeType::Ring2D)
    }
}

impl Component for RaycastableShapeComponent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "raycastable_shape"
    }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match self.shape {
            RaycastableShapeType::Aabb => "aabb",
            RaycastableShapeType::Cone => "cone",
            RaycastableShapeType::Ring2D => "ring_2d",
            RaycastableShapeType::Quad2D => "quad_2d",
            RaycastableShapeType::Triangle2D => "triangle_2d",
            RaycastableShapeType::Tetrahedron => "tetrahedron",
            RaycastableShapeType::Box => "box",
            RaycastableShapeType::InferFromBaseMesh => "infer_from_base_mesh",
        };
        ce_call("RaycastableShape", ctor, vec![])
    }
}
