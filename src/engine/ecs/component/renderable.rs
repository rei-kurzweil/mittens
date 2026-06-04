use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::mesh::MeshFactory;
use crate::engine::graphics::primitives::{
    CpuMeshHandle, InstanceHandle, MaterialHandle, Renderable,
};
use crate::engine::graphics::render_assets::RenderAssets;

/// Renderable component.
#[derive(Debug, Clone)]
pub struct RenderableComponent {
    pub renderable: Renderable,

    /// VisualWorld instance handle created for this renderable.
    pub handle: Option<InstanceHandle>,

    component: Option<ComponentId>,
}

impl RenderableComponent {
    pub fn new(renderable: Renderable) -> Self {
        Self {
            renderable,
            handle: None,
            component: None,
        }
    }

    pub fn from_cpu_mesh_handle(h: CpuMeshHandle, material: MaterialHandle) -> Self {
        Self::new(Renderable::new(h, material))
    }

    pub fn get_handle(&self) -> Option<InstanceHandle> {
        self.handle
    }

    /// Predefined renderable: 2D triangle (shared built-in mesh handle).
    pub fn triangle() -> Self {
        Self::from_cpu_mesh_handle(CpuMeshHandle::TRIANGLE_2D, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: 2D triangle (unique CPU mesh registered into `render_assets`).
    pub fn triangle_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::triangle_2d());
        Self::new(
            Renderable::new(h, MaterialHandle::TOON_MESH)
                .with_base_mesh(CpuMeshHandle::TRIANGLE_2D),
        )
    }

    /// Predefined renderable: 2D square/quad (shared built-in mesh handle).
    pub fn square() -> Self {
        Self::from_cpu_mesh_handle(CpuMeshHandle::QUAD_2D, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: 2D plane/quad (alias of `square`).
    pub fn plane() -> Self {
        Self::square()
    }

    /// Predefined renderable: 2D square/quad (unique CPU mesh registered into `render_assets`).
    pub fn square_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::quad_2d());
        Self::new(
            Renderable::new(h, MaterialHandle::TOON_MESH).with_base_mesh(CpuMeshHandle::QUAD_2D),
        )
    }

    /// Predefined renderable: cube primitive (shared built-in mesh handle).
    pub fn cube() -> Self {
        Self::from_cpu_mesh_handle(CpuMeshHandle::CUBE, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: cube primitive (unique CPU mesh registered into `render_assets`).
    pub fn cube_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::cube());
        Self::new(Renderable::new(h, MaterialHandle::TOON_MESH).with_base_mesh(CpuMeshHandle::CUBE))
    }

    /// Predefined renderable: sphere primitive (shared built-in mesh handle).
    pub fn sphere() -> Self {
        Self::from_cpu_mesh_handle(CpuMeshHandle::SPHERE, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: sphere primitive (unique CPU mesh registered into `render_assets`).
    pub fn sphere_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::sphere());
        Self::new(
            Renderable::new(h, MaterialHandle::TOON_MESH).with_base_mesh(CpuMeshHandle::SPHERE),
        )
    }

    /// Predefined renderable: tetrahedron primitive (shared built-in mesh handle).
    pub fn tetrahedron() -> Self {
        Self::from_cpu_mesh_handle(CpuMeshHandle::TETRAHEDRON, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: tetrahedron primitive (unique CPU mesh registered into `render_assets`).
    pub fn tetrahedron_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::tetrahedron());
        Self::new(
            Renderable::new(h, MaterialHandle::TOON_MESH)
                .with_base_mesh(CpuMeshHandle::TETRAHEDRON),
        )
    }

    /// Predefined renderable: tetrahedron (alias of `tetrahedron`).
    pub fn color_tetrahedron() -> Self {
        Self::tetrahedron()
    }

    /// Predefined renderable: 2D circle (shared built-in mesh handle).
    pub fn circle2d() -> Self {
        Self::from_cpu_mesh_handle(CpuMeshHandle::CIRCLE_2D, MaterialHandle::TOON_MESH)
    }
}

impl Component for RenderableComponent {
    fn name(&self) -> &'static str {
        "renderable"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterRenderable {
                component_ids: vec![component],
            },
        );
    }

    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RemoveRenderable {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        // Map known shared primitive handles back to the constructor that
        // produces them. Custom dynamic meshes (registered with
        // `RenderAssets`) don't have a round-tripable identifier and are
        // emitted as bare `Renderable` — clones will get the default mesh.
        let ctor = match self.renderable.base_mesh {
            CpuMeshHandle::CUBE => Some("cube"),
            CpuMeshHandle::SPHERE => Some("sphere"),
            CpuMeshHandle::TRIANGLE_2D => Some("triangle"),
            CpuMeshHandle::QUAD_2D => Some("square"),
            CpuMeshHandle::TETRAHEDRON => Some("tetrahedron"),
            CpuMeshHandle::CIRCLE_2D => Some("circle2d"),
            _ => None,
        };
        match ctor {
            Some(name) => ce_call("Renderable", name, vec![]),
            None => ce("Renderable"),
        }
    }
}
