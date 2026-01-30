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
        Self::from_cpu_mesh_handle(h, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: 2D square/quad (shared built-in mesh handle).
    pub fn square() -> Self {
        Self::from_cpu_mesh_handle(CpuMeshHandle::QUAD_2D, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: 2D square/quad (unique CPU mesh registered into `render_assets`).
    pub fn square_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::quad_2d());
        Self::from_cpu_mesh_handle(h, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: cube primitive (shared built-in mesh handle).
    pub fn cube() -> Self {
        Self::from_cpu_mesh_handle(CpuMeshHandle::CUBE, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: cube primitive (unique CPU mesh registered into `render_assets`).
    pub fn cube_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::cube());
        Self::from_cpu_mesh_handle(h, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: sphere primitive (shared built-in mesh handle).
    pub fn sphere() -> Self {
        Self::from_cpu_mesh_handle(CpuMeshHandle::SPHERE, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: sphere primitive (unique CPU mesh registered into `render_assets`).
    pub fn sphere_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::sphere());
        Self::from_cpu_mesh_handle(h, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: tetrahedron primitive (shared built-in mesh handle).
    pub fn tetrahedron() -> Self {
        Self::from_cpu_mesh_handle(CpuMeshHandle::TETRAHEDRON, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: tetrahedron primitive (unique CPU mesh registered into `render_assets`).
    pub fn tetrahedron_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::tetrahedron());
        Self::from_cpu_mesh_handle(h, MaterialHandle::TOON_MESH)
    }

    /// Predefined renderable: tetrahedron (alias of `tetrahedron`).
    pub fn color_tetrahedron() -> Self {
        Self::tetrahedron()
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

    fn init(&mut self, queue: &mut crate::engine::ecs::CommandQueue, component: ComponentId) {
        // Queue registration command instead of immediately registering
        queue.queue_register_renderable(component);
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "mesh".to_string(),
            serde_json::json!(self.renderable.mesh.0),
        );
        map.insert(
            "material".to_string(),
            serde_json::json!(self.renderable.material.0),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(mesh) = data.get("mesh") {
            let mesh_id: u32 = serde_json::from_value(mesh.clone())
                .map_err(|e| format!("Failed to decode mesh: {}", e))?;
            self.renderable.mesh = crate::engine::graphics::primitives::CpuMeshHandle(mesh_id);
        }
        if let Some(material) = data.get("material") {
            let material_id: u32 = serde_json::from_value(material.clone())
                .map_err(|e| format!("Failed to decode material: {}", e))?;
            self.renderable.material = MaterialHandle(material_id);
        }
        Ok(())
    }
}
